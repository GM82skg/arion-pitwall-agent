//! What the coaches receive. Documented in PROTOCOL.md; the dashboard reads exactly this.
//! Fast frame (kind 1, 30 Hz) and cars frame (kind 2, 10 Hz) are binary, little-endian, on the "fast" channel.
//! Slow messages are JSON text on the "slow" channel, 2 Hz.

use crate::lmu::layout::{LMUObjectOut, LMUVehicleScoring, LMUVehicleTelemetry};
use serde_json::{json, Value};

pub const PROTOCOL_VERSION: u8 = 1;
pub const FAST_LEN: usize = 76;

/// Text from a fixed-size, zero-terminated C string.
pub fn cstr(b: &[u8]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..n]).trim().to_string()
}

pub fn player(o: &LMUObjectOut) -> Option<(usize, LMUVehicleTelemetry)> {
    let t = &o.telemetry;
    if t.playerHasVehicle == 0 {
        return None;
    }
    let i = t.playerVehicleIdx as usize;
    if i >= t.telemInfo.len() {
        return None;
    }
    Some((i, t.telemInfo[i]))
}

fn scoring_of(o: &LMUObjectOut, id: i32) -> Option<LMUVehicleScoring> {
    let n = (o.scoring.scoringInfo.mNumVehicles.max(0) as usize).min(o.scoring.vehScoringInfo.len());
    o.scoring.vehScoringInfo[..n].iter().copied().find(|v| { let x = v.mID; x == id })
}

/// Bits per wheel (FL, FR, RL, RR): wheel turning far slower than the car moves while braking.
pub fn wheel_locks(v: &LMUVehicleTelemetry) -> u8 {
    let lv = v.mLocalVel;
    let speed = (lv.x * lv.x + lv.y * lv.y + lv.z * lv.z).sqrt();
    let brake = v.mUnfilteredBrake;
    if speed < 5.0 || brake < 0.05 {
        return 0;
    }
    let mut bits = 0u8;
    for (i, w) in v.mWheels.iter().enumerate() {
        let r = w.mStaticUndeflectedRadius as f64 / 100.0; // cm → m
        let rot = w.mRotation;
        if r > 0.1 && rot.abs() * r < 0.3 * speed {
            bits |= 1 << i;
        }
    }
    bits
}

/// Fast frame: everything the live trace and the live trail need.
pub fn fast_frame(o: &LMUObjectOut, seq: u32) -> Option<[u8; FAST_LEN]> {
    let (_, v) = player(o)?;
    let s = scoring_of(o, v.mID);
    let lv = v.mLocalVel;
    let speed = (lv.x * lv.x + lv.y * lv.y + lv.z * lv.z).sqrt();
    let pos = v.mPos;
    let mut flags: u16 = 0;
    if v.mABSActive != 0 { flags |= 1 }
    if v.mTCActive != 0 { flags |= 2 }
    if s.map(|s| s.mInPits != 0).unwrap_or(false) { flags |= 4 }
    if v.mLapInvalidated != 0 { flags |= 8 }
    flags |= (wheel_locks(&v) as u16) << 4;
    let mut b = [0u8; FAST_LEN];
    let mut w = 0usize;
    let mut put = |x: &[u8]| { b[w..w + x.len()].copy_from_slice(x); w += x.len(); };
    put(&[PROTOCOL_VERSION, 1]);
    put(&flags.to_le_bytes());
    put(&seq.to_le_bytes());
    put(&{ v.mElapsedTime }.to_le_bytes());
    put(&(s.map(|s| s.mLapDist).unwrap_or(0.0) as f32).to_le_bytes());
    put(&(v.mLapNumber.clamp(-32768, 32767) as i16).to_le_bytes());
    put(&[(v.mGear.clamp(-1, 15) as i8) as u8, 0]);
    for x in [pos.x, pos.y, pos.z, speed, v.mEngineRPM, v.mUnfilteredThrottle, v.mUnfilteredBrake, v.mUnfilteredSteering, v.mUnfilteredClutch] {
        put(&(x as f32).to_le_bytes());
    }
    for wh in v.mWheels.iter() {
        put(&(wh.mBrakePressure as f32).to_le_bytes());
    }
    debug_assert_eq!(w, FAST_LEN);
    Some(b)
}

/// Telemetry slot of each car id (positions there update ~50 Hz; timing positions only 5–6 Hz).
pub fn telemetry_by_id(o: &LMUObjectOut) -> std::collections::HashMap<i32, usize> {
    let n = (o.telemetry.activeVehicles as usize).min(o.telemetry.telemInfo.len());
    (0..n).map(|i| (o.telemetry.telemInfo[i].mID, i)).collect()
}

