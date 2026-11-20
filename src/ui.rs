//! The small window drivers see. Plain words, few buttons: paste the key once, then it just runs.
//! Language follows Windows (Greek or English).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{self, Color32, RichText};

use crate::{config, keystore, report::Report, session, signal, source};

pub struct T {
    el: bool,
}
impl T {
    fn s<'a>(&self, en: &'a str, el: &'a str) -> &'a str {
        if self.el { el } else { en }
    }
}

pub fn greek() -> bool {
    #[cfg(windows)]
    {
        let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        return (id & 0x3ff) == 0x08; // LANG_GREEK
    }
    #[allow(unreachable_code)]
    std::env::var("LANG").map(|l| l.starts_with("el")).unwrap_or(false)
}

struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Running {
    fn start(key: String, status: Arc<Mutex<session::Status>>, report: Arc<Mutex<Report>>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let sp = stop.clone();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("runtime");
            let door: Arc<dyn signal::Door> = Arc::new(signal::Supabase::new(config::SUPABASE_URL, config::SUPABASE_KEY, &key));
            rt.block_on(session::run(door, source::game(), status, report, sp));
            rt.shutdown_timeout(Duration::from_secs(2));
        });
        Running { stop, thread: Some(thread) }
    }
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub struct AgentApp {
    t: T,
    key_input: String,
    editing: bool,
    msg: Option<(bool, String)>,
    status: Arc<Mutex<session::Status>>,
    report: Arc<Mutex<Report>>,
    running: Option<Running>,
    quitting: bool,
    #[cfg(windows)]
    tray: Option<win::Tray>,
    #[cfg(windows)]
    autostart: bool,
}

impl AgentApp {
    pub fn new() -> Self {
        let mut a = AgentApp {
            t: T { el: greek() },
            key_input: String::new(),
            editing: false,
            msg: None,
            status: Arc::new(Mutex::new(session::Status::default())),
            report: Arc::new(Mutex::new(Report::default())),
            running: None,
            quitting: false,
            #[cfg(windows)]
            tray: None,
            #[cfg(windows)]
            autostart: win::autostart(),
        };
        if let Some(k) = keystore::load() {
            a.start(k);
        } else {
            a.editing = true;
        }
        a
    }
    fn start(&mut self, key: String) {
        if let Some(mut r) = self.running.take() {
            r.stop();
        }
        *self.status.lock().unwrap() = session::Status::default();
        self.running = Some(Running::start(key, self.status.clone(), self.report.clone()));
    }
    fn save_key(&mut self) {
        let k = self.key_input.trim().to_string();
        if !config::key_looks_valid(&k) {
            self.msg = Some((false, self.t.s("That isn't a telemetry key. Copy it again from the crew app: Profile > Telemetry key.",
                "Αυτό δεν είναι κλειδί τηλεμετρίας. Αντέγραψέ το ξανά από την εφαρμογή: Προφίλ > Κλειδί τηλεμετρίας.").into()));
            return;
        }
        match keystore::save(&k) {
            Ok(()) => {
                self.key_input.clear();
                self.editing = false;
                self.msg = None;
                self.start(k);
            }
            Err(e) => self.msg = Some((false, e)),
        }
    }
    fn save_report(&mut self) {
        let txt = self.report.lock().unwrap().text(config::VERSION, cpu_percent());
        let base = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(std::path::PathBuf::from).unwrap_or_default();
        let desk = base.join("Desktop");
        let path = if desk.is_dir() { desk } else { base }.join("ARION Pit Wall test report.txt");
        self.msg = Some(match std::fs::write(&path, txt) {
            Ok(()) => (true, format!("{} {}", self.t.s("Saved:", "Αποθηκεύτηκε:"), path.display())),
            Err(e) => (false, e.to_string()),
        });
    }
}

impl Default for AgentApp {
    fn default() -> Self {
        Self::new()
    }
}

const GREEN: Color32 = Color32::from_rgb(59, 209, 111);
const AMBER: Color32 = Color32::from_rgb(255, 176, 32);
const RED: Color32 = Color32::from_rgb(255, 90, 90);
const GREY: Color32 = Color32::from_rgb(140, 143, 152);
const YELLOW: Color32 = Color32::from_rgb(242, 194, 0);

/// A status line: a coloured dot (drawn, so it never depends on a font) and wrapped text.
fn line(ui: &mut egui::Ui, c: Color32, text: &str) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 16.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 4.5, c);
        ui.add(egui::Label::new(text).wrap());
    });
}

