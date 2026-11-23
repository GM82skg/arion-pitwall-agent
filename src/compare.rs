//! Live comparison against the class best (Phase 3a), computed in the driver app at full rate.
//!  · Reference: the fastest valid lap of any car in the driver's class this session (rivals or the driver),
//!    recorded from LMU's telemetry (position, speed, pedals) with its exact lap start (mLapStartET).
//!  · Corners: found automatically on the reference — each slows-down-to-a-minimum-and-back is one corner.
//!  · Live: the driver's position is projected onto the reference line → live delta, ghost car position,
//!    and after each corner a short verdict (time won/lost, braking point, minimum and exit speed).

use std::collections::{HashMap, VecDeque};

use serde_json::{json, Value};

use crate::lmu::layout::LMUObjectOut;
use crate::protocol;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pt { pub t: f32, pub s: f32, pub x: f32, pub z: f32, pub v: f32, pub brk: f32 }

#[derive(Clone, Debug)]
pub struct Corner { pub n: usize, pub s_brake: f32, pub s_apex: f32, pub s_end: f32, pub v_min: f32, pub v_end: f32 }

#[derive(Clone, Debug)]
pub struct RefLap { pub id: i32, pub name: String, pub own: bool, pub lap: f64, pub pts: Vec<Pt>, pub corners: Vec<Corner> }

#[derive(Default)]
struct Buf { lap: i32, start: f64, last_et: f64, pts: Vec<(f32, f32, f32, f32, f32)> } // t, x, z, v, brake

#[derive(Default)]
struct Live { lap: i32, hint: usize, own_hint: usize, pts: Vec<(f32, f32, f32, bool, bool, f32)>, done: usize, delta: f32, delta_own: Option<f32>, ghost: (f32, f32), s: f32, ok: bool }

#[derive(Default)]
pub struct Compare {
    bufs: HashMap<i32, Buf>,
    pub reference: Option<RefLap>,
    /// the driver's own fastest valid lap (the default delta on the dashboard)
    pub own: Option<RefLap>,
    pub rev: u16,
    live: Live,
    pub recent: VecDeque<Value>,
    class: String,
}

fn interp(a: f32, b: f32, f: f32) -> f32 { a + (b - a) * f }

