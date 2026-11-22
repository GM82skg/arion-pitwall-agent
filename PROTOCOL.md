# ARION Pit Wall — what the driver app sends (protocol version 1)

All numbers are little-endian. Two WebRTC data channels, both opened by the coach's browser:

| Channel | Settings | Carries |
|---|---|---|
| `fast` | `ordered: false, maxRetransmits: 0` | fast frames (kind 1, up to 30 Hz) and cars frames (kind 2, 10 Hz), binary |
| `slow` | reliable, ordered | slow message (JSON text, 2 Hz); latency ping `p:<number>` → reply `q:<same number>` |

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

## Slow message (JSON, `"t": "slow"`)

`session` (track, type, time, phase, flags, air/track temperature, rain, wetness min/avg/max, grip, clouds),
`car` (fuel, capacity, virtual energy, battery, TC, ABS, map, brake bias front, delta, damage, four wheels with
tyre temperatures °C, carcass, pressure kPa, wear, brake temperature, flat, compound),
`grid` (per car: id, driver name as in the game, car, class, place, laps, best, last, sectors, gaps, pits, stops).
SteamIDs are never sent.

Data source: LMU's own shared memory `LMU_Data`; layout generated from pyLMUSharedMemory (see THIRD-PARTY.md)
and checked field by field in `cargo test`.