/// The database's own messages, in the driver's language.
fn says(t: &T, e: &str) -> String {
    match e {
        "No connection to the ARION server." => t.s(e, "Δεν υπάρχει σύνδεση με τον server του ARION.").into(),
        "Unknown key." => t.s("Unknown key. Make a new one in the crew app and paste it here.",
            "Άγνωστο κλειδί. Φτιάξε νέο στην εφαρμογή και επικόλλησέ το εδώ.").into(),
        "Too many requests. Wait a minute." => t.s(e, "Πάρα πολλά αιτήματα. Περίμενε ένα λεπτό.").into(),
        _ => e.into(),
    }
}

impl eframe::App for AgentApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(windows)]
        {
            if self.tray.is_none() {
                self.tray = win::tray(ctx.clone(), self.t.el);
            }
            while let Some(ev) = self.tray.as_ref().and_then(|t| t.rx.try_recv().ok()) {
                match ev {
                    win::Ev::Show => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    win::Ev::Quit => {
                        self.quitting = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
            // The window's ✕ hides it to the tray; streaming goes on. Quit is in the tray menu.
            if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.tray.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
        #[cfg(not(windows))]
        let _ = (ctx, self.quitting);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().request_repaint_after(Duration::from_millis(500));
        let st = self.status.lock().unwrap().clone();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("ARION").strong().size(22.0).color(YELLOW));
                ui.label(RichText::new("Pit Wall").size(18.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("v{}", config::VERSION)).color(GREY));
                });
            });
            ui.add_space(8.0);
            if self.editing {
                ui.label(self.t.s("Telemetry key", "Κλειδί τηλεμετρίας"));
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.key_input).password(true).desired_width(260.0).hint_text("arion_tk_…"));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button(self.t.s("Save", "Αποθήκευση")).clicked() || enter {
                        self.save_key();
                    }
                });
                ui.label(RichText::new(self.t.s("From the crew app: Profile > Telemetry key > Make key > Copy.",
                    "Από την εφαρμογή: Προφίλ > Κλειδί τηλεμετρίας > Δημιουργία κλειδιού > Αντιγραφή.")).color(GREY));
                if self.running.is_some() && ui.button(self.t.s("Cancel", "Άκυρο")).clicked() {
                    self.editing = false;
                    self.key_input.clear();
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(self.t.s("Key saved on this PC", "Το κλειδί είναι αποθηκευμένο σε αυτόν τον υπολογιστή")).color(GREEN));
                    if ui.small_button(self.t.s("Change key", "Αλλαγή κλειδιού")).clicked() {
                        self.editing = true;
                        self.msg = None;
                    }
                });
            }
            ui.separator();
            if self.running.is_some() {
                match st.state.as_str() {
                    "online" => {
                        let car = st.team.as_ref().map(|t| format!(" · {} ({})", t.name, t.class)).unwrap_or_else(|| self.t.s(" · practice", " · εξάσκηση").into());
                        line(ui, GREEN, &format!("{} {}{}", self.t.s("Connected as", "Συνδεδεμένος ως"), st.name, car));
                    }
                    "error" => line(ui, RED, &says(&self.t, &st.error.clone().unwrap_or_default())),
                    _ => line(ui, AMBER, self.t.s("Connecting…", "Σύνδεση…")),
                }
                if st.lmu {
                    line(ui, GREEN, self.t.s("Le Mans Ultimate: on track", "Le Mans Ultimate: στην πίστα"));
                } else {
                    line(ui, GREY, self.t.s("Waiting for Le Mans Ultimate", "Αναμονή για το Le Mans Ultimate"));
                }
                if st.viewers > 0 {
                    line(ui, GREEN, &format!("{} {} / {}", self.t.s("Coaches watching:", "Προπονητές που βλέπουν:"), st.viewers, config::MAX_VIEWERS));
                } else {
                    line(ui, GREY, self.t.s("No coach watching", "Κανένας προπονητής δεν βλέπει"));
                }
            }
            ui.add_space(6.0);
            #[cfg(windows)]
            {
                let mut a = self.autostart;
                if ui.checkbox(&mut a, self.t.s("Start with Windows (in the tray)", "Εκκίνηση με τα Windows (στο tray)")).changed() {
                    if win::set_autostart(a) { self.autostart = a; }
                }
                ui.label(RichText::new(self.t.s("Closing this window keeps the app running in the tray. To stop it: tray icon > Quit.",
                    "Το κλείσιμο του παραθύρου αφήνει την εφαρμογή να τρέχει στο tray. Για διακοπή: εικονίδιο tray > Έξοδος.")).color(GREY));
            }
            if ui.button(self.t.s("Save test report", "Αποθήκευση αναφοράς δοκιμής")).clicked() {
                self.save_report();
            }
            if let Some((ok, m)) = &self.msg {
                ui.label(RichText::new(m).color(if *ok { GREEN } else { RED }));
            }
        });
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(mut r) = self.running.take() {
            r.stop(); // says bye to the database and closes the coaches' connections
        }
    }
}

