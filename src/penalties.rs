//! v0.1.26: each car's penalties still to serve, with their type («Drive Thru», «Stop/Go», …).
//!
//! Shared memory gives only a count per car (`mNumPenalties`, «number of outstanding penalties»). The type is in LMU's
//! results stream (`scoring.scoringStream`), as XML-like lines (format as TinyPedal reads it, not yet seen by us):
//!   issued: `<Penalty Driver="NAME" … Penalty="Drive Thru" …>`
//!   served: `<Penalty …>NAME served a Drive Thru …`
//! The stream may hold only the newest lines or keep older ones, so every line is handled once (by its full text) per
//! session. LMU's count stays in charge: if we missed a line (the app started mid-race, a name differs), the list is
//! trimmed (oldest first, as they are served in order) or filled with «Penalty» (type unknown) to match the count.
//! Raw `<Penalty` lines go to `penalties.log` in the app's folder, so the real format can be checked.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;

use crate::lmu::layout::LMUObjectOut;
use crate::protocol::cstr;

fn cp<T: Copy>(x: T) -> T {
    x
}

pub const UNKNOWN: &str = "Penalty";
const LOG_MAX: u64 = 1 << 20;

#[derive(Default)]
pub struct Penalties {
    seen: HashSet<String>,
    open: HashMap<String, Vec<String>>,
    last_stream: Vec<u8>,
    session: Option<(i32, f64)>,
    log: Option<PathBuf>,
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}
fn attr(line: &str, name: &str) -> Option<String> {
    let k = format!(" {name}=\"");
    let a = line.find(&k)? + k.len();
    let b = line[a..].find('"')? + a;
    Some(unescape(&line[a..b]))
}
/// drive-through and stop/go are told apart by their words; anything else by its exact text
fn family(t: &str) -> String {
    let l = t.to_lowercase();
    if l.contains("drive") { "dt".into() } else if l.contains("stop") { "sg".into() } else { l }
}

impl Penalties {
    pub fn new() -> Self {
        Self::default()
    }
    /// Also write the raw `<Penalty` lines to this file (at most 1 MB).
    pub fn with_log(path: PathBuf) -> Self {
        Self { log: Some(path), ..Self::default() }
    }

