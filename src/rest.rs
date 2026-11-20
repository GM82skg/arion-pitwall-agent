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
