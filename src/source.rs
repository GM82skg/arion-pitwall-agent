//! Where the game data comes from: LMU's shared memory on Windows, or a fake car for tests and demos.

use crate::lmu::layout::LMUObjectOut;

pub trait Source: Send {
    /// A copy of the whole LMU_Data block, or None when the game isn't running / not on track.
    fn read(&mut self) -> Option<Box<LMUObjectOut>>;
}

pub fn zeroed() -> Box<LMUObjectOut> {
    // Safety: LMUObjectOut is plain numbers and byte arrays; all-zero is a valid value.
    unsafe { Box::<LMUObjectOut>::new_zeroed().assume_init() }
}

fn put(dst: &mut [u8], s: &str) {
    let b = s.as_bytes();
    let n = b.len().min(dst.len() - 1);
    dst[..n].copy_from_slice(&b[..n]);
}

/// A car lapping an oval (about 1.6 km, ~45 s), braking into both ends, with 7 other cars. No game needed.
pub struct FakeSource {
    last: std::time::Instant,
    clock: f64,
    speed: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// demo mode: now and then a penalty, a knock, a pit stop — so alerts and key moments can be seen
    events: bool,
}
impl FakeSource {
    pub fn new() -> Self {
        Self::with_speed(1.0)
    }
    /// A faster clock, so tests see laps, stints and energy use within seconds.
    pub fn with_speed(speed: f64) -> Self {
        FakeSource { last: std::time::Instant::now(), clock: 0.0, speed: std::sync::Arc::new(std::sync::atomic::AtomicU32::new((speed * 100.0) as u32)), events: false }
    }
    /// Demo mode: the speed can be changed while it runs (1× or 10×), and race events happen now and then.
    pub fn demo(speed: std::sync::Arc<std::sync::atomic::AtomicU32>) -> Self {
        FakeSource { last: std::time::Instant::now(), clock: 0.0, speed, events: true }
    }
}
impl Default for FakeSource {
    fn default() -> Self {
        Self::new()
    }
}

