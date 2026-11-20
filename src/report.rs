//! Phase 1 test report: measures what we couldn't find documented, while a driver races.
//!  1. how often LMU updates our car and the other cars, and how much of the other cars' data is filled online
//!  2. whether per-wheel brake pressure moves while ABS works and the pedal is held steady
//!  3. whether the LMU web interface answers during a race, and how fast
//!  4. this app's own CPU use
//! Written as a plain text file the driver can send. No names, no SteamIDs.

use std::collections::HashSet;
use std::time::Instant;

use crate::lmu::layout::LMUObjectOut;
use crate::protocol;
use crate::rest::Probe;

#[derive(Default)]
pub struct Report {
    started: Option<Instant>,
    sec_start: Option<Instant>,
    // per-second distinct values
    tel_et: HashSet<u64>,
    sco_et: HashSet<u64>,
    remote_pos: HashSet<u64>,
    pub tel_hz: Vec<usize>,
    pub sco_hz: Vec<usize>,
    pub remote_hz: Vec<usize>,
    // remote cars' telemetry slots
    remote_samples: u64,
    remote_inputs: u64,
    remote_temps: u64,
    remote_wheels_rot: u64,
    // ABS
    last_brake: f64,
    last_press: f64,
    abs_steady: u64,
    abs_press_moving: u64,
    pub rest: Vec<Vec<Probe>>,
    pub track: String,
    pub game_version: i32,
    pub frames_sent: u64,
    pub max_viewers: usize,
}

fn h(x: f64) -> u64 {
    x.to_bits()
}

impl Report {
    pub fn observe(&mut self, o: &LMUObjectOut) {
        let now = Instant::now();
        self.started.get_or_insert(now);
        let s0 = *self.sec_start.get_or_insert(now);
        if now.duration_since(s0).as_secs_f64() >= 1.0 {
            if !self.tel_et.is_empty() {
                self.tel_hz.push(self.tel_et.len());
                self.sco_hz.push(self.sco_et.len());
                self.remote_hz.push(self.remote_pos.len());
            }
            self.tel_et.clear();
            self.sco_et.clear();
            self.remote_pos.clear();
            self.sec_start = Some(now);
        }
        self.game_version = o.generic.gameVersion;
        let Some((pi, v)) = protocol::player(o) else { return };
        self.track = protocol::cstr(&{ o.scoring.scoringInfo.mTrackName });
        self.tel_et.insert(h(v.mElapsedTime));
        self.sco_et.insert(h(o.scoring.scoringInfo.mCurrentET));
        // other cars: first active slot that isn't ours
        let n = (o.telemetry.activeVehicles as usize).min(o.telemetry.telemInfo.len());
        let mut first_remote = true;
        for (i, t) in o.telemetry.telemInfo[..n].iter().enumerate() {
            if i == pi {
                continue;
            }
            self.remote_samples += 1;
            if t.mUnfilteredThrottle != 0.0 || t.mUnfilteredBrake != 0.0 {
                self.remote_inputs += 1;
            }
            let w = t.mWheels;
            if w.iter().any(|w| { let tt = w.mTemperature; tt[1] > 1.0 }) {
                self.remote_temps += 1;
            }
            if w.iter().any(|w| { let r = w.mRotation; r != 0.0 }) {
                self.remote_wheels_rot += 1;
            }
            if first_remote {
                first_remote = false;
                let p = t.mPos;
                self.remote_pos.insert(h(p.x) ^ h(p.z).rotate_left(17));
            }
        }
        // ABS: pedal steady (within 2%) while ABS works — does front-left pressure still move?
        let brake = v.mUnfilteredBrake;
        let press = v.mWheels[0].mBrakePressure;
        if v.mABSActive != 0 && brake > 0.3 && (brake - self.last_brake).abs() < 0.02 {
            self.abs_steady += 1;
            if (press - self.last_press).abs() > 0.01 {
                self.abs_press_moving += 1;
            }
        }
        self.last_brake = brake;
        self.last_press = press;
    }

    fn stat(v: &[usize]) -> String {
        if v.is_empty() {
            return "no data".into();
        }
        let mut s = v.to_vec();
        s.sort_unstable();
        format!("median {} per second (lowest {}, highest {}) over {} s", s[s.len() / 2], s[0], s[s.len() - 1], s.len())
    }
    fn pct(a: u64, b: u64) -> String {
        if b == 0 { "no data".into() } else { format!("{:.0}% of samples", a as f64 * 100.0 / b as f64) }
    }

    pub fn text(&self, version: &str, cpu: Option<f64>) -> String {
        let mins = self.started.map(|s| s.elapsed().as_secs_f64() / 60.0).unwrap_or(0.0);
        let mut t = format!("ARION Pit Wall test report\nApp version {version} · LMU version {} · track {} · {mins:.0} min recorded\n\n", self.game_version, if self.track.is_empty() { "-" } else { &self.track });
        t += "1. Update rates\n";
        t += &format!("   Our car (telemetry):   {}\n", Self::stat(&self.tel_hz));
        t += &format!("   Timing (scoring):      {}\n", Self::stat(&self.sco_hz));
        t += &format!("   Another car's position:{}\n", Self::stat(&self.remote_hz));
        t += "   Other cars' telemetry slots:\n";
        t += &format!("     pedals filled:        {}\n", Self::pct(self.remote_inputs, self.remote_samples));
        t += &format!("     tyre temps filled:    {}\n", Self::pct(self.remote_temps, self.remote_samples));
        t += &format!("     wheel speeds filled:  {}\n", Self::pct(self.remote_wheels_rot, self.remote_samples));
        t += "\n2. ABS and brake pressure\n";
        t += &format!("   Samples with ABS working and the pedal steady: {}\n", self.abs_steady);
        t += &format!("   ... where front-left pressure still moved:      {}\n", Self::pct(self.abs_press_moving, self.abs_steady));
        t += "\n3. LMU web interface (localhost:6397)\n";
        match self.rest.last() {
            None => t += "   not checked yet\n",
            Some(last) => {
                for p in last {
                    let okn = self.rest.iter().filter(|r| r.iter().any(|x| x.endpoint == p.endpoint && x.ok)).count();
                    t += &format!("   {:42} {} · {} ms · {} bytes · answered {}/{} checks\n", p.endpoint, if p.ok { "OK" } else { "no answer" }, p.ms, p.bytes, okn, self.rest.len());
                }
            }
        }
        t += "\n4. This app\n";
        t += &format!("   Fast frames sent: {} · most coaches at once: {}\n", self.frames_sent, self.max_viewers);
        if let Some(c) = cpu {
            t += &format!("   CPU: {c:.2}% of one core on average\n");
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
        assert!(r.tel_hz.len() >= 2, "{:?}", r.tel_hz);
        assert!(r.tel_hz.iter().all(|&x| x > 50), "{:?}", r.tel_hz);
        let txt = r.text("0.1.1", Some(0.5));
        assert!(txt.contains("Test Ring") && txt.contains("median"), "{txt}");
        assert!(!txt.to_lowercase().contains("steam"));
    }
}
