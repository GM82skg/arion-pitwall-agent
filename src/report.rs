//! Test report: measures, per session, what we couldn't find documented.
//!  1. how often LMU updates our car, the timing, and the other cars — split into real players (online)
//!     and AI, counting only cars that are moving — and how much of their data is filled
//!  2. whether per-wheel brake pressure moves while ABS works and the pedal is held steady
//!  3. whether the LMU web interface answers, and how fast
//!  4. this app's own CPU use
//! A new section starts when the track or session changes. No names, no SteamIDs, no server names.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::lmu::layout::LMUObjectOut;
use crate::protocol;
use crate::rest::Probe;

/// LMU's mControl: who drives a car.
const REMOTE: i8 = 2;
const AI: i8 = 1;
const MOVING: f64 = 10.0; // m/s

#[derive(Default)]
struct Others {
    pos_this_sec: HashMap<i32, HashSet<u64>>,
    rates: Vec<usize>,
    samples: u64,
    inputs: u64,
    temps: u64,
    wheels: u64,
    cars: HashSet<i32>,
}

struct Session {
    track: String,
    session: i32,
    started: Instant,
    on_track: Instant,
    on_track_secs: f64,
    max_remote: usize,
    tel_et: HashSet<u64>,
    sco_et: HashSet<u64>,
    tel_hz: Vec<usize>,
    sco_hz: Vec<usize>,
    others: HashMap<i8, Others>,
    last_brake: f64,
    last_press: f64,
    abs_steady: u64,
    abs_moving: u64,
    rest: Vec<Vec<Probe>>,
    frames_sent: u64,
    max_viewers: usize,
}

impl Session {
    fn new(track: String, session: i32) -> Self {
        let now = Instant::now();
        Session {
            track, session, started: now, on_track: now, on_track_secs: 0.0, max_remote: 0,
            tel_et: HashSet::new(), sco_et: HashSet::new(), tel_hz: vec![], sco_hz: vec![], others: HashMap::new(),
            last_brake: 0.0, last_press: 0.0, abs_steady: 0, abs_moving: 0, rest: vec![], frames_sent: 0, max_viewers: 0,
        }
    }
    fn flush_second(&mut self) {
        if !self.tel_et.is_empty() {
            self.tel_hz.push(self.tel_et.len());
            self.sco_hz.push(self.sco_et.len());
            self.on_track_secs += 1.0;
        }
        self.tel_et.clear();
        self.sco_et.clear();
        for o in self.others.values_mut() {
            for (_, s) in o.pos_this_sec.drain() {
                if !s.is_empty() {
                    o.rates.push(s.len());
                }
            }
        }
    }
}

#[derive(Default)]
pub struct Report {
    cur: Option<Session>,
    done: Vec<Session>,
    sec_start: Option<Instant>,
    pub game_version: i32,
}

fn h(x: f64) -> u64 {
    x.to_bits()
}

impl Report {
    pub fn observe(&mut self, o: &LMUObjectOut) {
        let now = Instant::now();
        self.game_version = o.generic.gameVersion;
        let Some((pi, v)) = protocol::player(o) else { return };
        let track = protocol::cstr(&{ o.scoring.scoringInfo.mTrackName });
        let sess = o.scoring.scoringInfo.mSession;
        if self.cur.as_ref().map(|c| c.track != track || c.session != sess).unwrap_or(true) {
            if let Some(mut c) = self.cur.take() {
                c.flush_second();
                self.done.push(c);
            }
            self.cur = Some(Session::new(track, sess));
            self.sec_start = Some(now);
        }
        let cur = self.cur.as_mut().unwrap();
        if now.duration_since(self.sec_start.unwrap_or(now)).as_secs_f64() >= 1.0 {
            cur.flush_second();
            self.sec_start = Some(now);
        }
        cur.on_track = now;
        cur.tel_et.insert(h(v.mElapsedTime));
        cur.sco_et.insert(h(o.scoring.scoringInfo.mCurrentET));

        // other cars, by who drives them, moving only
        let n = (o.scoring.scoringInfo.mNumVehicles.max(0) as usize).min(o.scoring.vehScoringInfo.len());
        let slots = protocol::telemetry_by_id(o);
        let mut remote_now = 0usize;
        for sv in &o.scoring.vehScoringInfo[..n] {
            let ctrl = sv.mControl;
            if ctrl == REMOTE {
                remote_now += 1;
            }
            if ctrl != REMOTE && ctrl != AI {
                continue;
            }
            let Some(&i) = slots.get(&{ sv.mID }) else { continue };
            if i == pi {
                continue;
            }
            let t = &o.telemetry.telemInfo[i];
            let lv = t.mLocalVel;
            if (lv.x * lv.x + lv.y * lv.y + lv.z * lv.z).sqrt() < MOVING {
                continue;
            }
            let e = cur.others.entry(ctrl).or_default();
            e.cars.insert(sv.mID);
            e.samples += 1;
            if t.mUnfilteredThrottle != 0.0 || t.mUnfilteredBrake != 0.0 {
                e.inputs += 1;
            }
            let w = t.mWheels;
            if w.iter().any(|w| { let tt = w.mTemperature; tt[1] > 1.0 }) {
                e.temps += 1;
            }
            if w.iter().any(|w| { let r = w.mRotation; r != 0.0 }) {
                e.wheels += 1;
            }
            let p = t.mPos;
            e.pos_this_sec.entry(sv.mID).or_default().insert(h(p.x) ^ h(p.z).rotate_left(17));
        }
        cur.max_remote = cur.max_remote.max(remote_now);

        // ABS: pedal steady (within 2%) while ABS works — does front-left pressure still move?
        let brake = v.mUnfilteredBrake;
        let press = v.mWheels[0].mBrakePressure;
        if v.mABSActive != 0 && brake > 0.3 && (brake - cur.last_brake).abs() < 0.02 {
            cur.abs_steady += 1;
            if (press - cur.last_press).abs() > 0.01 {
                cur.abs_moving += 1;
            }
        }
        cur.last_brake = brake;
        cur.last_press = press;
    }

