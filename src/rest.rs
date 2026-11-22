//! LMU's local web interface (port 6397), used by the test report in Phase 1:
//! is it there during a race, how fast does it answer, which parts return data.
use std::time::{Duration, Instant};

pub const BASE: &str = "http://localhost:6397";
pub const ENDPOINTS: [&str; 5] = [
    "/rest/sessions",
    "/rest/sessions/weather",
    "/rest/garage/getPlayerGarageData",
    "/rest/garage/UIScreen/RepairAndRefuel",
    "/rest/strategy/pitstop-estimate",
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct Probe {
    pub endpoint: String,
    pub ok: bool,
    pub status: u16,
    pub ms: u64,
    pub bytes: usize,
}

pub fn probe(base: &str) -> Vec<Probe> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .http_status_as_error(false)
        .build()
        .into();
    ENDPOINTS
        .iter()
        .map(|e| {
            let t = Instant::now();
            match agent.get(&format!("{base}{e}")).call() {
                Ok(mut r) => {
                    let status = r.status().as_u16();
                    let bytes = r.body_mut().read_to_vec().map(|b| b.len()).unwrap_or(0);
                    Probe { endpoint: e.to_string(), ok: (200..300).contains(&status), status, ms: t.elapsed().as_millis() as u64, bytes }
                }
                Err(_) => Probe { endpoint: e.to_string(), ok: false, status: 0, ms: t.elapsed().as_millis() as u64, bytes: 0 },
            }
        })
        .collect()
}

// ---------- live LMU web data for the dashboard (Phase 2) ----------
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

fn agent2() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(2))).http_status_as_error(false).build().into()
}
fn get(a: &ureq::Agent, base: &str, path: &str) -> Option<Value> {
    let mut r = a.get(&format!("{base}{path}")).call().ok()?;
    if !(200..300).contains(&r.status().as_u16()) {
        return None;
    }
    r.body_mut().read_json::<Value>().ok()
}
fn f(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}
fn four(v: &Value) -> Option<Vec<f64>> {
    let a = v.as_array()?;
    (a.len() >= 4).then(|| a[..4].iter().map(|x| f(x).unwrap_or(-1.0)).collect())
}

/// LMU's forecast: per session, 5 slots starting at 0, 20, 40, 60, 80 % of the session (as TinyPedal reads it):
/// [start fraction, sky 0–10, temperature °C, rain chance 0–1]
pub fn parse_forecast(w: &Value) -> Value {
    let nodes = [(0.0, "START"), (0.2, "NODE_25"), (0.4, "NODE_50"), (0.6, "NODE_75"), (0.8, "FINISH")];
    let mut out = serde_json::Map::new();
    for (key, name) in [("PRACTICE", "practice"), ("QUALIFY", "qualify"), ("RACE", "race")] {
        let Some(sess) = w.get(key) else { continue };
        let slots: Option<Vec<Value>> = nodes.iter().map(|(fr, n)| {
            let node = sess.get(*n)?;
            let sky = f(node.get("WNV_SKY")?.get("currentValue")?)?;
            let temp = f(node.get("WNV_TEMPERATURE")?.get("currentValue")?)?;
            let rain = f(node.get("WNV_RAIN_CHANCE")?.get("currentValue")?)? * 0.01;
            Some(json!([fr, sky as i64, temp, rain.clamp(0.0, 1.0)]))
        }).collect();
        if let Some(sl) = slots {
            out.insert(name.into(), Value::Array(sl));
        }
    }
    Value::Object(out)
}

/// Repair & refuel screen: aero damage 0–1, suspension damage per wheel 0–1, brake disc thickness (m), max energy.
pub fn parse_wear(r: &Value) -> Value {
    let w = r.get("wearables");
    json!({
        "aero": w.and_then(|w| w.get("body")).and_then(|b| b.get("aero")).and_then(f),
        "susp": w.and_then(|w| w.get("suspension")).and_then(four),
        "brakeMm": w.and_then(|w| w.get("brakes")).and_then(four).map(|v| v.iter().map(|m| if *m >= 0.0 { m * 1000.0 } else { -1.0 }).collect::<Vec<_>>()),
        "maxVE": r.get("fuelInfo").and_then(|i| i.get("maxVirtualEnergy")).and_then(f),
    })
}

/// Keeps the latest LMU web data (last good values stay when a call fails, e.g. while loading).
#[derive(Clone, Default)]
pub struct Live {
    pub data: Arc<Mutex<Value>>,
}
impl Live {
    pub fn snapshot(&self) -> Value {
        self.data.lock().unwrap().clone()
    }
    fn set(&self, key: &str, v: Value) {
        let mut d = self.data.lock().unwrap();
        if !d.is_object() {
            *d = json!({});
        }
        d[key] = v;
        d["at"] = json!(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|x| x.as_secs()).unwrap_or(0));
    }
    /// Runs on its own thread: wear/damage and pit stop estimate every 2 s, forecast every 30 s.
    pub fn run(&self, base: String, stop: Arc<AtomicBool>, active: Arc<AtomicBool>) {
        let a = agent2();
        let mut n: u64 = 0;
        while !stop.load(Ordering::Relaxed) {
            if active.load(Ordering::Relaxed) {
                if let Some(r) = get(&a, &base, "/rest/garage/UIScreen/RepairAndRefuel") {
                    self.set("wear", parse_wear(&r));
                }
                if let Some(p) = get(&a, &base, "/rest/strategy/pitstop-estimate") {
                    self.set("pit", json!({ "total": p.get("total").and_then(f), "repair": p.get("damage").and_then(f) }));
                }
                if n % 15 == 0 {
                    if let Some(w) = get(&a, &base, "/rest/sessions/weather") {
                        let fc = parse_forecast(&w);
                        if fc.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                            self.set("forecast", fc);
                        }
                    }
                }
                n += 1;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_forecast_and_wear_like_tinypedal() {
        let node = |sky: i64, t: f64, r: f64| json!({"WNV_SKY":{"currentValue":sky},"WNV_TEMPERATURE":{"currentValue":t},"WNV_RAIN_CHANCE":{"currentValue":r}});
        let w = json!({"RACE":{"START":node(0,24.0,0.0),"NODE_25":node(2,23.0,10.0),"NODE_50":node(6,21.0,30.0),"NODE_75":node(8,20.0,60.0),"FINISH":node(9,19.0,80.0)}});
        let fc = parse_forecast(&w);
        assert_eq!(fc["race"].as_array().unwrap().len(), 5);
        assert_eq!(fc["race"][4], json!([0.8, 9, 19.0, 0.8]));
        assert!(fc.get("practice").is_none());
        let r = json!({"wearables":{"body":{"aero":0.14},"suspension":[0.06,0,0,0.01],"brakes":[0.032,0.031,0.028,0.028]},"fuelInfo":{"maxVirtualEnergy":100.0}});
        let wr = parse_wear(&r);
        assert_eq!(wr["aero"], json!(0.14));
        assert_eq!(wr["susp"][0], json!(0.06));
        assert!((wr["brakeMm"][0].as_f64().unwrap() - 32.0).abs() < 1e-9);
        assert_eq!(wr["maxVE"], json!(100.0));
        assert!(parse_wear(&json!({}))["aero"].is_null(), "missing parts stay empty");
    }
}
