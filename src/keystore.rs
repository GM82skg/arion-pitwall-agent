//! The telemetry key, kept on this PC only. On Windows it is encrypted with the user's own Windows
//! protection (DPAPI): another Windows user, or a copy of the file on another PC, can't read it.

use std::path::PathBuf;

pub fn dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("ARION Pit Wall")
}
fn file() -> PathBuf {
    dir().join("key.bin")
}

pub fn load() -> Option<String> {
    let b = std::fs::read(file()).ok()?;
    let k = String::from_utf8(unprotect(&b)?).ok()?;
    crate::config::key_looks_valid(&k).then(|| k.trim().to_string())
}

pub fn save(key: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    let b = protect(key.trim().as_bytes()).ok_or("Windows couldn't protect the key.")?;
    std::fs::write(file(), b).map_err(|e| e.to_string())
}

pub fn forget() {
    let _ = std::fs::remove_file(file());
}

#[cfg(windows)]
fn protect(data: &[u8]) -> Option<Vec<u8>> {
    dpapi(data, true)
}
#[cfg(windows)]
fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    dpapi(data, false)
}
#[cfg(windows)]
fn dpapi(data: &[u8], encrypt: bool) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB::default();
        let r = if encrypt {
            CryptProtectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        } else {
            CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        };
        r.ok()?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut core::ffi::c_void)));
        Some(v)
    }
}
// Elsewhere (tests and development only): plain file.
#[cfg(not(windows))]
fn protect(data: &[u8]) -> Option<Vec<u8>> {
    Some(data.to_vec())
}
#[cfg(not(windows))]
fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    Some(data.to_vec())
}