/// Cars frame: every car's track position for the map (16 bytes per car), from telemetry when available.
pub fn cars_frame(o: &LMUObjectOut) -> Vec<u8> {
    let n = (o.scoring.scoringInfo.mNumVehicles.max(0) as usize).min(o.scoring.vehScoringInfo.len());
    let slots = telemetry_by_id(o);
    let mut b = Vec::with_capacity(4 + n * 16);
    b.extend_from_slice(&[PROTOCOL_VERSION, 2]);
    b.extend_from_slice(&(n as u16).to_le_bytes());
    for v in &o.scoring.vehScoringInfo[..n] {
        let mut p = v.mPos;
        if let Some(&i) = slots.get(&{ v.mID }) {
            let tp = o.telemetry.telemInfo[i].mPos;
            if tp.x != 0.0 || tp.z != 0.0 {
                p = tp;
            }
        }
        b.extend_from_slice(&(v.mID as u16).to_le_bytes());
        b.push(v.mPlace);
        b.push((v.mInPits != 0) as u8 | ((v.mIsPlayer != 0) as u8) << 1);
        b.extend_from_slice(&(p.x as f32).to_le_bytes());
        b.extend_from_slice(&(p.z as f32).to_le_bytes());
        b.extend_from_slice(&(v.mLapDist as f32).to_le_bytes());
    }
    b
}

/// Copy a value out of a packed struct (a reference to it would be unaligned).
#[inline]
fn cp<T: Copy>(x: T) -> T {
    x
}
const K: f64 = 273.15;

fn wheel_json(v: &LMUVehicleTelemetry) -> Value {
    let wheels = cp(v.mWheels);
    Value::Array(wheels.iter().map(|w| {
        let t = cp(w.mTemperature);
        json!({ "t": [t[0] - K, t[1] - K, t[2] - K], "carcass": cp(w.mTireCarcassTemperature) - K, "p": cp(w.mPressure),
                "wear": cp(w.mWear), "brake": cp(w.mBrakeTemp) - K, "flat": cp(w.mFlat) != 0, "compound": cp(w.mCompoundType) })
    }).collect())
}

/// Slow message: the selected car, the session and all cars (names as shown in the game). No SteamIDs.
pub fn slow_json(o: &LMUObjectOut) -> Option<Value> {
    slow_json_with(o, &Value::Null)
}

