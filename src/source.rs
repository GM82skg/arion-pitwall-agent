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
    cars: Vec<FakeCar>,
}

/// One fake car driving the test track: it slows for two corners, so it brakes, turns and accelerates.
#[derive(Clone, Default)]
struct FakeCar {
    d: f64,        // distance into the lap (m)
    v: f64,        // speed (m/s)
    brake: f64,
    throttle: f64,
    laps: i16,
    lap_start: f64,
    last: f64,
    best: f64,
    s1: f64,
    s2: f64,
    last_s1: f64,
    last_s2: f64,
}

pub const FAKE_LAP: f64 = 1600.0;
/// Top speed and corner sharpness per car. Car 6 (GT3) is the class's fastest: it brakes later for sharper corners.
fn fake_car_spec(i: usize) -> (f64, f64) {
    match i {
        0..=2 => (60.0 - i as f64, 0.034),  // Hypercars
        5 => (51.0, 0.026),                 // fastest GT3: later braking
        3 => (50.0, 0.030),                 // the player
        _ => (49.5 - (i as f64 - 4.0) * 0.4, 0.031),
    }
}
/// Speed target along the lap (fraction 0..1): corners at 25 % and 75 %.
fn fake_profile(f: f64, w: f64) -> f64 {
    let bump = |c: f64| (-((f - c) / w).powi(2)).exp();
    1.0 - 0.55 * bump(0.25) - 0.45 * bump(0.75)
}
/// Track shape: an oval whose two ends are the two corners.
pub fn fake_pos(d: f64) -> (f64, f64) {
    let a = d / FAKE_LAP * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
    (400.0 * a.cos(), 150.0 * a.sin())
}

impl FakeSource {
    pub fn new() -> Self {
        Self::with_speed(1.0)
    }
    /// A faster clock, so tests see laps, stints and energy use within seconds.
    pub fn with_speed(speed: f64) -> Self {
        Self::make(std::sync::Arc::new(std::sync::atomic::AtomicU32::new((speed * 100.0) as u32)), false)
    }
    /// Demo mode: the speed can be changed while it runs (1× or 10×), and race events happen now and then.
    pub fn demo(speed: std::sync::Arc<std::sync::atomic::AtomicU32>) -> Self {
        Self::make(speed, true)
    }
    fn make(speed: std::sync::Arc<std::sync::atomic::AtomicU32>, events: bool) -> Self {
        let cars = (0..8).map(|i| {
            let (vmax, w) = fake_car_spec(i);
            let d = (8 - i) as f64 * 140.0;
            FakeCar { d, v: vmax * fake_profile(d / FAKE_LAP, w), ..Default::default() }
        }).collect();
        FakeSource { last: std::time::Instant::now(), clock: 0.0, speed, events, cars }
    }
    /// Moves every car forward by dt seconds of race time (small steps, so 10× stays smooth).
    fn advance(&mut self, et0: f64, dt: f64) {
        let steps = (dt / 0.02).ceil().max(1.0) as usize;
        let h = dt / steps as f64;
        for (i, c) in self.cars.iter_mut().enumerate() {
            let (vmax, w) = fake_car_spec(i);
            for k in 0..steps {
                let t = et0 + h * (k as f64 + 1.0);
                // look 40 m ahead: brake if the target there is lower, otherwise accelerate
                let ahead = vmax * fake_profile(((c.d + 40.0) % FAKE_LAP) / FAKE_LAP, w);
                let target = ahead.min(vmax * fake_profile(c.d / FAKE_LAP, w));
                let acc = if target < c.v - 0.3 { -14.0 } else if target > c.v + 0.3 { 7.0 } else { 0.0 };
                c.brake = if acc < 0.0 { 0.85 } else { 0.0 };
                c.throttle = if acc > 0.0 { 1.0 } else if acc == 0.0 { 0.75 } else { 0.0 };
                c.v = (c.v + acc * h).clamp(15.0, vmax);
                let before = c.d;
                c.d += c.v * h;
                if before < FAKE_LAP / 3.0 && c.d >= FAKE_LAP / 3.0 { c.s1 = t - c.lap_start }
                if before < FAKE_LAP * 2.0 / 3.0 && c.d >= FAKE_LAP * 2.0 / 3.0 { c.s2 = t - c.lap_start }
                if c.d >= FAKE_LAP {
                    c.d -= FAKE_LAP;
                    if c.lap_start > 0.0 || c.laps > 0 {
                        c.last = t - c.lap_start;
                        c.last_s1 = c.s1;
                        c.last_s2 = c.s2;
                        if c.best == 0.0 || c.last < c.best { c.best = c.last }
                    }
                    c.laps += 1;
                    c.lap_start = t;
                }
            }
        }
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
        let dt = now.duration_since(self.last).as_secs_f64() * k;
        self.last = now;
        Some(self.step(dt))
    }
}