/// This app's average CPU use since it started, in % of one core (Windows).
pub fn cpu_percent() -> Option<f64> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u).ok()?;
        let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64 / 1e7;
        let now = windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime();
        let wall = t(now) - t(c);
        return (wall > 0.0).then(|| (t(k) + t(u)) / wall * 100.0);
    }
    #[allow(unreachable_code)]
    None
}

/// 32×32 ARION icon: yellow ring and dot on black (drawn here, no image file needed).
pub fn icon_rgba() -> (Vec<u8>, u32, u32) {
    let n = 32u32;
    let mut v = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
            let d = (dx * dx + dy * dy).sqrt();
            let i = ((y * n + x) * 4) as usize;
            let px: [u8; 4] = if d > 15.5 { [0, 0, 0, 0] } else if (10.0..13.5).contains(&d) || d < 4.5 { [242, 194, 0, 255] } else { [14, 15, 17, 255] };
            v[i..i + 4].copy_from_slice(&px);
        }
    }
    (v, n, n)
}

/// Windows only: tray icon (Show / Quit) and "Start with Windows" (HKCU Run, no admin rights needed).
#[cfg(windows)]
pub mod win {
    use std::sync::mpsc::{channel, Receiver};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ};

    pub enum Ev { Show, Quit }
    pub struct Tray {
        _icon: TrayIcon,
        pub rx: Receiver<Ev>,
    }

    pub fn tray(ctx: eframe::egui::Context, el: bool) -> Option<Tray> {
        let show = MenuItem::new(if el { "Εμφάνιση" } else { "Show" }, true, None);
        let quit = MenuItem::new(if el { "Έξοδος (σταματά η αποστολή)" } else { "Quit (stops sending)" }, true, None);
        let menu = Menu::new();
        menu.append(&show).ok()?;
        menu.append(&quit).ok()?;
        let (rgba, w, h) = super::icon_rgba();
        let icon = tray_icon::Icon::from_rgba(rgba, w, h).ok()?;
        let tray = TrayIconBuilder::new().with_menu(Box::new(menu)).with_icon(icon).with_tooltip("ARION Pit Wall").build().ok()?;
        let (tx, rx) = channel();
        let (show_id, quit_id) = (show.id().clone(), quit.id().clone());
        let (tx2, ctx2) = (tx.clone(), ctx.clone());
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if e.id == show_id { let _ = tx.send(Ev::Show); }
            if e.id == quit_id { let _ = tx.send(Ev::Quit); }
            ctx.request_repaint();
        }));
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            let show = match e {
                TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => true,
                TrayIconEvent::DoubleClick { .. } => true,
                _ => false,
            };
            if show { let _ = tx2.send(Ev::Show); ctx2.request_repaint(); }
        }));
        Some(Tray { _icon: tray, rx })
    }

    const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    const NAME: PCWSTR = w!("ARION Pit Wall");

    pub fn autostart() -> bool {
        let mut size = 0u32;
        unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN, NAME, RRF_RT_REG_SZ, None, None, Some(&mut size)).is_ok() }
    }
    pub fn set_autostart(on: bool) -> bool {
        unsafe {
            if !on {
                return RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, NAME).is_ok() || !autostart();
            }
            let Ok(exe) = std::env::current_exe() else { return false };
            let cmd = HSTRING::from(format!("\"{}\" --tray", exe.display()));
            let bytes = std::slice::from_raw_parts(cmd.as_ptr() as *const u8, (cmd.len() + 1) * 2);
            RegSetKeyValueW(HKEY_CURRENT_USER, RUN, NAME, REG_SZ.0, Some(bytes.as_ptr() as *const _), bytes.len() as u32).is_ok()
        }
    }
}

pub fn run() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: {
            let (rgba, width, height) = icon_rgba();
            egui::ViewportBuilder::default()
                .with_inner_size([480.0, 250.0])
                .with_min_inner_size([440.0, 220.0])
                .with_title("ARION Pit Wall")
                .with_icon(std::sync::Arc::new(egui::IconData { rgba, width, height }))
                .with_visible(!std::env::args().any(|a| a == "--tray"))
        },
        ..Default::default()
    };
    eframe::run_native("ARION Pit Wall", opts, Box::new(|_cc| Ok(Box::new(AgentApp::new()))))
}
