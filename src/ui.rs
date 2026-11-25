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
    fn start(key: String, status: Arc<Mutex<session::Status>>, report: Arc<Mutex<Report>>, local: crate::local::Local, demo: Option<Arc<std::sync::atomic::AtomicU32>>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let sp = stop.clone();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("runtime");
            let door: Arc<dyn signal::Door> = Arc::new(signal::Supabase::new(config::SUPABASE_URL, config::SUPABASE_KEY, &key));
            let src: Box<dyn source::Source> = match &demo { Some(sp) => Box::new(source::FakeSource::demo(sp.clone())), None => source::game() };
            rt.block_on(session::run(door, src, status, report, local, sp, demo.is_some()));
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

#[derive(Default)]
struct UpState { busy: bool, err: Option<String>, restart: bool }

pub struct AgentApp {
    t: T,
    key_input: String,
    editing: bool,
    msg: Option<(bool, String)>,
    status: Arc<Mutex<session::Status>>,
    report: Arc<Mutex<Report>>,
    running: Option<Running>,
    local: crate::local::Local,
    /// demo mode: fake car instead of LMU; its speed (100 = 1×)
    demo: Option<Arc<std::sync::atomic::AtomicU32>>,
    quitting: bool,
    /// v0.1.16: «Update now» in progress / its error / done (restart and close)
    up: Arc<Mutex<UpState>>,
    /// v0.1.28: the tray's «Check for updates»: its result and when it was asked (the line shows for 2 minutes)
    chk: Arc<Mutex<Option<(crate::update::Manual, std::time::Instant)>>>,
    #[cfg(windows)]
    tip_set: bool,
    wordmark: Option<egui::TextureHandle>,
    #[cfg(windows)]
    tray: Option<win::Tray>,
    #[cfg(windows)]
    autostart: bool,
}

impl AgentApp {
    pub fn new() -> Self {
        Self::with_demo(false)
    }
    /// download, check, swap, start the new version; the window closes this one once it has started
    fn start_update(&mut self, rel: crate::update::Release) {
        let up = self.up.clone();
        *up.lock().unwrap() = UpState { busy: true, ..Default::default() };
        std::thread::spawn(move || {
            let r = crate::update::install(&rel).and_then(|exe| crate::update::restart(&exe));
            let mut u = up.lock().unwrap();
            u.busy = false;
            match r { Ok(()) => u.restart = true, Err(e) => u.err = Some(e) }
        });
    }
    pub fn with_demo(demo: bool) -> Self {
        let mut a = AgentApp {
            t: T { el: greek() },
            key_input: String::new(),
            editing: false,
            msg: None,
            status: Arc::new(Mutex::new(session::Status::default())),
            report: Arc::new(Mutex::new(Report::default())),
            running: None,
            local: crate::local::Local::new(),
            demo: demo.then(|| Arc::new(std::sync::atomic::AtomicU32::new(100))),
            quitting: false,
            up: Arc::new(Mutex::new(UpState::default())),
            chk: Arc::new(Mutex::new(None)),
            #[cfg(windows)]
            tip_set: false,
            wordmark: None,
            #[cfg(windows)]
            tray: None,
            #[cfg(windows)]
            autostart: win::autostart(),
        };
        // look for a newer version now and every 6 hours (only tells; updating is the driver's click)
        std::thread::spawn(|| loop {
            let _ = crate::update::check(config::VERSION);
            std::thread::sleep(crate::update::EVERY);
        });
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
        self.running = Some(Running::start(key, self.status.clone(), self.report.clone(), self.local.clone(), self.demo.clone()));
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
        // the new version is in place and started: close this one (it waits for us to free the dashboard port)
        if self.up.lock().unwrap().restart && !self.quitting {
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        #[cfg(windows)]
        {
            if !self.tip_set {
                if let (Some(t), Some(r)) = (self.tray.as_ref(), crate::update::available()) {
                    let _ = t.icon.set_tooltip(Some(format!("ARION Pit Wall · {} v{}", self.t.s("update available:", "διαθέσιμη ενημέρωση:"), r.version)));
                    self.tip_set = true;
                }
            }
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
                    win::Ev::Check => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        let busy = matches!(self.chk.lock().unwrap().as_ref(), Some((crate::update::Manual::Checking, _)));
                        if !busy {
                            *self.chk.lock().unwrap() = Some((crate::update::Manual::Checking, std::time::Instant::now()));
                            let (chk, c2) = (self.chk.clone(), ctx.clone());
                            std::thread::spawn(move || {
                                let m = crate::update::manual(config::VERSION);
                                *chk.lock().unwrap() = Some((m, std::time::Instant::now()));
                                c2.request_repaint();
                            });
                        }
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
        let mut used_bottom = 0.0f32;
        egui::CentralPanel::default().show(ui, |ui| {
            let wm = self
                .wordmark
                .get_or_insert_with(|| {
                    ui.ctx().load_texture("arion-wordmark", egui::ColorImage::from_rgba_unmultiplied(WORDMARK_SIZE, WORDMARK), egui::TextureOptions::LINEAR)
                })
                .clone();
            ui.horizontal(|ui| {
                ui.add(egui::Image::new(&wm).fit_to_exact_size(egui::vec2(WORDMARK_SIZE[0] as f32 / 2.0, WORDMARK_SIZE[1] as f32 / 2.0)));
                ui.label(RichText::new("Pit Wall").strong().size(16.0).color(Color32::from_rgb(245, 245, 247)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("v{}", config::VERSION)).color(GREY));
                });
            });
            if let Some(sp) = &self.demo {
                ui.add_space(4.0);
                egui::Frame::new().fill(Color32::from_rgb(42, 31, 8)).stroke(egui::Stroke::new(1.0, AMBER)).corner_radius(6.0).inner_margin(6.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.t.s("DEMO · fake car, not Le Mans Ultimate", "DEMO · ψεύτικο αυτοκίνητο, όχι Le Mans Ultimate")).color(AMBER).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let cur = sp.load(std::sync::atomic::Ordering::Relaxed);
                            if ui.selectable_label(cur == 1000, "10×").clicked() { sp.store(1000, std::sync::atomic::Ordering::Relaxed) }
                            if ui.selectable_label(cur == 100, "1×").clicked() { sp.store(100, std::sync::atomic::Ordering::Relaxed) }
                        });
                    });
                });
            }
            ui.add_space(8.0);
            if self.editing {
                ui.label(self.t.s("Telemetry key", "Κλειδί τηλεμετρίας"));
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.key_input).password(true).desired_width(200.0).hint_text("arion_tk_…"));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    // One click for drivers who don't use Ctrl+V: paste from the clipboard and save.
                    if ui.button(self.t.s("Paste", "Επικόλληση")).clicked() {
                        match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                            Ok(t) => {
                                self.key_input = t.trim().to_string();
                                self.save_key();
                            }
                            Err(_) => self.msg = Some((false, self.t.s("The clipboard is empty. Copy the key in the crew app first.",
                                "Το πρόχειρο είναι άδειο. Αντέγραψε πρώτα το κλειδί από την εφαρμογή.").into())),
                        }
                    }
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
                if st.lmu && self.demo.is_some() {
                    line(ui, AMBER, self.t.s("Demo car: on track", "Demo αυτοκίνητο: στην πίστα"));
                } else if st.lmu {
                    line(ui, GREEN, self.t.s("Le Mans Ultimate: on track", "Le Mans Ultimate: στην πίστα"));
                } else {
                    line(ui, GREY, self.t.s("Waiting for Le Mans Ultimate", "Αναμονή για το Le Mans Ultimate"));
                }
                // "My dashboard": the driver's own view on this PC (not one of the 5 coach places)
                ui.horizontal(|ui| {
                    let mine = self.local.watching();
                    let txt = if mine > 0 { format!("{} {mine}", self.t.s("My dashboard open:", "Το dashboard μου ανοιχτό:")) } else { self.t.s("My dashboard: on this PC", "Το dashboard μου: σε αυτόν τον υπολογιστή").to_string() };
                    line(ui, if mine > 0 { GREEN } else { GREY }, &txt);
                    if let Some(u) = self.local.url() {
                        if ui.small_button(self.t.s("Open my dashboard", "Άνοιγμα του dashboard μου")).clicked() {
                            open_url(&u);
                        }
                    }
                });
                if st.viewers > 0 {
                    line(ui, GREEN, &format!("{} {} / {}", self.t.s("Coaches watching:", "Προπονητές που βλέπουν:"), st.viewers, config::MAX_VIEWERS));
                } else {
                    line(ui, GREY, self.t.s("No coach watching", "Κανένας προπονητής δεν βλέπει"));
                }
            }
            // v0.1.28: the tray's «Check for updates»: checking… / latest / couldn't check (a newer version: the line below)
            let shown = self.chk.lock().unwrap().as_ref().and_then(|(m, at)| (matches!(m, crate::update::Manual::Checking) || at.elapsed().as_secs() < 120).then(|| m.clone()));
            if let Some((k, t)) = shown.as_ref().and_then(|m| crate::update::manual_text(m, self.t.el)) {
                line(ui, [GREY, GREEN, RED][k as usize], &t);
            }
            // v0.1.16: a newer version: «Update now» only between sessions (LMU not running, no coach watching)
            if let Some(rel) = crate::update::available() {
                let live = (st.lmu && self.demo.is_none()) || st.viewers > 0;
                let (busy, err) = { let u = self.up.lock().unwrap(); (u.busy, u.err.clone()) };
                ui.horizontal(|ui| {
                    line(ui, AMBER, &format!("{} v{}", self.t.s("Update available:", "Διαθέσιμη ενημέρωση:"), rel.version));
                    if busy {
                        ui.label(RichText::new(self.t.s("updating…", "ενημέρωση…")).color(GREY));
                    } else if live {
                        ui.label(RichText::new(self.t.s("after the session", "μετά τη συνεδρία")).color(GREY));
                    } else if ui.small_button(self.t.s("Update now", "Ενημέρωση τώρα")).clicked() {
                        self.start_update(rel.clone());
                    }
                    if ui.small_button(self.t.s("What's new", "Τι νέο")).clicked() {
                        open_url(&rel.page);
                    }
                });
                if let Some(e) = err {
                    line(ui, RED, &e);
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
            // where the next widget would go = the end of the contents (the panel's own rect always fills the window)
            used_bottom = ui.cursor().top() - ui.spacing().item_spacing.y;
        });
        // v0.1.16: the window is as tall as its contents (demo bar, error lines, the longer Greek), so no text is ever cut off.
        // Only the height follows; the width stays where the user put it, and the wrapped text decides the height.
        let ctx = ui.ctx().clone();
        if let Some(r) = ctx.input(|i| i.viewport().inner_rect) {
            let want = (used_bottom + 8.0).ceil();   // + the panel's bottom margin
            if want > 60.0 && (want - r.height()).abs() > 1.5 {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(r.width(), want)));
            }
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(mut r) = self.running.take() {
            r.stop(); // says bye to the database and closes the coaches' connections
        }
    }
}