    pub fn add_rest(&mut self, p: Vec<Probe>) {
        if let Some(c) = self.cur.as_mut() {
            c.rest.push(p);
        }
    }
    pub fn add_frame(&mut self) {
        if let Some(c) = self.cur.as_mut() {
            c.frames_sent += 1;
        }
    }
    pub fn viewers(&mut self, n: usize) {
        if let Some(c) = self.cur.as_mut() {
            c.max_viewers = c.max_viewers.max(n);
        }
    }
    pub fn sessions(&self) -> usize {
        self.done.len() + self.cur.is_some() as usize
    }

    fn stat(v: &[usize]) -> String {
        if v.is_empty() {
            return "no data".into();
        }
        let mut s = v.to_vec();
        s.sort_unstable();
        format!("median {} per second (lowest {}, highest {}) over {} car-seconds", s[s.len() / 2], s[0], s[s.len() - 1], s.len())
    }
    fn pct(a: u64, b: u64) -> String {
        if b == 0 { "no data".into() } else { format!("{:.0}%", a as f64 * 100.0 / b as f64) }
    }

    pub fn text(&self, version: &str, cpu: Option<f64>) -> String {
        let mut t = format!("ARION Pit Wall test report\nApp version {version} · LMU version {}\n", self.game_version);
        if let Some(c) = cpu {
            t += &format!("App CPU: {c:.2}% of one core on average\n");
        }
        if self.sessions() == 0 {
            t += "\nNo session on track yet.\n";
        }
        for (k, s) in self.done.iter().chain(self.cur.iter()).enumerate() {
            let kind = if s.max_remote > 0 { format!("ONLINE, up to {} other players", s.max_remote) } else { "OFFLINE (no other players)".into() };
            t += &format!("\n===== Session {} · {} · session type {} · {} · {:.0} min on track =====\n",
                k + 1, if s.track.is_empty() { "-" } else { &s.track }, s.session, kind, s.on_track_secs / 60.0);
            let _ = (s.started, s.on_track);
            t += "1. Update rates\n";
            t += &format!("   Our car (telemetry):  {}\n", Self::stat(&s.tel_hz).replace("car-seconds", "s"));
            t += &format!("   Timing (scoring):     {}\n", Self::stat(&s.sco_hz).replace("car-seconds", "s"));
            for (ctrl, label) in [(REMOTE, "Real players (online)"), (AI, "AI cars")] {
                match s.others.get(&ctrl) {
                    None => t += &format!("   {label}: none moving\n"),
                    Some(o) => {
                        t += &format!("   {label}, {} cars, moving only:\n", o.cars.len());
                        t += &format!("     position updates:   {}\n", Self::stat(&o.rates));
                        t += &format!("     pedals filled:      {} of samples\n", Self::pct(o.inputs, o.samples));
                        t += &format!("     tyre temps filled:  {} of samples\n", Self::pct(o.temps, o.samples));
                        t += &format!("     wheel speeds filled:{} of samples\n", Self::pct(o.wheels, o.samples));
                    }
                }
            }
            t += "2. ABS and brake pressure\n";
            t += &format!("   Moments with ABS working and the pedal steady: {}\n", s.abs_steady);
            t += &format!("   ... where front-left pressure still moved:      {}\n", Self::pct(s.abs_moving, s.abs_steady));
            t += "3. LMU web interface (localhost:6397)\n";
            match s.rest.last() {
                None => t += "   not checked in this session\n",
                Some(last) => {
                    for p in last {
                        let okn = s.rest.iter().filter(|r| r.iter().any(|x| x.endpoint == p.endpoint && x.ok)).count();
                        t += &format!("   {:42} {} · {} ms · answered {}/{} checks\n", p.endpoint, if p.ok { "OK" } else { "no answer" }, p.ms, okn, s.rest.len());
                    }
                }
            }
            t += &format!("4. Streaming: {} fast frames sent · most coaches at once: {}\n", s.frames_sent, s.max_viewers);
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FakeSource, Source};
    #[test]
    fn report_from_the_fake_car() {
        let mut f = FakeSource::new();
        let mut r = Report::default();
        let t0 = Instant::now();
        while t0.elapsed().as_millis() < 2300 {
            r.observe(&f.read().unwrap());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let txt = r.text("0.1.4", Some(0.5));
        assert!(txt.contains("Session 1 · Test Ring") && txt.contains("ONLINE, up to 4 other players"), "{txt}");
        assert!(txt.contains("Real players (online), 4 cars") && txt.contains("AI cars, 3 cars"), "{txt}");
        assert!(!txt.to_lowercase().contains("steam"));
        // a different session starts a new section
        let mut o = f.read().unwrap();
        o.scoring.scoringInfo.mSession = 1;
        r.observe(&o);
        assert_eq!(r.sessions(), 2);
        assert!(r.text("0.1.4", None).contains("Session 2 · Test Ring · session type 1"));
    }
}
