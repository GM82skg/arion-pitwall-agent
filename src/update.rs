//! Updates (v0.1.16). The app asks GitHub for the newest release at start and every 6 hours. When there is a newer
//! one it says so in the window (and coaches see it on the dashboard). «Update now» is the driver's choice, never
//! automatic, and only between sessions (LMU not running, no coach watching): it downloads the new exe, checks its
//! SHA-256 against the checksum file the release build publishes, swaps it in place of the running file (same name,
//! so «Start with Windows» keeps working) and restarts.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use sha2::{Digest, Sha256};

pub const RELEASES_API: &str = "https://api.github.com/repos/GM82skg/arion-pitwall-agent/releases/latest";
const EXE: &str = "ARION-Pit-Wall.exe";
const SUM: &str = "ARION-Pit-Wall.exe.sha256";
pub const EVERY: Duration = Duration::from_secs(6 * 3600);

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub page: String,
    exe: String,
    sum: String,
}

/// The newest release found by the last check, when it is newer than this app (read by the window and the session).
pub fn available() -> Option<Release> { slot().lock().unwrap().clone() }
fn slot() -> &'static Mutex<Option<Release>> {
    static S: OnceLock<Mutex<Option<Release>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Tests point this at a local stand-in for GitHub.
fn api() -> String { std::env::var("ARION_UPDATE_API").unwrap_or_else(|_| RELEASES_API.into()) }

fn agent(secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(secs))).http_status_as_error(false).build().into()
}

/// "0.1.10" > "0.1.9": compare the numbers, not the text. A leading "v" is ignored.
pub fn newer(candidate: &str, current: &str) -> bool {
    let p = |s: &str| s.trim().trim_start_matches('v').split('.').map(|x| x.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    p(candidate) > p(current)
}

/// Read GitHub's answer: the version and the two files every release has (exe and checksum).
pub fn parse(j: &Value) -> Option<Release> {
    let tag = j["tag_name"].as_str()?;
    let url = |name: &str| j["assets"].as_array()?.iter().find(|a| a["name"] == name)?["browser_download_url"].as_str().map(String::from);
    Some(Release { version: tag.trim_start_matches('v').into(), page: j["html_url"].as_str().unwrap_or_default().into(), exe: url(EXE)?, sum: url(SUM)? })
}

/// One check. Ok(None) = up to date (or the newest release has no checksum file yet: never offered without one).
pub fn check(current: &str) -> Result<Option<Release>, String> {
    let mut r = agent(15).get(&api()).header("User-Agent", &format!("ARION-Pit-Wall/{current}")).header("Accept", "application/vnd.github+json")
        .call().map_err(|e| e.to_string())?;
    if r.status().as_u16() != 200 { return Err(format!("GitHub answered {}", r.status().as_u16())); }
    let j: Value = r.body_mut().read_json().map_err(|e| e.to_string())?;
    let rel = parse(&j).filter(|x| newer(&x.version, current));
    *slot().lock().unwrap() = rel.clone();
    Ok(rel)
}

fn fetch(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut r = agent(300).get(url).header("User-Agent", "ARION-Pit-Wall").call().map_err(|e| e.to_string())?;
    if r.status().as_u16() != 200 { return Err(format!("download answered {}", r.status().as_u16())); }
    r.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| e.to_string())
}

pub fn sha256_hex(b: &[u8]) -> String { Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect() }

/// The checksum file holds the 64 hex characters (anything after them, like a file name, is ignored).
pub fn expected_sum(text: &str) -> Option<String> {
    let h: String = text.trim().chars().take(64).collect::<String>().to_lowercase();
    (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then_some(h)
}

/// Put `new_bytes` in place of the file at `exe` (which may be running: Windows allows renaming a running exe, not
/// overwriting it). The old file becomes `<name>.old` and is removed at the next start. On any failure the old file
/// is put back, so the app is never left without its exe.
pub fn swap(exe: &Path, new_bytes: &[u8]) -> Result<(), String> {
    let new = exe.with_extension("new");
    let old = exe.with_extension("old");
    let _ = std::fs::remove_file(&old);
    std::fs::write(&new, new_bytes).map_err(|e| format!("can't write next to the app ({e}): download it from Releases instead"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755));
    }
    if let Err(e) = std::fs::rename(exe, &old) {
        let _ = std::fs::remove_file(&new);
        return Err(format!("can't replace the app ({e})"));
    }
    if let Err(e) = std::fs::rename(&new, exe) {
        let _ = std::fs::rename(&old, exe);
        let _ = std::fs::remove_file(&new);
        return Err(format!("can't replace the app ({e})"));
    }
    Ok(())
}