/// Project a position onto a reference line, searching near the last known point (the first quarter at a lap start).
/// Returns (new hint, distance along the line, metres off the line).
fn project(p: &[Pt], hint: usize, x: f32, z: f32) -> (usize, f32, f32) {
    let (lo, hi) = if hint == 0 { (0, (p.len() / 4).max(2)) } else { (hint.saturating_sub(40), (hint + 400).min(p.len() - 1)) };
    let mut best = (f32::MAX, lo);
    for k in lo..hi.min(p.len() - 1) { let d = (p[k].x - x).powi(2) + (p[k].z - z).powi(2); if d < best.0 { best = (d, k) } }
    let k = best.1;
    let (a, b) = (p[k], p[k + 1]);
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    let l2 = dx * dx + dz * dz;
    let f = if l2 > 0.0 { (((x - a.x) * dx + (z - a.z) * dz) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (k.max(1), interp(a.s, b.s, f), best.0.sqrt())
}

impl RefLap {
    /// time at distance s along the reference (s ascending)
    pub fn t_at_s(&self, s: f32) -> f32 {
        let p = &self.pts;
        let i = p.partition_point(|q| q.s < s).clamp(1, p.len() - 1);
        let (a, b) = (p[i - 1], p[i]);
        let f = if b.s > a.s { ((s - a.s) / (b.s - a.s)).clamp(0.0, 1.0) } else { 0.0 };
        interp(a.t, b.t, f)
    }
    /// reference position at time t into the lap (the ghost)
    pub fn pos_at_t(&self, t: f32) -> (f32, f32) {
        let p = &self.pts;
        let i = p.partition_point(|q| q.t < t).clamp(1, p.len() - 1);
        let (a, b) = (p[i - 1], p[i]);
        let f = if b.t > a.t { ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0) } else { 0.0 };
        (interp(a.x, b.x, f), interp(a.z, b.z, f))
    }
    pub fn len(&self) -> f32 { self.pts.last().map(|p| p.s).unwrap_or(0.0) }
}

/// Speed on a 2 m grid (smoothed), for finding corners.
fn grid(p: &[Pt]) -> (Vec<f32>, Vec<f32>) {
    let len = p.last().map(|q| q.s).unwrap_or(0.0);
    let n = (len / 2.0) as usize;
    let mut s = Vec::with_capacity(n);
    let mut v = Vec::with_capacity(n);
    let mut j = 1;
    for k in 0..n {
        let x = k as f32 * 2.0;
        while j < p.len() - 1 && p[j].s < x { j += 1 }
        let (a, b) = (p[j - 1], p[j]);
        let f = if b.s > a.s { ((x - a.s) / (b.s - a.s)).clamp(0.0, 1.0) } else { 0.0 };
        s.push(x);
        v.push(interp(a.v, b.v, f));
    }
    let w = 5; // ±10 m
    let sm: Vec<f32> = (0..v.len()).map(|i| { let a = i.saturating_sub(w); let b = (i + w + 1).min(v.len()); v[a..b].iter().sum::<f32>() / (b - a) as f32 }).collect();
    (s, sm)
}

/// Corners: a speed minimum at least 10 % below the fastest point of the 300 m before it, 120 m apart.
pub fn find_corners(p: &[Pt]) -> Vec<Corner> {
    if p.len() < 50 { return vec![] }
    let (s, v) = grid(p);
    let n = v.len();
    let mut out: Vec<Corner> = vec![];
    for i in 30..n.saturating_sub(30) {
        let lo = i.saturating_sub(30); let hi = (i + 31).min(n);
        if v[lo..hi].iter().any(|&x| x < v[i]) { continue }
        let before = v[i.saturating_sub(150)..i].iter().cloned().fold(0.0, f32::max);
        if v[i] > 0.9 * before { continue }
        if out.last().map(|c| s[i] - c.s_apex < 120.0).unwrap_or(false) { continue }
        let mut b = i; while b > 0 && v[b - 1] >= v[b] - 0.02 { b -= 1 }      // back to where slowing down began
        let mut e = i; while e + 1 < n && v[e + 1] >= v[e] - 0.02 && v[e] < 0.97 * before { e += 1 } // out to where it's back up
        out.push(Corner { n: out.len() + 1, s_brake: s[b], s_apex: s[i], s_end: s[e], v_min: v[i], v_end: v[e] });
    }
    out
}

impl Compare {
    pub fn new() -> Self { Self::default() }

    /// Feed the newest data. Returns corner verdicts that just completed.
    pub fn observe(&mut self, o: &LMUObjectOut) -> Vec<Value> {
        let Some((pi, me)) = protocol::player(o) else { return vec![] };
        let si = o.scoring.scoringInfo;
        let n = (si.mNumVehicles.max(0) as usize).min(o.scoring.vehScoringInfo.len());
        let my_id = me.mID;
        let sc: Vec<_> = o.scoring.vehScoringInfo[..n].to_vec();
        let Some(mine) = sc.iter().find(|v| { let x = v.mID; x == my_id }) else { return vec![] };
        let class = protocol::cstr(&{ mine.mVehicleClass });
        if class != self.class { self.class = class.clone(); self.bufs.clear(); self.reference = None; self.own = None; self.rev = self.rev.wrapping_add(1) }
        let slots = protocol::telemetry_by_id(o);
        // record every car of the class; a finished valid lap faster than the reference becomes the reference
        for v in sc.iter().filter(|v| protocol::cstr(&{ v.mVehicleClass }) == class) {
            let id = v.mID;
            let Some(&i) = slots.get(&id) else { continue };
            let t = if i == pi { me } else { o.telemetry.telemInfo[i] };
            let lap = t.mLapNumber;
            let et = t.mElapsedTime;
            let b = self.bufs.entry(id).or_insert_with(|| Buf { lap, start: t.mLapStartET, last_et: -1.0, pts: vec![] });
            if lap != b.lap {
                let last = v.mLastLapTime;
                let full = b.pts.first().map(|p| p.0 < 1.0).unwrap_or(false) && b.pts.len() > 50;
                let sane = b.pts.last().map(|p| (p.0 as f64 - last).abs() < 1.5).unwrap_or(false);
                let good = full && sane && v.mCountLapFlag == 2 && last > 0.0;
                let class_best = good && self.reference.as_ref().map(|r| last < r.lap - 1e-3).unwrap_or(true);
                let own_best = good && id == my_id && self.own.as_ref().map(|r| last < r.lap - 1e-3).unwrap_or(true);
                if class_best || own_best {
                    let mut pts: Vec<Pt> = Vec::with_capacity(b.pts.len());
                    let mut s = 0.0f32;
                    for (k, p) in b.pts.iter().enumerate() {
                        if k > 0 { let q = b.pts[k - 1]; s += ((p.1 - q.1).powi(2) + (p.2 - q.2).powi(2)).sqrt() }
                        pts.push(Pt { t: p.0, s, x: p.1, z: p.2, v: p.3, brk: p.4 });
                    }
                    let corners = find_corners(&pts);
                    let r = RefLap { id, name: protocol::cstr(&{ v.mDriverName }), own: id == my_id, lap: last, pts, corners };
                    if own_best { self.own = Some(r.clone()); self.live.own_hint = usize::MAX } // used from the next lap start
                    if class_best {
                        self.reference = Some(r);
                        self.live.done = usize::MAX; // don't judge the lap in progress against a reference that changed mid-lap
                    }
                    self.rev = self.rev.wrapping_add(1);
                }
                *b = Buf { lap, start: t.mLapStartET, last_et: -1.0, pts: vec![] };
            }
            if et != b.last_et {
                b.last_et = et;
                let lv = t.mLocalVel;
                let spd = (lv.x * lv.x + lv.y * lv.y + lv.z * lv.z).sqrt() as f32;
                if b.pts.len() < 60 * 900 { b.pts.push(((et - b.start) as f32, t.mPos.x as f32, t.mPos.z as f32, spd, t.mUnfilteredBrake as f32)) }
            }
        }
        self.live_step(&me)
    }

    fn live_step(&mut self, me: &crate::lmu::layout::LMUVehicleTelemetry) -> Vec<Value> {
        let Some(r) = self.reference.as_ref() else { self.live.ok = false; return vec![] };
        let lap = me.mLapNumber;
        let t_live = (me.mElapsedTime - me.mLapStartET) as f32;
        let (x, z) = (me.mPos.x as f32, me.mPos.z as f32);
        let lv = me.mLocalVel;
        let spd = (lv.x * lv.x + lv.y * lv.y + lv.z * lv.z).sqrt() as f32;
        let mut out = vec![];
        if lap != self.live.lap {
            self.live = Live { lap, ..Default::default() };
        }
        // project onto the class-best line near the last known point (the whole line once per lap start)
        let (hint, s, off) = project(&r.pts, self.live.hint, x, z);
        self.live.hint = hint;
        self.live.ok = off <= 40.0 && t_live >= 0.0;
        // and onto the driver's own best lap, for the default delta
        self.live.delta_own = match self.own.as_ref() {
            Some(o) if t_live >= 0.0 && self.live.own_hint != usize::MAX => { let (h, so, off_o) = project(&o.pts, self.live.own_hint, x, z); self.live.own_hint = h; if off_o <= 40.0 { Some(t_live - o.t_at_s(so)) } else { None } }
            _ => None,
        };
        if self.live.ok {
            self.live.s = s;
            self.live.delta = t_live - r.t_at_s(s);
            self.live.ghost = r.pos_at_t(t_live);
            let locks = crate::protocol::wheel_locks(me) != 0;
            self.live.pts.push((s, t_live, spd, me.mABSActive != 0, locks, me.mUnfilteredBrake as f32));
            // corner verdicts: once the car is 5 m past a corner's end
            while self.live.done < r.corners.len() && s > r.corners[self.live.done].s_end + 5.0 {
                let c = &r.corners[self.live.done];
                if let Some(v) = self.judge(c, r) { self.recent.push_front(v.clone()); if self.recent.len() > 12 { self.recent.pop_back(); } out.push(v) }
                self.live.done += 1;
            }
        }
        out
    }

    /// One corner, this lap against the reference: time, braking point, minimum speed, exit speed.
    fn judge(&self, c: &Corner, r: &RefLap) -> Option<Value> {
        let lp = &self.live.pts;
        let s0 = (c.s_brake - 50.0).max(0.0);
        let at = |s: f32| -> Option<f32> { let i = lp.partition_point(|q| q.0 < s); if i == 0 || i >= lp.len() { return None } let (a, b) = (lp[i - 1], lp[i]); let f = if b.0 > a.0 { (s - a.0) / (b.0 - a.0) } else { 0.0 }; Some(interp(a.1, b.1, f)) };
        let (t0, t1) = (at(s0)?, at(c.s_end)?);
        let dt = (t1 - t0) - (r.t_at_s(c.s_end) - r.t_at_s(s0));
        let seg: Vec<_> = lp.iter().filter(|q| q.0 >= s0 && q.0 <= c.s_end).collect();
        if seg.len() < 5 { return None }
        let (imin, vmin) = seg.iter().enumerate().fold((0, f32::MAX), |m, (i, q)| if q.2 < m.1 { (i, q.2) } else { m });
        // braking point = where continuous braking (pedal over 10 %) began before the slowest point, for both cars.
        // (Walking back on speed alone ran through the end of the straight, where speed barely changes per sample.)
        // A corner taken with a lift only has no braking point; then the verdict leaves braking out.
        let apex_s = seg[imin].0;
        let li = lp.partition_point(|q| q.0 < apex_s).min(lp.len() - 1);
        let mut b = li; while b > 0 && lp[b - 1].5 > 0.1 && lp[b - 1].0 > c.s_brake - 300.0 { b -= 1 }
        let live_brake = if lp[b].5 > 0.1 || (b < li && lp[b + 1].5 > 0.1) { Some(lp[b].0) } else { None };
        let rp = &r.pts;
        let rmin = { let lo = rp.partition_point(|q| q.s < s0); let hi = rp.partition_point(|q| q.s <= c.s_end).max(lo + 1).min(rp.len());
            (lo..hi).fold((lo, f32::MAX), |m, k| if rp[k].v < m.1 { (k, rp[k].v) } else { m }).0 };
        let mut rb = rmin; while rb > 0 && rp[rb - 1].brk > 0.1 && rp[rb - 1].s > c.s_brake - 300.0 { rb -= 1 }
        let ref_brake = if rp[rb].brk > 0.1 || (rb < rmin && rp[rb + 1].brk > 0.1) { Some(rp[rb].s) } else { None };
        let brake_m = match (live_brake, ref_brake) { (Some(a), Some(b)) => Some((a - b).round()), _ => None };
        let vexit = seg.last().map(|q| q.2).unwrap_or(0.0);
        Some(json!({ "t": "corner", "n": c.n, "lap": self.live.lap, "dt": (dt * 1000.0).round() / 1000.0,
            "brake_m": brake_m, "vmin": ((vmin - c.v_min) * 3.6).round(), "vexit": ((vexit - c.v_end) * 3.6).round(),
            "abs": seg.iter().any(|q| q.3), "lock": seg.iter().any(|q| q.4), "rev": self.rev }))
    }

    /// Fast frame, kind 3 (24 bytes): version, kind, rev u16, live delta to the class best f32, ghost x f32, ghost z f32,
    /// distance f32, live delta to the driver's own best f32 (NaN while there is none).
    pub fn frame(&self) -> Option<[u8; 24]> {
        if self.reference.is_none() || !self.live.ok { return None }
        let mut b = [0u8; 24];
        b[0] = protocol::PROTOCOL_VERSION; b[1] = 3;
        b[2..4].copy_from_slice(&self.rev.to_le_bytes());
        b[4..8].copy_from_slice(&self.live.delta.to_le_bytes());
        b[8..12].copy_from_slice(&self.live.ghost.0.to_le_bytes());
        b[12..16].copy_from_slice(&self.live.ghost.1.to_le_bytes());
        b[16..20].copy_from_slice(&self.live.s.to_le_bytes());
        b[20..24].copy_from_slice(&self.live.delta_own.unwrap_or(f32::NAN).to_le_bytes());
        Some(b)
    }

    /// The reference for the dashboard: who, lap time, the line (every ~5 m), corners, recent verdicts.
    pub fn ref_json(&self) -> Option<Value> {
        let r = self.reference.as_ref()?;
        // at most ~1500 points (Le Mans is 13.6 km), so the message stays well under WebRTC's size limit
        let step = (r.len() / 1500.0).max(5.0);
        let mut line = vec![]; let mut last = -step;
        for p in &r.pts { if p.s - last >= step { line.push(json!([p.x.round(), p.z.round(), p.s.round(), (p.v * 3.6).round()])); last = p.s } }
        let at = |s: f32| { let (x, z) = r.pos_at_t(r.t_at_s(s)); json!([(x * 10.0).round() / 10.0, (z * 10.0).round() / 10.0]) };
        Some(json!({ "t": "ref", "rev": self.rev, "car": r.id, "name": r.name, "own": r.own, "lap": r.lap, "own_lap": self.own.as_ref().map(|o| o.lap), "len": r.len().round(),
            "line": line, "corners": r.corners.iter().map(|c| json!({ "n": c.n, "brake": c.s_brake, "apex": c.s_apex, "end": c.s_end,
                "vmin": (c.v_min * 3.6).round(), "at_brake": at(c.s_brake), "at_apex": at(c.s_apex) })).collect::<Vec<_>>(),
            "recent": self.recent.iter().cloned().collect::<Vec<_>>() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::FakeSource;
    #[test]
    fn class_best_reference_corners_and_verdicts() {
        // 4 minutes of racing at LMU's rate (50 samples a second), exactly and repeatably
        let mut f = FakeSource::new();
        let mut c = Compare::new();
        let mut verdicts = vec![];
        let mut deltas = vec![];
        let mut own_deltas = vec![];
        for _ in 0..(240 * 50) {
            let o = f.step(0.02);
            verdicts.extend(c.observe(&o));
            if let Some(fr) = c.frame() {
                deltas.push(f32::from_le_bytes(fr[4..8].try_into().unwrap()));
                let o = f32::from_le_bytes(fr[20..24].try_into().unwrap());
                if o.is_finite() { own_deltas.push(o) }
            }
        }
        let r = c.reference.as_ref().expect("a class-best reference");
        assert_eq!(r.id, 6, "the fastest GT3 is car 6 (it brakes later)");
        assert!(r.lap > 30.0 && r.lap < 50.0, "lap {}", r.lap);
        assert_eq!(r.corners.len(), 2, "the test track has two corners: {:?}", r.corners);
        assert!(r.len() > 1700.0 && r.len() < 1830.0, "reference length {} (the oval is ~1818 m around)", r.len());
        assert!(!deltas.is_empty() && deltas.iter().all(|d| d.abs() < 5.0), "live delta stays sensible");
        assert!(!verdicts.is_empty(), "corner verdicts after each corner");
        let v = &verdicts[verdicts.len() - 1];
        assert!(v["dt"].as_f64().unwrap() > 0.0, "the player is slower than the class best in corners: {v}");
        assert!(v["brake_m"].as_f64().unwrap() < 0.0, "the player brakes earlier than the reference: {v}");
        // car 6 brakes later than every other car (source.rs), so no verdict may say the player braked later
        assert!(verdicts.iter().all(|v| v["brake_m"].as_f64().map(|m| m < 0.0).unwrap_or(true)), "{verdicts:?}");
        assert!(verdicts.iter().filter(|v| v["brake_m"].is_number()).count() * 2 >= verdicts.len(), "most corners have a braking point: {verdicts:?}");
        let own = c.own.as_ref().expect("the player's own best lap");
        assert!(own.own && own.lap >= r.lap - 1e-6, "own best {} can't beat the class best {}", own.lap, r.lap);
        assert!(!own_deltas.is_empty() && own_deltas.iter().all(|d| d.abs() < 3.0), "own delta stays sensible");
        // the player is slower than car 6, so against the class best it's further behind than against itself
        let n = deltas.len().min(own_deltas.len()).min(500);
        let (dc, dow): (f32, f32) = (deltas[deltas.len() - n..].iter().sum::<f32>() / n as f32, own_deltas[own_deltas.len() - n..].iter().sum::<f32>() / n as f32);
        assert!(dc > dow, "class delta {dc} vs own delta {dow}");
        let j = c.ref_json().unwrap();
        assert!((j["own_lap"].as_f64().unwrap() - own.lap).abs() < 1e-9);
        assert!(j["line"].as_array().unwrap().len() > 300 && j["corners"].as_array().unwrap().len() == 2);
        // every verdict of the player: slower in the corners, braking earlier, slower at the apex
        assert!(verdicts.iter().all(|v| v["dt"].as_f64().unwrap() > -0.05), "{verdicts:?}");
    }
}
