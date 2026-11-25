# ARION Pit Wall — what the driver app sends (protocol version 1)

All numbers are little-endian. Two WebRTC data channels, both opened by the coach's browser:

| Channel | Settings | Carries |
|---|---|---|
| `fast` | `ordered: false, maxRetransmits: 0` | fast frames (kind 1, up to 30 Hz), cars frames (kind 2, 10 Hz) and comparison frames (kind 3, with kind 1), binary |
| `slow` | reliable, ordered | slow message (JSON text, 2 Hz), reference (`ref`) and corner verdicts (`corner`); latency ping `p:<number>` → reply `q:<same number>` |

Every binary frame starts with `version (u8) = 1`, `kind (u8)`.

## Fast frame (kind 1), 76 bytes — the driver's car

| Offset | Type | Field |
|---|---|---|
| 0 | u8 | version = 1 |
| 1 | u8 | kind = 1 |
| 2 | u16 | flags: bit0 ABS active · bit1 TC active · bit2 in pits · bit3 lap invalid · bits4–7 wheel lock FL, FR, RL, RR |
| 4 | u32 | sequence (+1 per frame; a gap = a dropped frame) |
| 8 | f64 | game time of this sample (s) |
| 16 | f32 | lap distance (m) |
| 20 | i16 | lap number |
| 22 | i8 | gear (−1 reverse, 0 neutral) |
| 23 | u8 | reserved |
| 24 | f32 ×3 | world position x, y, z (m) |
| 36 | f32 | speed (m/s) |
| 40 | f32 | engine rpm |
| 44 | f32 | throttle 0–1 (pedal, unfiltered) |
| 48 | f32 | brake 0–1 (pedal, unfiltered) |
| 52 | f32 | steering −1…1 |
| 56 | f32 | clutch 0–1 |
| 60 | f32 ×4 | brake pressure FL, FR, RL, RR, 0–1 (LMU's value; not bar) |

Wheel lock: speed > 5 m/s, brake pedal > 5 %, and the wheel turning under 30 % of road speed.

## Cars frame (kind 2) — every car, for the map

`version, kind=2, count (u16)`, then 16 bytes per car:
`slot id (u16), place (u8), bits (u8: bit0 in pits, bit1 the driver's car), x (f32), z (f32), lap distance (f32)`.
The slot id matches `grid[].id` in the slow message. Positions come from each car's telemetry slot (LMU updates
those ~50 times a second), falling back to timing positions (5–6 a second) for a car without one. Sent 10 times a second;
the dashboard moves cars smoothly between frames.

## Comparison frame (kind 3), 24 bytes — live delta and ghost (since v0.1.15)

Sent with each fast frame once there is a class-best reference lap and the car is on its line.

| Offset | Type | Field |
|---|---|---|
| 0 | u8 | version = 1 |
| 1 | u8 | kind = 3 |
| 2 | u16 | reference revision (matches `rev` in the `ref` message) |
| 4 | f32 | live delta to the class best (s; + = slower) |
| 8 | f32 ×2 | ghost (the class-best car at the same time into the lap): world x, z |
| 16 | f32 | distance along the reference line (m) |
| 20 | f32 | live delta to the driver's own best lap (s; NaN while there is none) |

The class best is the fastest valid lap of any car in the driver's class, recorded since the app started, from LMU's
telemetry. The driver's own best is the fastest valid lap of their own car, used from the start of the lap after it.

## Reference message (JSON, `"t": "ref"`)

Sent when the reference changes and every 20 s: `rev`, `car` (slot id), `name`, `own` (true when the class best is the
driver's own lap), `lap` (class-best lap time), `own_lap` (the driver's best lap time or null), `len` (m),
`line` ([x, z, distance, km/h] about every 5 m, at most ~1500 points), `corners` (n, brake/apex/end distances, minimum
km/h, `at_brake` and `at_apex` as [x, z]), `recent` (the last corner verdicts).

## Corner verdict (JSON, `"t": "corner"`)

After each corner of the reference: `n`, `lap`, `dt` (s lost (+) or gained (−) from 50 m before braking to the
corner's end), `brake_m` (− = braked earlier than the reference), `vmin` and `vexit` (km/h against the reference),
`abs`, `lock`, `rev`.

## Slow message (JSON, `"t": "slow"`)

`session` (track, type, time, phase, flags, air/track temperature, rain, wetness min/avg/max, grip, clouds; `sectorFlags` is LMU's `mSectorFlag` as sent: 1 = local yellow in that sector,
11 = clear (seen in LMU), so readers must test `== 1`, not `> 0`),
`car` (fuel, capacity, virtual energy, battery, TC (`tc`, LMU's `mTC`, «TC1» traction control; since v0.1.27 also
`tcCut`, `mTCCut`, «TC2» power cut, and `tcSlip`, `mTCSlip`, «TC3» slip angle), ABS, map, brake bias front, delta, damage, four wheels with
tyre temperatures °C, carcass, pressure kPa, wear, brake temperature, flat, compound),
`grid` (per car: id, driver name as in the game, car, class, place, laps, best, last, sectors, gaps, pits, stops,
virtual energy, front and rear tyre compound names `tyreF`/`tyreR`; since v0.1.24 also per wheel, FL FR RL RR,
`tyreWear` (tyre life 0–1) and `tyreCI` (the compound's index in that car's tyre list); since v0.1.26 `pens`, the
penalties still to serve, oldest first, in LMU's words from its results stream ("Drive Thru", "Stop/Go", …; "Penalty" when
the type is unknown), always as many as LMU's count `penalties`).
SteamIDs are never sent.

Data source: LMU's own shared memory `LMU_Data`; layout generated from pyLMUSharedMemory (see THIRD-PARTY.md)
and checked field by field in `cargo test`.
