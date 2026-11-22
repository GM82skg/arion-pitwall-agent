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
    t0: std::time::Instant,
    speed: f64,
}
impl FakeSource {
    pub fn new() -> Self {
        FakeSource { t0: std::time::Instant::now(), speed: 1.0 }
    }
    /// A faster clock, so tests see laps, stints and energy use within seconds.
    pub fn with_speed(speed: f64) -> Self {
        FakeSource { t0: std::time::Instant::now(), speed }
    }
}
impl Default for FakeSource {
    fn default() -> Self {
        Self::new()
    }
}

impl Source for FakeSource {
    fn read(&mut self) -> Option<Box<LMUObjectOut>> {
        let et = self.t0.elapsed().as_secs_f64() * self.speed;
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