/// Opens a web address in the default browser.
pub fn open_url(u: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(u).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(u).spawn();
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

/// The ARION icon (from the team's official logo), 64×64 RGBA, for the window and the tray.
pub fn icon_rgba() -> (Vec<u8>, u32, u32) {
    (include_bytes!("../assets/icon64.rgba").to_vec(), 64, 64)
}
/// The ARION wordmark as in the crew app's navbar (white), 261×50 RGBA, shown at half size.
const WORDMARK: &[u8] = include_bytes!("../assets/wordmark.rgba");
const WORDMARK_SIZE: [usize; 2] = [261, 50];

/// Windows only: tray icon (Show / Check for updates / Quit) and "Start with Windows" (HKCU Run, no admin rights needed).
#[cfg(windows)]
pub mod win {
    use std::sync::mpsc::{channel, Receiver};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ};

    pub enum Ev { Show, Check, Quit }
    pub struct Tray {
        pub icon: TrayIcon,
        pub rx: Receiver<Ev>,
    }

    pub fn tray(ctx: eframe::egui::Context, el: bool) -> Option<Tray> {
        let show = MenuItem::new(if el { "Εμφάνιση" } else { "Show" }, true, None);
        let check = MenuItem::new(if el { "Έλεγχος για ενημερώσεις" } else { "Check for updates" }, true, None);
        let quit = MenuItem::new(if el { "Έξοδος (σταματά η αποστολή)" } else { "Quit (stops sending)" }, true, None);
        let menu = Menu::new();
        menu.append(&show).ok()?;
        menu.append(&check).ok()?;
        menu.append(&PredefinedMenuItem::separator()).ok()?;
        menu.append(&quit).ok()?;
        let (rgba, w, h) = super::icon_rgba();
        let icon = tray_icon::Icon::from_rgba(rgba, w, h).ok()?;
        let tray = TrayIconBuilder::new().with_menu(Box::new(menu)).with_icon(icon).with_tooltip("ARION Pit Wall").build().ok()?;
        let (tx, rx) = channel();
        let (show_id, check_id, quit_id) = (show.id().clone(), check.id().clone(), quit.id().clone());
        let (tx2, ctx2) = (tx.clone(), ctx.clone());
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if e.id == show_id { let _ = tx.send(Ev::Show); }
            if e.id == check_id { let _ = tx.send(Ev::Check); }
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
        Some(Tray { icon: tray, rx })
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

pub fn run(demo: bool) -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: {
            let (rgba, width, height) = icon_rgba();
            egui::ViewportBuilder::default()
                .with_inner_size([480.0, 250.0])
                .with_min_inner_size([440.0, 120.0])
                .with_title(if demo { "ARION Pit Wall – DEMO" } else { "ARION Pit Wall" })
                .with_icon(std::sync::Arc::new(egui::IconData { rgba, width, height }))
                .with_visible(!std::env::args().any(|a| a == "--tray"))
        },
        ..Default::default()
    };
    eframe::run_native("ARION Pit Wall", opts, Box::new(move |_cc| Ok(Box::new(AgentApp::with_demo(demo)))))
}
