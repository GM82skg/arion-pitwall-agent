#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! ARION Pit Wall driver app. Started with no options: the window. With --key: command line (tests, demos).
//!   arion-pitwall-agent --key arion_tk_…            stream from Le Mans Ultimate (Windows)
//!   arion-pitwall-agent --key … --fake               stream a fake car (tests, demos)
//!   … --fake --fake-penalties                         … whose cars get penalties (with their type) now and then
//!   … --fake --fake-settings                          … whose driver changes TC1/TC2/TC3, ABS, bias and map now and then
//! Test options: --url, --apikey (another database), --seconds N (stop after N seconds).
//!   arion-pitwall-agent --update                     the window's «Update now» from the command line
//!   arion-pitwall-agent --check-updates              the tray's «Check for updates» from the command line

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use arion_pitwall_agent::{config, local, report, session, signal, source};

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn main() {
    arion_pitwall_agent::update::at_start();
    // v0.1.28: the tray's «Check for updates» from the command line (tests): prints the window's line
    if std::env::args().any(|a| a == "--check-updates") {
        use arion_pitwall_agent::update;
        let m = update::manual(config::VERSION);
        match update::manual_text(&m, false) {
            Some((_, t)) => println!("{t}"),
            None => if let update::Manual::Newer(v) = &m { println!("Update available: v{v}") },
        }
        std::process::exit(if matches!(m, update::Manual::Failed(_)) { 1 } else { 0 });
    }
    if std::env::args().any(|a| a == "--update") {
        use arion_pitwall_agent::update;
        match update::check(config::VERSION) {
            Ok(None) => println!("Up to date (v{}).", config::VERSION),
            Ok(Some(r)) => match update::install(&r) {
                Ok(exe) => { println!("Updated to v{} ({}).", r.version, exe.display()); }
                Err(e) => { eprintln!("Update failed: {e}"); std::process::exit(1) }
            },
            Err(e) => { eprintln!("Couldn't check for updates: {e}"); std::process::exit(1) }
        }
        return;
    }
    if !std::env::args().any(|a| a == "--key") {
        // --demo: the normal window, streaming a fake car instead of Le Mans Ultimate (for trying everything out)
        if let Err(e) = arion_pitwall_agent::ui::run(std::env::args().any(|a| a == "--demo")) {
            eprintln!("{e}");
        }
        return;
    }
    let key = arg("--key").or_else(|| std::env::var("ARION_KEY").ok()).unwrap_or_default();
    if !config::key_looks_valid(&key) {
        eprintln!("Paste your telemetry key: --key arion_tk_… (Profile → Telemetry key in the crew app).");
        std::process::exit(2);
    }
    let url = arg("--url").unwrap_or_else(|| config::SUPABASE_URL.into());
    let apikey = arg("--apikey").unwrap_or_else(|| config::SUPABASE_KEY.into());
    let fake = std::env::args().any(|a| a == "--fake");
    let seconds: Option<u64> = arg("--seconds").and_then(|s| s.parse().ok());

    let speed: f64 = arg("--fake-speed").and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let src: Box<dyn source::Source> = if fake {
        let mut f = source::FakeSource::with_speed(speed);
        f.pens = std::env::args().any(|a| a == "--fake-penalties");
        f.sets = std::env::args().any(|a| a == "--fake-settings");
        Box::new(f)
    } else { source::game() };
    let door: Arc<dyn signal::Door> = Arc::new(signal::Supabase::new(&url, &apikey, &key));
    let status = Arc::new(Mutex::new(session::Status::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let rep = Arc::new(Mutex::new(report::Report::default()));
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("runtime");
    let (st, sp) = (status.clone(), stop.clone());
    rt.spawn(async move {
        let t0 = std::time::Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let s = st.lock().unwrap().clone();
            println!("{} · {} · {} · LMU {} · coaches {} · frames {}{}", s.state, s.name,
                s.team.map(|t| t.name).unwrap_or_else(|| "practice".into()), if s.lmu { "on track" } else { "waiting" },
                s.viewers, s.frames, s.error.map(|e| format!(" · {e}")).unwrap_or_default());
            if seconds.map(|n| t0.elapsed().as_secs() >= n).unwrap_or(false) { sp.store(true, Ordering::Relaxed); break; }
        }
    });
    let sp = stop.clone();
    let lc = local::Local::new();
    let lc2 = lc.clone();
    rt.spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Some(u) = lc2.url() { println!("My dashboard: {u}"); }
    });
    rt.block_on(session::run(door, src, status, rep.clone(), lc, sp, false));
    if let Some(p) = arg("--report") {
        let _ = std::fs::write(p, rep.lock().unwrap().text(config::VERSION, None));
    }
    // WebRTC keeps helper threads; don't wait for them on exit.
    rt.shutdown_timeout(std::time::Duration::from_secs(2));
    std::process::exit(0);
}