/// Slow message plus LMU's web data (forecast, pit stop estimate, wear and damage) when available.
pub fn slow_json_with(o: &LMUObjectOut, rest: &Value) -> Option<Value> {
    let (_, v) = player(o)?;
    let si = cp(o.scoring.scoringInfo);
    let n = (cp(si.mNumVehicles).max(0) as usize).min(o.scoring.vehScoringInfo.len());
    let slots = telemetry_by_id(o);
    let grid: Vec<Value> = o.scoring.vehScoringInfo[..n].iter().map(|c| json!({
        "id": cp(c.mID), "name": cstr(&cp(c.mDriverName)), "car": cstr(&cp(c.mVehicleName)), "class": cstr(&cp(c.mVehicleClass)),
        "place": cp(c.mPlace), "laps": cp(c.mTotalLaps), "best": cp(c.mBestLapTime), "last": cp(c.mLastLapTime),
        "s1": cp(c.mLastSector1), "s2": cp(c.mLastSector2), "behindNext": cp(c.mTimeBehindNext), "behindLeader": cp(c.mTimeBehindLeader),
        "pits": cp(c.mInPits) != 0, "stops": cp(c.mNumPitstops), "player": cp(c.mIsPlayer) != 0,
        "bestS1": cp(c.mBestSector1), "bestS2": cp(c.mBestSector2), "curS1": cp(c.mCurSector1), "curS2": cp(c.mCurSector2),
        "est": cp(c.mEstimatedLapTime), "intoLap": cp(c.mTimeIntoLap), "fuel": cp(c.mFuelFraction) as f64 / 255.0,
        "penalties": cp(c.mNumPenalties), "pitState": cp(c.mPitState), "countLap": cp(c.mCountLapFlag), "control": cp(c.mControl),
        "energy": slots.get(&cp(c.mID)).map(|&i| cp(o.telemetry.telemInfo[i].mVirtualEnergy)),
        "tyreF": slots.get(&cp(c.mID)).map(|&i| cstr(&cp(o.telemetry.telemInfo[i].mFrontTireCompoundName))),
        "tyreR": slots.get(&cp(c.mID)).map(|&i| cstr(&cp(o.telemetry.telemInfo[i].mRearTireCompoundName))),
        // v0.1.24: per wheel (FL FR RL RR) for every car: tyre life 0–1 and the compound's index in that car's tyre list.
        // Whether LMU fills these for other cars in an online race is still to be checked; the dashboard copes if not.
        "tyreWear": slots.get(&cp(c.mID)).map(|&i| cp(o.telemetry.telemInfo[i].mWheels).iter().map(|w| cp(w.mWear)).collect::<Vec<f64>>()),
        "tyreCI": slots.get(&cp(c.mID)).map(|&i| cp(o.telemetry.telemInfo[i].mWheels).iter().map(|w| cp(w.mCompoundIndex)).collect::<Vec<u8>>()) })).collect();
    Some(json!({
        "t": "slow", "v": PROTOCOL_VERSION,
        "session": { "track": cstr(&cp(si.mTrackName)), "type": cp(si.mSession), "length": cp(si.mLapDist), "start": cp(si.mStartET), "maxLaps": cp(si.mMaxLaps), "now": cp(si.mCurrentET), "end": cp(si.mEndET),
                     "left": cp(si.mSessionTimeRemaining), "phase": cp(si.mGamePhase), "yellow": cp(si.mYellowFlagState),
                     "sectorFlags": cp(si.mSectorFlag), "air": cp(si.mAmbientTemp), "track_temp": cp(si.mTrackTemp), "rain": cp(si.mRaining),
                     "wet": [cp(si.mMinPathWetness), cp(si.mAvgPathWetness), cp(si.mMaxPathWetness)], "grip": cp(si.mTrackGripLevel),
                     "clouds": cp(si.mCloudCoverage) },
        "car": { "id": cp(v.mID), "fuel": cp(v.mFuel), "fuelCap": cp(v.mFuelCapacity), "energy": cp(v.mVirtualEnergy), "soc": cp(v.mStateOfCharge),
                 "tc": cp(v.mTC), "abs": cp(v.mABS), "map": cp(v.mMotorMap), "bias": 1.0 - cp(v.mRearBrakeBias), "rearBias": cp(v.mRearBrakeBias), "impact": cp(v.mLastImpactMagnitude), "impactET": cp(v.mLastImpactET), "delta": cp(v.mDeltaBest),
                 "damage": cp(v.mDentSeverity), "detached": cp(v.mDetached) != 0, "wheels": wheel_json(&v) },
        "grid": grid,
        "rest": rest,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FakeSource, Source};
    #[test]
    fn frames_from_the_fake_car() {
        let mut f = FakeSource::new();
        let o = f.read().expect("fake data");
        let b = fast_frame(&o, 7).expect("player car");
        assert_eq!(b.len(), FAST_LEN);
        assert_eq!(b[0], 1);
        assert_eq!(b[1], 1);
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 7);
        let speed = f32::from_le_bytes(b[36..40].try_into().unwrap());
        assert!(speed > 10.0, "speed {speed}");
        let c = cars_frame(&o);
        assert_eq!(c[1], 2);
        let n = u16::from_le_bytes([c[2], c[3]]) as usize;
        assert_eq!(c.len(), 4 + 16 * n);
        // positions come from telemetry (fake telemetry is offset by +0.5 m from timing)
        let x0 = f32::from_le_bytes(c[8..12].try_into().unwrap()) as f64;
        let sx = o.scoring.vehScoringInfo[0].mPos.x;
        assert!((x0 - (sx + 0.5)).abs() < 1e-3, "cars frame should use telemetry position");
        let s = slow_json(&o).unwrap();
        assert_eq!(s["grid"].as_array().unwrap().len(), n);
        assert_eq!(s["session"]["track"], "Test Ring");
        assert!(s.to_string().len() < 16000);
    }
    #[test]
    fn lock_detection() {
        let mut f = FakeSource::new();
        let o = f.read().unwrap();
        let (_, mut v) = player(&o).unwrap();
        v.mLocalVel.z = 50.0; v.mUnfilteredBrake = 1.0;
        for w in v.mWheels.iter_mut() { w.mStaticUndeflectedRadius = 34; w.mRotation = 50.0 / 0.34; }
        assert_eq!(wheel_locks(&v), 0);
        v.mWheels[0].mRotation = 5.0; // front left nearly stopped
        assert_eq!(wheel_locks(&v), 1);
        v.mUnfilteredBrake = 0.0;
        assert_eq!(wheel_locks(&v), 0, "no brake, no lock flag");
    }
}
