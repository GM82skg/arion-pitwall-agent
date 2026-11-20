//! The app's main loop: say hello, ask for coaches every 2 s, answer them, and stream telemetry.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::config;
use crate::hub::Hub;
use crate::protocol;
use crate::report::Report;
use crate::rest;
use crate::signal::{self, Door, Team};
use crate::source::Source;

#[derive(Debug, Clone, Default)]
pub struct Status {
    /// "connecting", "online", "error"
    pub state: String,
    pub name: String,
    pub team: Option<Team>,
    pub viewers: usize,
    pub lmu: bool,
    pub error: Option<String>,
    pub frames: u64,
}

async fn call(door: &Arc<dyn Door>, action: &'static str, data: Value) -> Result<Value, String> {
    let d = door.clone();
    tokio::task::spawn_blocking(move || d.call(action, data)).await.map_err(|e| e.to_string())?
}

fn set(status: &Arc<Mutex<Status>>, f: impl FnOnce(&mut Status)) {
    f(&mut status.lock().unwrap());
}

/// Runs until `stop` is set. Never panics on network errors: it reports them and retries.
pub async fn run(door: Arc<dyn Door>, mut source: Box<dyn Source>, status: Arc<Mutex<Status>>, report: Arc<Mutex<Report>>, stop: Arc<AtomicBool>) {
    let hub = Hub::new();

    // Telemetry: read the game at 100 Hz (so the report can see LMU's own update rate), send at most
    // FAST_HZ fast frames, only when the game produced new data. Cars 10 Hz, slow data SLOW_HZ.
    let (hub_t, status_t, stop_t, report_t) = (hub.clone(), status.clone(), stop.clone(), report.clone());
    let telemetry = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(10));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let fast_gap = Duration::from_secs_f64(1.0 / config::FAST_HZ as f64) - Duration::from_millis(3);
        let (mut seq, mut last_et) = (0u32, f64::NAN);
        let (mut last_fast, mut last_cars, mut last_slow) = (Instant::now(), Instant::now(), Instant::now());
        let mut last_rest = Instant::now() - Duration::from_secs(50);
        while !stop_t.load(Ordering::Relaxed) {
            tick.tick().await;
            let snap = source.read();
            let on_track = snap.as_ref().map(|o| protocol::player(o).is_some()).unwrap_or(false);
            set(&status_t, |s| s.lmu = on_track);
            let Some(o) = snap else { continue };
            report_t.lock().unwrap().observe(&o);
            if on_track && last_rest.elapsed() > Duration::from_secs(60) {
                last_rest = Instant::now();
                let r = report_t.clone();
                tokio::task::spawn_blocking(move || { let p = rest::probe(rest::BASE); r.lock().unwrap().rest.push(p); });
            }
            let viewers = hub_t.connected();
            {
                let mut r = report_t.lock().unwrap();
                r.max_viewers = r.max_viewers.max(viewers);
            }
            if viewers == 0 {
                continue;
            }
            if let Some((_, v)) = protocol::player(&o) {
                let et = v.mElapsedTime;
                if et != last_et && last_fast.elapsed() >= fast_gap {
                    last_et = et;
                    last_fast = Instant::now();
                    seq = seq.wrapping_add(1);
                    if let Some(f) = protocol::fast_frame(&o, seq) {
                        hub_t.send_fast(&f).await;
                        set(&status_t, |s| s.frames += 1);
                        report_t.lock().unwrap().frames_sent += 1;
                    }
                }
            }
            if last_cars.elapsed() >= Duration::from_millis(100) {
                last_cars = Instant::now();
                hub_t.send_fast(&protocol::cars_frame(&o)).await;
            }
            if last_slow.elapsed() >= Duration::from_secs_f64(1.0 / config::SLOW_HZ as f64) {
                last_slow = Instant::now();
                if let Some(j) = protocol::slow_json(&o) {
                    hub_t.send_slow(&j.to_string()).await;
                }
            }
        }
    });

    // Connection setup with coaches, through the database.
    'outer: while !stop.load(Ordering::Relaxed) {
        set(&status, |s| { s.state = "connecting".into(); s.error = None; });
        let hello = match call(&door, "hello", json!({ "version": config::VERSION })).await.and_then(|v| signal::parse_hello(&v)) {
            Ok(h) => h,
            Err(e) => {
                set(&status, |s| { s.state = "error".into(); s.error = Some(e); });
                for _ in 0..10 {
                    if stop.load(Ordering::Relaxed) { break 'outer; }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                continue;
            }
        };
        set(&status, |s| { s.state = "online".into(); s.name = hello.name.clone(); s.team = hello.team.clone(); });
        let mut after: i64 = 0;
        while !stop.load(Ordering::Relaxed) {
            match call(&door, "poll", json!({ "after": after })).await {
                Ok(v) => {
                    let (team, sigs) = signal::parse_poll(&v);
                    set(&status, |s| { s.team = team; s.error = None; s.state = "online".into(); });
                    for sg in sigs {
                        after = after.max(sg.id);
                        match sg.kind.as_str() {
                            "offer" => {
                                let reply = match hub.answer(&sg.viewer, &sg.body).await {
                                    Ok(ans) => json!({ "viewer": sg.viewer, "kind": "answer", "body": ans }),
                                    Err(e) => json!({ "viewer": sg.viewer, "kind": "bye", "body": { "reason": e } }),
                                };
                                if let Err(e) = call(&door, "send", reply).await {
                                    set(&status, |s| s.error = Some(e));
                                }
                            }
                            "bye" => hub.remove(&sg.viewer).await,
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    let restart = e.contains("No session") || e.contains("Unknown key");
                    set(&status, |s| s.error = Some(e));
                    if restart {
                        hub.close_all().await;
                        continue 'outer;
                    }
                }
            }
            set(&status, |s| s.viewers = hub.connected());
            for _ in 0..(config::POLL_SECS * 4) {
                if stop.load(Ordering::Relaxed) { break; }
                tokio::time::sleep(Duration::from_millis(250)).await;
                set(&status, |s| s.viewers = hub.connected());
            }
        }
    }
    let _ = call(&door, "bye", json!({})).await;
    hub.close_all().await;
    let _ = telemetry.await;
}
