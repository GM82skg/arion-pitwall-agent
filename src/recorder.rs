//! Lap recording on the driver's PC, at LMU's own rate (~50–65 Hz), for lap comparisons.
//!  · every lap of the driver's own car
//!  · the best lap so far of every other car in the same class (overwritten when it improves)
//! Files stay on this PC (…/ARION Pit Wall/laps/<session>/). Nothing is uploaded here.
//!
//! File: "ARIONLAP1\n", one JSON line (track, session, car, class, lap, lap time, sample layout), then samples:
//! 44 bytes each, little-endian: et f64 · x,y,z f32 · speed f32 · throttle, brake, steering f32 · rpm f32 ·
//! gear i8 · flags u16 (bit0 ABS, bit1 TC, bits4–7 wheel lock) · pad u8.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::json;

use crate::lmu::layout::{LMUObjectOut, LMUVehicleTelemetry};
use crate::protocol;

pub const SAMPLE_LEN: usize = 44;

#[derive(Default)]
struct Car {
    lap: i16,
    last_et: f64,
    buf: Vec<u8>,
    best: f64,
}

pub struct Recorder {
    root: PathBuf,
    dir: Option<PathBuf>,
    key: (String, i32),
    cars: HashMap<i32, Car>,
    pub saved: u64,
}

fn sample(t: &LMUVehicleTelemetry) -> [u8; SAMPLE_LEN] {
    let lv = t.mLocalVel;
    let p = t.mPos;
    let speed = (lv.x * lv.x + lv.y * lv.y + lv.z * lv.z).sqrt();
    let mut flags: u16 = 0;
    if t.mABSActive != 0 { flags |= 1 }
    if t.mTCActive != 0 { flags |= 2 }
    flags |= (protocol::wheel_locks(t) as u16) << 4;
    let mut b = [0u8; SAMPLE_LEN];
    let mut w = 0;
    let mut put = |x: &[u8]| { b[w..w + x.len()].copy_from_slice(x); w += x.len(); };
    put(&{ t.mElapsedTime }.to_le_bytes());
    for x in [p.x, p.y, p.z, speed, t.mUnfilteredThrottle, t.mUnfilteredBrake, t.mUnfilteredSteering, t.mEngineRPM] {
        put(&(x as f32).to_le_bytes());
    }
    put(&[(t.mGear.clamp(-1, 15) as i8) as u8]);
    put(&flags.to_le_bytes());
    put(&[0]);
    b
}

fn safe(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>().trim_matches('-').chars().take(40).collect()
}

impl Recorder {
    pub fn new(root: PathBuf) -> Self {
        Recorder { root, dir: None, key: (String::new(), -1), cars: HashMap::new(), saved: 0 }
    }

