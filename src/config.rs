//! Public settings. Nothing here is secret: the Supabase address and its publishable key are the same
//! ones the crew app's web page sends to every visitor. Access is protected by the database rules and by
//! each driver's telemetry key, which is typed in by the driver and never stored in this program.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SUPABASE_URL: &str = "https://isuiksufkyishdowejob.supabase.co";
pub const SUPABASE_KEY: &str = "sb_publishable_DMqGCrW9YEmzEN2_cP67-w_ljr1JBo9";
/// Free public STUN server (finds the PC's public address; no account, no data relayed).
pub const STUN_URL: &str = "stun:stun.cloudflare.com:3478";
/// Coaches that can watch one driver at the same time.
pub const MAX_VIEWERS: usize = 5;
/// How often the app asks the database for new coaches (seconds). 30 calls a minute, the limit is 120.
pub const POLL_SECS: u64 = 2;
/// Fast telemetry rate sent to coaches.
pub const FAST_HZ: u32 = 30;
/// Slow data rate (tyres, energy, laps, all cars).
pub const SLOW_HZ: u32 = 2;

/// A telemetry key as the crew app makes it: "arion_tk_" + 64 lowercase hex characters.
pub fn key_looks_valid(k: &str) -> bool {
    let k = k.trim();
    k.len() == 73 && k.starts_with("arion_tk_") && k[9..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn key_format() {
        assert!(key_looks_valid(&format!("arion_tk_{}", "0a".repeat(32))));
        assert!(key_looks_valid(&format!("  arion_tk_{}\n", "f".repeat(64))));
        assert!(!key_looks_valid(&format!("arion_tk_{}", "F".repeat(64))));
        assert!(!key_looks_valid(&format!("arion_tk_{}", "0".repeat(63))));
        assert!(!key_looks_valid("hello"));
    }
}
