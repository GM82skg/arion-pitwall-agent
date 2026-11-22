//! Car tab alerts (migration 59): short events a coach watching *another* car should know about.
//! Red (2): stopped on track, puncture, rear wing off, heavy damage.  Amber (1): penalty, contact,
//! track limits, pit stop. One message per event, the same event at most every 30 s. Fixed words only.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::lmu::layout::LMUObjectOut;
use crate::protocol;

#[derive(Default)]
pub struct Alerts {
    seen: bool,
    penalties: i16,
    flats: u8,
    detached: bool,
    dents: Vec<u8>,
    invalid: bool,
    in_pits: bool,
    stopped_since: Option<Instant>,
    stopped_sent: bool,
    last: HashMap<&'static str, Instant>,
}

impl Alerts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look at the newest data; returns an alert to send, if any.
    pub fn observe(&mut self, o: &LMUObjectOut) -> Option<(u8, &'static str)> {
        self.observe_at(o, Instant::now())
    }

    pub fn observe_at(&mut self, o: &LMUObjectOut, now: Instant) -> Option<(u8, &'static str)> {
        let (_, v) = protocol::player(o)?;
        let id = v.mID;
        let n = (o.scoring.scoringInfo.mNumVehicles.max(0) as usize).min(o.scoring.vehScoringInfo.len());
        let sc = o.scoring.vehScoringInfo[..n].iter().find(|s| { let x = s.mID; x == id }).copied()?;
        let lv = v.mLocalVel;
        let speed = (lv.x * lv.x + lv.y * lv.y + lv.z * lv.z).sqrt();
        let wheels = v.mWheels;
        let flats = wheels.iter().enumerate().fold(0u8, |a, (i, w)| if w.mFlat != 0 { a | 1 << i } else { a });
        let dents: Vec<u8> = v.mDentSeverity.to_vec();
        let in_pits = sc.mInPits != 0;
        let garage = sc.mInGarageStall != 0;
        let (pen, det, inv) = (sc.mNumPenalties, v.mDetached != 0, v.mLapInvalidated != 0);

        let mut out: Option<(u8, &'static str)> = None;
        let raise = |lvl: u8, w: &'static str, out: &mut Option<(u8, &'static str)>| {
            if out.map(|(l, _)| l >= lvl).unwrap_or(false) {
                return; // keep the more urgent one from this same moment
            }
            *out = Some((lvl, w));
        };
        if self.seen {
            if flats & !self.flats != 0 { raise(2, "puncture", &mut out) }
            if det && !self.detached { raise(2, "wing off", &mut out) }
            let heavier = dents.iter().zip(&self.dents).any(|(a, b)| a > b);
            if heavier {
                if dents.iter().any(|&d| d >= 2) && !self.dents.iter().any(|&d| d >= 2) { raise(2, "damage", &mut out) } else { raise(1, "contact", &mut out) }
            }
            if pen > self.penalties { raise(1, "penalty", &mut out) }
            if inv && !self.invalid { raise(1, "track limits", &mut out) }
            if in_pits && !self.in_pits && !garage { raise(1, "pit stop", &mut out) }
        }
        // stopped on track: below 2 m/s for 5 s, not in the pit lane or garage
        if speed < 2.0 && !in_pits && !garage {
            let since = *self.stopped_since.get_or_insert(now);
            if !self.stopped_sent && now.duration_since(since) >= Duration::from_secs(5) {
                self.stopped_sent = true;
                raise(2, "stopped", &mut out);
            }
        } else {
            self.stopped_since = None;
            self.stopped_sent = false;
        }
        self.seen = true;
        self.penalties = pen;
        self.flats = flats;
        self.detached = det;
        self.dents = dents;
        self.invalid = inv;
        self.in_pits = in_pits;

        let (lvl, w) = out?;
        if self.last.get(w).map(|t| now.duration_since(*t) < Duration::from_secs(30)).unwrap_or(false) {
            return None;
        }
        self.last.insert(w, now);
        Some((lvl, w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FakeSource, Source};
    #[test]
    fn events_become_alerts() {
        let mut f = FakeSource::new();
        let mut a = Alerts::new();
        let t0 = Instant::now();
        let base = f.read().unwrap();
        assert_eq!(a.observe_at(&base, t0), None, "first look: nothing to report");
        let mut o = f.read().unwrap();
        o.telemetry.telemInfo[3].mWheels[1].mFlat = 1;
        assert_eq!(a.observe_at(&o, t0 + Duration::from_secs(1)), Some((2, "puncture")));
        assert_eq!(a.observe_at(&o, t0 + Duration::from_secs(2)), None, "same state: no repeat");
        let mut o = f.read().unwrap();
        o.scoring.vehScoringInfo[3].mNumPenalties = 1;
        assert_eq!(a.observe_at(&o, t0 + Duration::from_secs(3)), Some((1, "penalty")));
        let mut o2 = f.read().unwrap();
        o2.scoring.vehScoringInfo[3].mNumPenalties = 1;
        o2.telemetry.telemInfo[3].mDentSeverity[0] = 1;
        assert_eq!(a.observe_at(&o2, t0 + Duration::from_secs(4)), Some((1, "contact")));
        o2.telemetry.telemInfo[3].mDentSeverity[0] = 2;
        assert_eq!(a.observe_at(&o2, t0 + Duration::from_secs(5)), Some((2, "damage")));
    }
    #[test]
    fn stopped_after_five_seconds_once() {
        let mut f = FakeSource::new();
        let mut a = Alerts::new();
        let t0 = Instant::now();
        let mut o = f.read().unwrap();
        o.telemetry.telemInfo[3].mLocalVel.z = 0.0;
        assert_eq!(a.observe_at(&o, t0), None);
        assert_eq!(a.observe_at(&o, t0 + Duration::from_secs(4)), None);
        assert_eq!(a.observe_at(&o, t0 + Duration::from_secs(6)), Some((2, "stopped")));
        assert_eq!(a.observe_at(&o, t0 + Duration::from_secs(9)), None, "once per stop");
        o.scoring.vehScoringInfo[3].mInPits = 1;
        assert_eq!(a.observe_at(&o, t0 + Duration::from_secs(10)), Some((1, "pit stop")));
    }
}