    pub fn observe(&mut self, o: &LMUObjectOut) {
        let Some((pi, me)) = protocol::player(o) else { return };
        let si = o.scoring.scoringInfo;
        let track = protocol::cstr(&{ si.mTrackName });
        let key = (track.clone(), si.mSession);
        if key != self.key {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            self.dir = Some(self.root.join(format!("{stamp}_{}_s{}", safe(&track), key.1)));
            self.key = key;
            self.cars.clear();
        }
        let n = (si.mNumVehicles.max(0) as usize).min(o.scoring.vehScoringInfo.len());
        let slots = protocol::telemetry_by_id(o);
        let my_id = me.mID;
        let my_class = o.scoring.vehScoringInfo[..n].iter().find(|v| { let id = v.mID; id == my_id }).map(|v| protocol::cstr(&{ v.mVehicleClass }));
        for sv in &o.scoring.vehScoringInfo[..n] {
            let id = sv.mID;
            let own = id == my_id;
            let class = protocol::cstr(&{ sv.mVehicleClass });
            if !own && Some(&class) != my_class.as_ref() {
                continue; // only the driver's class
            }
            let Some(&i) = slots.get(&id) else { continue };
            let t = if i == pi { me } else { o.telemetry.telemInfo[i] };
            let laps = sv.mTotalLaps;
            let car = self.cars.entry(id).or_insert_with(|| Car { lap: laps, last_et: f64::NAN, buf: Vec::new(), best: f64::INFINITY });
            if laps != car.lap {
                // a lap was completed: keep it if it's ours, or if it's this car's best so far
                let lap_time = sv.mLastLapTime;
                // LMU: mCountLapFlag 2 = the lap and its time count (not an invalid or pit lap)
                let keep_best = !own && sv.mCountLapFlag == 2 && lap_time > 0.0 && lap_time < car.best;
                if car.buf.len() >= SAMPLE_LEN * 50 && lap_time > 0.0 && (own || keep_best) {
                    if let Some(dir) = &self.dir {
                        let name = if own { format!("own_lap{:03}.lap", car.lap + 1) } else { format!("best_car{id}.lap") };
                        let head = json!({ "track": self.key.0, "session": self.key.1, "car": id, "name": protocol::cstr(&{ sv.mDriverName }),
                            "vehicle": protocol::cstr(&{ sv.mVehicleName }), "class": class, "lap": car.lap + 1, "lapTime": lap_time,
                            "own": own, "sample": "et f64, x y z speed throttle brake steering rpm f32, gear i8, flags u16, pad u8" });
                        let mut f = b"ARIONLAP1\n".to_vec();
                        f.extend_from_slice(head.to_string().as_bytes());
                        f.push(b'\n');
                        f.extend_from_slice(&car.buf);
                        if std::fs::create_dir_all(dir).and_then(|_| std::fs::write(dir.join(&name), &f)).is_ok() {
                            self.saved += 1;
                        }
                    }
                }
                if keep_best {
                    car.best = lap_time;
                }
                car.lap = laps;
                car.buf.clear();
            }
            let et = t.mElapsedTime;
            if et != car.last_et {
                car.last_et = et;
                if car.buf.len() < SAMPLE_LEN * 60 * 600 {
                    car.buf.extend_from_slice(&sample(&t)); // cap: 10 minutes at 60 Hz per lap
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FakeSource, Source};
    #[test]
    fn records_own_laps_and_class_bests() {
        let dir = std::env::temp_dir().join(format!("arion-rec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut r = Recorder::new(dir.clone());
        let mut f = FakeSource::new();
        let mut et = 0.0;
        // drive 3 simulated laps: bump laps every 300 samples
        for k in 0..900 {
            let mut o = f.read().unwrap();
            et += 0.02;
            let lap = (k / 300) as i16;
            for i in 0..8 {
                o.scoring.vehScoringInfo[i].mTotalLaps = lap;
                o.scoring.vehScoringInfo[i].mLastLapTime = 100.0 - lap as f64 - i as f64 * 0.1;
                o.telemetry.telemInfo[i].mElapsedTime = et;
            }
            r.observe(&o);
        }
        let sess: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(sess.len(), 1);
        let files: Vec<String> = std::fs::read_dir(sess[0].as_ref().unwrap().path()).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        assert!(files.contains(&"own_lap001.lap".to_string()) && files.contains(&"own_lap002.lap".to_string()), "{files:?}");
        // GT3 cars besides us (ids 5..8); Hypercars (ids 1..3) are another class and not recorded
        assert!(files.iter().any(|f| f == "best_car5.lap") && !files.iter().any(|f| f == "best_car1.lap"), "{files:?}");
        let b = std::fs::read(sess[0].as_ref().unwrap().path().join("own_lap001.lap")).unwrap();
        assert!(b.starts_with(b"ARIONLAP1\n"));
        let nl = b[10..].iter().position(|&c| c == b'\n').unwrap() + 11;
        assert_eq!((b.len() - nl) % SAMPLE_LEN, 0);
        assert!((b.len() - nl) / SAMPLE_LEN >= 290, "about 300 samples per lap");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