    fn note(&self, line: &str) {
        let Some(p) = &self.log else { return };
        if std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) > LOG_MAX { return }
        if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "{line}");
        }
    }

    /// One `<Penalty` line, handled once per session.
    pub fn line(&mut self, line: &str) {
        let line = line.trim();
        if !line.starts_with("<Penalty") || !self.seen.insert(line.to_string()) { return }
        self.note(line);
        if let (Some(d), Some(p)) = (attr(line, "Driver"), attr(line, "Penalty")) {
            if !d.is_empty() && !p.is_empty() { self.open.entry(d).or_default().push(p) }
            return;
        }
        // served: the name between the first '>' and « served»
        let Some(a) = line.find('>') else { return };
        let Some(b) = line[a + 1..].find(" served").map(|x| x + a + 1) else { return };
        let name = unescape(line[a + 1..b].trim());
        let what = family(&line[b..]);
        if let Some(v) = self.open.get_mut(&name) {
            let i = v.iter().position(|p| what.contains(&family(p)) || family(p) == what).unwrap_or(0);
            if i < v.len() { v.remove(i); }
            if v.is_empty() { self.open.remove(&name); }
        }
    }

    /// Look at the newest data: a new session starts afresh; new stream lines are read; each car's list follows its count.
    pub fn observe(&mut self, o: &LMUObjectOut) {
        let si = cp(o.scoring.scoringInfo);
        let (sess, et) = (cp(si.mSession), cp(si.mCurrentET));
        if let Some((s, t)) = self.session { if s != sess || et + 1.0 < t { self.seen.clear(); self.open.clear(); self.last_stream.clear(); } }
        self.session = Some((sess, et));
        let st = &o.scoring.scoringStream;
        let end = st.iter().position(|&b| b == 0).unwrap_or(st.len());
        if st[..end] != self.last_stream[..] {
            self.last_stream = st[..end].to_vec();
            let text = String::from_utf8_lossy(&st[..end]).into_owned();
            for l in text.split('\n') { self.line(l) }
        }
        let n = (cp(si.mNumVehicles).max(0) as usize).min(o.scoring.vehScoringInfo.len());
        for c in &o.scoring.vehScoringInfo[..n] {
            let count = cp(c.mNumPenalties).max(0) as usize;
            let name = cstr(&cp(c.mDriverName));
            self.trim(&name, count);
        }
    }

    fn trim(&mut self, name: &str, count: usize) {
        if count == 0 { self.open.remove(name); return }
        let v = self.open.entry(name.to_string()).or_default();
        if v.len() > count { v.drain(..v.len() - count); }
        while v.len() < count { v.push(UNKNOWN.to_string()) }
    }

    /// This driver's penalties still to serve, oldest first, in LMU's words.
    pub fn of(&self, name: &str) -> Vec<String> {
        self.open.get(name).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn issued_and_served_by_type() {
        let mut p = Penalties::new();
        p.line(r#"<Penalty Driver="M Rossi" ID="4" Penalty="Drive Thru" Reason="Speeding" et="100.0">M Rossi received a Drive Thru</Penalty>"#);
        p.line(r#"<Penalty Driver="M Rossi" ID="4" Penalty="Stop/Go" et="110.0">M Rossi received a Stop/Go</Penalty>"#);
        assert_eq!(p.of("M Rossi"), vec!["Drive Thru", "Stop/Go"]);
        p.line(r#"<Penalty et="140.0">M Rossi served a Stop/Go penalty</Penalty>"#);
        assert_eq!(p.of("M Rossi"), vec!["Drive Thru"], "the stop/go is removed, not the older drive-through");
        p.line(r#"<Penalty et="140.0">M Rossi served a Stop/Go penalty</Penalty>"#);
        assert_eq!(p.of("M Rossi"), vec!["Drive Thru"], "the same line twice counts once");
        p.line(r#"<Penalty et="190.0">M Rossi served a Drive Thru penalty</Penalty>"#);
        assert!(p.of("M Rossi").is_empty());
    }
    #[test]
    fn names_with_escapes_and_unknown_types() {
        let mut p = Penalties::new();
        p.line(r#"<Penalty Driver="A &amp; B#1358" Penalty="Time 5s">x</Penalty>"#);
        assert_eq!(p.of("A & B#1358"), vec!["Time 5s"]);
        p.line(r#"<Penalty>A &amp; B#1358 served a Time 5s penalty</Penalty>"#);
        assert!(p.of("A & B#1358").is_empty());
        p.line("<Incident>not a penalty</Incident>");
        p.line(r#"<Penalty>Nobody served a Drive Thru</Penalty>"#);
        assert!(p.open.is_empty(), "other lines and unknown drivers change nothing");
    }
    #[test]
    fn the_fake_race_through_a_whole_cycle() {
        use crate::source::{FakeSource, Source};
        let mut f = FakeSource::with_speed(400.0);
        f.pens = true;
        let mut p = Penalties::new();
        let mut seen: Vec<(String, Vec<String>, Vec<String>)> = vec![];
        let t0 = std::time::Instant::now();
        while t0.elapsed() < std::time::Duration::from_millis(1100) {
            let o = f.read().unwrap();
            p.observe(&o);
            let et = { o.scoring.scoringInfo.mCurrentET } % 360.0;
            let row = (format!("{:.0}", (et / 20.0).floor() * 20.0), p.of("Driver 6"), p.of("Driver 4"));
            if seen.last().map(|r| r.0 != row.0).unwrap_or(true) { seen.push(row) }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let at = |k: &str| seen.iter().find(|r| r.0 == k).cloned();
        let (_, a, b) = at("100").expect("reached 1:40");
        assert_eq!((a, b), (vec!["Stop/Go".to_string()], vec![]), "1:40: car 6 has its stop/go, you have nothing");
        let (_, a, b) = at("160").expect("reached 2:40");
        assert_eq!((a, b), (vec!["Stop/Go".to_string()], vec!["Drive Thru".to_string()]), "2:40: both, by type");
        let (_, a, b) = at("240").expect("reached 4:00");
        assert_eq!((a, b), (vec![], vec!["Drive Thru".to_string()]), "4:00: car 6 served");
        let (_, a, b) = at("340").expect("reached 5:40");
        assert!(a.is_empty() && b.is_empty(), "5:40: all served");
    }
    #[test]
    fn the_count_decides_how_many() {
        let mut p = Penalties::new();
        p.trim("X", 2);
        assert_eq!(p.of("X"), vec![UNKNOWN, UNKNOWN], "missed lines: type unknown, count right");
        p.line(r#"<Penalty Driver="Y" Penalty="Drive Thru">.</Penalty>"#);
        p.line(r#"<Penalty Driver="Y" Penalty="Stop/Go">.</Penalty>"#);
        p.trim("Y", 1);
        assert_eq!(p.of("Y"), vec!["Stop/Go"], "a missed «served» line: the oldest goes");
        p.trim("Y", 0);
        assert!(p.of("Y").is_empty());
    }
}