impl FakeSource {
    /// Advance the race by dt seconds and return LMU's data at that moment (tests call this directly: exact, repeatable).
    pub fn step(&mut self, dt: f64) -> Box<LMUObjectOut> {
        let et0 = self.clock;
        self.clock += dt;
        let et = self.clock;
        self.advance(et0, dt);
        let mut o = zeroed();
        let si = &mut o.scoring.scoringInfo;
        put(&mut si.mTrackName, "Test Ring");
        si.mSession = 10;
        si.mCurrentET = et;
        si.mEndET = 6.0 * 3600.0;
        si.mSessionTimeRemaining = (6.0 * 3600.0 - et) as f32;
        si.mGamePhase = 5;
        // LMU sends 11 for a sector with no local yellow and 1 for a local yellow (seen in LMU on 24 Nov)
        si.mSectorFlag = [11, 11, 11];
        si.mAmbientTemp = 22.8;
        si.mTrackTemp = 31.4;
        si.mLapDist = FAKE_LAP;
        si.mNumVehicles = 8;
        si.mTrackGripLevel = 3;
        si.mGameMode = 2;
        // places: by laps and distance
        let mut order: Vec<usize> = (0..8).collect();
        order.sort_by(|&a, &b| {
            let ta = self.cars[a].laps as f64 * FAKE_LAP + self.cars[a].d;
            let tb = self.cars[b].laps as f64 * FAKE_LAP + self.cars[b].d;
            tb.partial_cmp(&ta).unwrap()
        });
        let lead = self.cars[order[0]].laps as f64 * FAKE_LAP + self.cars[order[0]].d;
        for i in 0..8usize {
            let c = self.cars[i].clone();
            let (x, z) = fake_pos(c.d);
            let v = &mut o.scoring.vehScoringInfo[i];
            v.mID = i as i32 + 1;
            put(&mut v.mDriverName, &format!("Driver {}", i + 1));
            put(&mut v.mVehicleName, &format!("Car #{}", 10 + i));
            put(&mut v.mVehicleClass, if i < 3 { "Hypercar" } else { "GT3" });
            v.mPlace = order.iter().position(|&o| o == i).unwrap_or(i) as u8 + 1;
            v.mLapDist = c.d;
            v.mPos.x = x;
            v.mPos.z = z;
            v.mTotalLaps = c.laps;
            v.mBestLapTime = c.best;
            v.mLastLapTime = c.last;
            v.mLastSector1 = c.last_s1;
            v.mLastSector2 = c.last_s2;
            v.mCurSector1 = c.s1;
            v.mCurSector2 = c.s2;
            v.mBestSector1 = if c.best > 0.0 { c.best * 0.31 } else { 0.0 };
            v.mBestSector2 = if c.best > 0.0 { c.best * 0.66 } else { 0.0 };
            v.mIsPlayer = (i == 3) as u8;
            v.mCountLapFlag = 2;
            v.mControl = if i == 3 { 0 } else if i % 2 == 0 { 2 } else { 1 }; // you, remote players, AI
            v.mTimeBehindLeader = (lead - (c.laps as f64 * FAKE_LAP + c.d)) / 45.0;
            v.mFuelFraction = (230.0 - (c.laps as f64 * 9.0) % 200.0) as u8;
            // every car also has a telemetry slot (positions there are fresher; offset 0.5 m so tests can tell)
            let t = &mut o.telemetry.telemInfo[i];
            t.mID = i as i32 + 1;
            t.mElapsedTime = et;
            t.mLapStartET = c.lap_start;
            t.mLapNumber = c.laps as i32;
            t.mPos.x = x + if i == 3 { 0.0 } else { 0.5 };
            t.mPos.z = z;
            t.mLocalVel.z = -c.v;
            t.mUnfilteredThrottle = c.throttle as f32 as f64;
            t.mUnfilteredBrake = c.brake;
            t.mVirtualEnergy = (0.95 - ((c.laps as f64 + c.d / FAKE_LAP) * (0.035 + i as f64 * 0.002)) % 0.8) as f32;
            // tyres (v0.1.24): this fake car model's list is 0 Soft, 1 Medium, 2 Hard. Car 1 runs soft all round, car 2 a soft
            // left front with mediums elsewhere, car 3 hards, the rest mediums; life drops with laps and differs per wheel
            let (names, idx): ([&str; 2], [u8; 4]) = match i { 0 => (["Soft", "Soft"], [0; 4]), 1 => (["Medium", "Medium"], [0, 1, 1, 1]), 2 => (["Hard", "Hard"], [2; 4]), _ => (["Medium", "Medium"], [1; 4]) };
            for (k, b) in names[0].bytes().enumerate() { t.mFrontTireCompoundName[k] = b }
            for (k, b) in names[1].bytes().enumerate() { t.mRearTireCompoundName[k] = b }
            for (k, w) in t.mWheels.iter_mut().enumerate() {
                w.mCompoundIndex = idx[k];
                w.mWear = (0.98 - ((c.laps as f64 + c.d / FAKE_LAP) * (0.012 + k as f64 * 0.002 + i as f64 * 0.001)) % 0.6).max(0.3);
            }
        }
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
        // the player: car 4, with full telemetry
        let c = self.cars[3].clone();
        let braking = c.brake > 0.0;
        let t = &mut o.telemetry;
        t.activeVehicles = 8;
        t.playerHasVehicle = 1;
        t.playerVehicleIdx = 3;
        let v = &mut t.telemInfo[3];
        v.mGear = if c.v < 25.0 { 3 } else if c.v < 38.0 { 4 } else { 5 };
        v.mEngineRPM = 4500.0 + c.v * 60.0;
        v.mUnfilteredSteering = (-((c.d / FAKE_LAP - 0.25) / 0.04).powi(2)).exp() * 0.5 - (-((c.d / FAKE_LAP - 0.75) / 0.04).powi(2)).exp() * 0.5;
        v.mABSActive = (braking && (et * 10.0) as i64 % 3 == 0) as u8;
        v.mFuel = 60.0;
        v.mFuelCapacity = 100.0;
        v.mRearBrakeBias = 0.458;
        v.mTC = 3;
        v.mABS = 4;
        v.mMotorMap = 2;
        for (k, w) in v.mWheels.iter_mut().enumerate() {
            w.mStaticUndeflectedRadius = 34;
            w.mRotation = -c.v / 0.34;
            w.mBrakePressure = if braking { if k < 2 { 0.9 * 0.542 } else { 0.9 * 0.458 } } else { 0.0 };
            w.mTemperature = [361.0, 363.0, 362.0];
            w.mTireCarcassTemperature = 360.0;
            w.mPressure = 171.0;
            w.mWear = 0.82;
            w.mBrakeTemp = 700.0;
        }
        o
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
