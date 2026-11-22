//! "My dashboard": the driver's own dashboard, served by this app on this PC only.
//!   http://127.0.0.1:<port>/        the dashboard page (built into the app)
//!   http://127.0.0.1:<port>/events  live data as Server-Sent Events: "fast" (base64 binary frames) and "slow" (JSON)
//! Listens on 127.0.0.1 only (never the network) and answers only requests addressed to 127.0.0.1/localhost,
//! so a web page elsewhere can't read it. No database, no login, doesn't count toward the 5 coaches.

use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

pub const FIRST_PORT: u16 = 6420;
const PAGE: &str = include_str!("../assets/dashboard.html");

#[derive(Clone, Debug)]
pub enum Msg {
    Fast(Vec<u8>),
    Slow(String),
}

#[derive(Clone)]
pub struct Local {
    pub tx: broadcast::Sender<Msg>,
    pub port: Arc<AtomicU16>,
    pub watching: Arc<AtomicUsize>,
}

impl Local {
    pub fn new() -> Self {
        Local { tx: broadcast::channel(256).0, port: Arc::new(AtomicU16::new(0)), watching: Arc::new(AtomicUsize::new(0)) }
    }
    pub fn watching(&self) -> usize {
        self.watching.load(Ordering::Relaxed)
    }
    pub fn send(&self, m: Msg) {
        if self.watching() > 0 {
            let _ = self.tx.send(m);
        }
    }
    pub fn url(&self) -> Option<String> {
        match self.port.load(Ordering::Relaxed) {
            0 => None,
            p => Some(format!("http://127.0.0.1:{p}/")),
        }
    }
}

impl Default for Local {
    fn default() -> Self {
        Self::new()
    }
}

/// The dashboard page in local mode: told so before its script runs, without the online login library.
pub fn page() -> String {
    page_from(PAGE)
}

fn page_from(src: &str) -> String {
    // Git on Windows may turn the file's line endings into \r\n: work on \n either way.
    let p = src.replace("\r\n", "\n");
    let p = p.replace("<script>\n'use strict';", "<script>window.PW_LOCAL=1;</script>\n<script>\n'use strict';");
    match (p.find("<script src=\"https://cdn.jsdelivr.net/npm/@supabase"), p.find("</script>\n<style>")) {
        (Some(a), Some(b)) if a < b => format!("{}{}", &p[..a], &p[b + "</script>\n".len()..]),
        _ => p,
    }
}

pub fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() { s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char) } else { s.push('=') }
        }
    }
    s
}

fn allowed_host(head: &str, port: u16) -> bool {
    head.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.strip_prefix("host:").map(|h| { let h = h.trim(); h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}") }).unwrap_or(false)
    })
}

/// Start listening (first free port from 6420). Runs until the app closes.
pub async fn serve(local: Local) {
    let mut listener = None;
    for p in FIRST_PORT..FIRST_PORT + 20 {
        if let Ok(l) = TcpListener::bind(("127.0.0.1", p)).await {
            local.port.store(p, Ordering::Relaxed);
            listener = Some(l);
            break;
        }
    }
    let Some(listener) = listener else { return };
    let port = local.port.load(Ordering::Relaxed);
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { continue };
        let local = local.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let mut n = 0;
            while n < buf.len() {
                match sock.read(&mut buf[n..]).await { Ok(0) | Err(_) => return, Ok(k) => n += k }
                if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") { break }
            }
            let head = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
            if !head.starts_with("GET ") || !allowed_host(&head, port) {
                let _ = sock.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                return;
            }
            if path == "/" || path.starts_with("/?") {
                let body = page();
                let h = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len());
                let _ = sock.write_all(h.as_bytes()).await;
                let _ = sock.write_all(body.as_bytes()).await;
                return;
            }
            if path == "/events" {
                if sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\nretry: 1000\n\n").await.is_err() { return }
                let _ = sock.set_nodelay(true);
                let mut rx = local.tx.subscribe();
                local.watching.fetch_add(1, Ordering::Relaxed);
                loop {
                    let m = match tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await {
                        Ok(Ok(m)) => match m { Msg::Fast(b) => format!("event: fast\ndata: {}\n\n", b64(&b)), Msg::Slow(j) => format!("event: slow\ndata: {j}\n\n") },
                        Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                        Ok(Err(_)) => break,
                        Err(_) => ": keep-alive\n\n".to_string(), // also notices a closed tab
                    };
                    if sock.write_all(m.as_bytes()).await.is_err() { break }
                }
                local.watching.fetch_sub(1, Ordering::Relaxed);
                return;
            }
            let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(&[0, 255, 16, 128]), "AP8QgA==");
    }
    #[test]
    fn only_local_hosts() {
        assert!(allowed_host("GET / HTTP/1.1\r\nHost: 127.0.0.1:6420\r\n", 6420));
        assert!(allowed_host("GET / HTTP/1.1\r\nhost: localhost:6420\r\n", 6420));
        assert!(!allowed_host("GET / HTTP/1.1\r\nHost: evil.example:6420\r\n", 6420));
        assert!(!allowed_host("GET / HTTP/1.1\r\n", 6420));
    }
    #[test]
    fn page_runs_in_local_mode_without_the_login_library() {
        let p = page();
        assert!(p.contains("window.PW_LOCAL=1"));
        assert!(!p.contains("<script src=\"https://cdn.jsdelivr.net/npm/@supabase"), "the online login library is left out locally");
        assert!(p.contains("<style>") && p.contains("ARION Pit Wall"), "the rest of the page is intact");
    }
    #[test]
    fn page_works_with_windows_line_endings() {
        let crlf = PAGE.replace("\r\n", "\n").replace('\n', "\r\n");
        let p = page_from(&crlf);
        assert!(p.contains("window.PW_LOCAL=1"), "local mode is switched on");
        assert!(!p.contains("<script src=\"https://cdn.jsdelivr.net/npm/@supabase"), "the online login library is left out");
    }
}