/// Download, verify, swap. Returns the path to start again.
pub fn install(rel: &Release) -> Result<PathBuf, String> {
    let sum_text = String::from_utf8(fetch(&rel.sum, 4096)?).map_err(|_| "bad checksum file".to_string())?;
    let want = expected_sum(&sum_text).ok_or("bad checksum file")?;
    let bytes = fetch(&rel.exe, 200 * 1024 * 1024)?;
    let got = sha256_hex(&bytes);
    if got != want { return Err(format!("the download doesn't match its checksum (got {}…, expected {}…): nothing changed", &got[..12], &want[..12])); }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    swap(&exe, &bytes)?;
    Ok(exe)
}

/// Start the new version with the same options, telling it to wait until this one has closed.
pub fn restart(exe: &Path) -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--after-update").collect();
    args.push("--after-update".into());
    std::process::Command::new(exe).args(args).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// At start: remove the file left by the last update, and after an update wait (up to 15 s) for the old app to free
/// its dashboard port, so «My dashboard» and the OBS links stay on 127.0.0.1:6420.
pub fn at_start() {
    if let Ok(exe) = std::env::current_exe() {
        let old = exe.with_extension("old");
        for _ in 0..10 { if !old.exists() || std::fs::remove_file(&old).is_ok() { break } std::thread::sleep(Duration::from_millis(300)) }
    }
    if std::env::args().any(|a| a == "--after-update") {
        for _ in 0..150 {
            if std::net::TcpListener::bind(("127.0.0.1", crate::local::FIRST_PORT)).is_ok() { break }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_compare_as_numbers() {
        assert!(newer("0.1.10", "0.1.9") && newer("v0.2.0", "0.1.99") && newer("1.0.0", "0.9.9"));
        assert!(!newer("0.1.9", "0.1.10") && !newer("0.1.16", "0.1.16") && !newer("v0.1.15", "0.1.16"));
    }
    #[test]
    fn release_needs_exe_and_checksum() {
        let j = serde_json::json!({"tag_name":"v0.1.17","html_url":"https://x/r","assets":[
            {"name":"ARION-Pit-Wall-v0.1.17.exe","browser_download_url":"https://x/v.exe"},
            {"name":"ARION-Pit-Wall.exe","browser_download_url":"https://x/a.exe"},
            {"name":"ARION-Pit-Wall.exe.sha256","browser_download_url":"https://x/a.sha256"}]});
        let r = parse(&j).unwrap();
        assert_eq!((r.version.as_str(), r.exe.as_str(), r.sum.as_str()), ("0.1.17", "https://x/a.exe", "https://x/a.sha256"));
        let mut k = j.clone();
        k["assets"].as_array_mut().unwrap().pop();
        assert!(parse(&k).is_none(), "never offered without a checksum file");
    }
    #[test]
    fn checksum_file_and_hash() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let h = "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD  ARION-Pit-Wall.exe\n";
        assert_eq!(expected_sum(h).unwrap(), sha256_hex(b"abc"));
        assert!(expected_sum("not a hash").is_none() && expected_sum("abc123").is_none());
    }
    #[test]
    fn swap_replaces_and_keeps_the_old_until_next_start() {
        let d = std::env::temp_dir().join(format!("arion-up-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let exe = d.join("ARION-Pit-Wall-v0.1.16.exe");
        std::fs::write(&exe, b"old").unwrap();
        swap(&exe, b"new").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert_eq!(std::fs::read(exe.with_extension("old")).unwrap(), b"old");
        assert!(!exe.with_extension("new").exists());
        // can't write next to the app (folder gone): an error, nothing half-done
        let gone = d.join("nope").join("x.exe");
        assert!(swap(&gone, b"new").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