impl Source for FakeSource {
    fn read(&mut self) -> Option<Box<LMUObjectOut>> {
        let now = std::time::Instant::now();
        let k = self.speed.load(std::sync::atomic::Ordering::Relaxed) as f64 / 100.0;
        self.clock += now.duration_since(self.last).as_secs_f64() * k;
        self.last = now;
        let et = self.clock;
        let mut o = zeroed();
        let lap_len = 1600.0;
        let si = &mut o.scoring.scoringInfo;
        put(&mut si.mTrackName, "Test Ring");
        si.mSession = 10;
        si.mCurrentET = et;
        si.mEndET = 6.0 * 3600.0;
        si.mSessionTimeRemaining = (6.0 * 3600.0 - et) as f32;
        si.mGamePhase = 5;
        si.mAmbientTemp = 22.8;
        si.mTrackTemp = 31.4;
        si.mLapDist = lap_len;
        si.mNumVehicles = 8;
        si.mTrackGripLevel = 3;
        let pos_at = |d: f64| -> (f64, f64) {
            let a = d / lap_len * std::f64::consts::TAU;
            (400.0 * a.cos(), 150.0 * a.sin())
        };
        for i in 0..8usize {
            let speed = 36.0 - i as f64 * 0.4;
            let d = (et * speed + i as f64 * 140.0) % lap_len;
            let (x, z) = pos_at(d);
            let v = &mut o.scoring.vehScoringInfo[i];
            v.mID = i as i32 + 1;
            put(&mut v.mDriverName, &format!("Driver {}", i + 1));
            put(&mut v.mVehicleName, &format!("Car #{}", 10 + i));
            put(&mut v.mVehicleClass, if i < 3 { "Hypercar" } else { "GT3" });
            v.mPlace = i as u8 + 1;
            v.mLapDist = d;
            v.mPos.x = x;
            v.mPos.z = z;
            v.mTotalLaps = (et * speed / lap_len) as i16;
            v.mBestLapTime = lap_len / speed;
            v.mLastLapTime = lap_len / speed + 0.3;
            v.mIsPlayer = (i == 3) as u8;
            v.mCountLapFlag = 2;
            v.mControl = if i == 3 { 0 } else if i % 2 == 0 { 2 } else { 1 }; // you, remote players, AI
            let lap_t = lap_len / speed + 0.3 + ((v.mTotalLaps as f64 * 1.7 + i as f64).sin() * 0.4);
            v.mLastLapTime = if v.mTotalLaps > 0 { lap_t } else { 0.0 };
            v.mLastSector1 = if v.mTotalLaps > 0 { lap_t * 0.31 } else { 0.0 };
            v.mLastSector2 = if v.mTotalLaps > 0 { lap_t * 0.66 } else { 0.0 };
            v.mBestSector1 = lap_len / speed * 0.31;
            v.mBestSector2 = lap_len / speed * 0.66;
            v.mTimeBehindLeader = i as f64 * 3.5;
            v.mFuelFraction = (230.0 - (v.mTotalLaps as f64 * 9.0) % 200.0) as u8;
            // every car also has a telemetry slot (positions there are fresher; offset 0.5 m so tests can tell)
            let t = &mut o.telemetry.telemInfo[i];
            t.mID = i as i32 + 1;
            t.mElapsedTime = et;
            t.mPos.x = x + 0.5;
            t.mPos.z = z;
            t.mLocalVel.z = -speed;
            t.mUnfilteredThrottle = 1.0;
            t.mVirtualEnergy = (0.95 - ((et * speed / lap_len) * (0.035 + i as f64 * 0.002)) % 0.8) as f32;
        }
        // the player: car 4, braking hard twice a lap
        o.scoring.scoringInfo.mGameMode = 2;
        if self.events {
            // every ~6 minutes of game time: a penalty at 2 min, a knock at 3 min, a pit stop at 5 min
            let cyc = et % 360.0;
            let sv = &mut o.scoring.vehScoringInfo[3];
            sv.mNumPenalties = ((et / 360.0) as i16) + (cyc > 120.0) as i16;
            sv.mNumPitstops = ((et / 360.0) as i16) + (cyc > 300.0) as i16;
            sv.mInPits = (300.0..318.0).contains(&cyc) as u8;
            if cyc > 180.0 { o.telemetry.telemInfo[3].mDentSeverity[1] = 1 }
            o.scoring.scoringInfo.mYellowFlagState = (240.0..255.0).contains(&cyc) as u8;
        }
        let d = o.scoring.vehScoringInfo[3].mLapDist;
        let (x, z) = pos_at(d);
        let ph = d / lap_len;
        let braking = (0.20..0.26).contains(&ph) || (0.70..0.76).contains(&ph);
        let speed = if braking { 22.0 } else { 50.0 };
        let t = &mut o.telemetry;
        t.activeVehicles = 8;
        t.playerHasVehicle = 1;
        t.playerVehicleIdx = 3;
        let v = &mut t.telemInfo[3];
        v.mID = 4;
        v.mElapsedTime = et;
        v.mPos.x = x;
        v.mLapNumber = o.scoring.vehScoringInfo[3].mTotalLaps as i32;
        v.mPos.x = x;
        v.mPos.z = z;
        v.mLocalVel.z = -speed;
        v.mGear = if braking { 3 } else { 5 };
        v.mEngineRPM = 6000.0 + 1500.0 * (et * 2.0).sin();
        v.mUnfilteredThrottle = if braking { 0.0 } else { 1.0 };
        v.mUnfilteredBrake = if braking { 0.9 } else { 0.0 };
        v.mUnfilteredSteering = (ph * std::f64::consts::TAU * 2.0).sin() * 0.3;
        v.mABSActive = (braking && (et * 10.0) as i64 % 3 == 0) as u8;
        v.mFuel = 60.0;
        v.mFuelCapacity = 100.0;
        v.mVirtualEnergy = (0.95 - ((et * 50.0 / lap_len) * 0.04) % 0.8) as f32;
        v.mRearBrakeBias = 0.458;
        v.mTC = 3;
        v.mABS = 4;
        v.mMotorMap = 2;
        for (k, w) in v.mWheels.iter_mut().enumerate() {
            w.mStaticUndeflectedRadius = 34;
            w.mRotation = -speed / 0.34;
            w.mBrakePressure = if braking { if k < 2 { 0.9 * 0.542 } else { 0.9 * 0.458 } } else { 0.0 };
            w.mTemperature = [361.0, 363.0, 362.0];
            w.mTireCarcassTemperature = 360.0;
            w.mPressure = 171.0;
            w.mWear = 0.82;
            w.mBrakeTemp = 700.0;
        }
        Some(o)
    }
}

/// The game's shared memory (Windows). Elsewhere: no game, nothing to read.
#[cfg(not(windows))]
pub fn game() -> Box<dyn Source> {
    struct NoGame;
    impl Source for NoGame {
        fn read(&mut self) -> Option<Box<LMUObjectOut>> {
            None
        }
    }
    Box::new(NoGame)
}
#[cfg(windows)]
pub fn game() -> Box<dyn Source> {
    Box::new(crate::lmu::shm::SharedMemory::new())
}

#[cfg(test)]
mod demo_tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use std::sync::Arc;
    #[test]
    fn demo_runs_faster_and_has_race_events() {
        let sp = Arc::new(AtomicU32::new(100_000)); // 1000× so 0.15 s of real time = 150 s of race
        let mut f = FakeSource::demo(sp.clone());
        std::thread::sleep(std::time::Duration::from_millis(150));
        let o = f.read().unwrap();
        let et = o.scoring.scoringInfo.mCurrentET;
        assert!(et > 120.0 && et < 300.0, "game time {et}");
        assert_eq!({ o.scoring.vehScoringInfo[3].mNumPenalties }, 1, "a penalty after 2 minutes");
        sp.store(0, std::sync::atomic::Ordering::Relaxed); // paused
        let a = f.read().unwrap().scoring.scoringInfo.mCurrentET;
        std::thread::sleep(std::time::Duration::from_millis(30));
        let b = f.read().unwrap().scoring.scoringInfo.mCurrentET;
        assert_eq!(b, a, "speed changes while running");
    }
}
