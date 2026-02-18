use serde::{Deserialize, Serialize};
use std::{
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
};

// ── Quota Snapshot ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuotaSnapshot {
    pub plan_type: Option<String>,
    pub five_hour_pct: Option<f64>,
    pub five_hour_reset_at: Option<u64>,
    pub seven_day_pct: Option<f64>,
    pub seven_day_reset_at: Option<u64>,
    pub available: bool,
    pub error: Option<String>,
}

// ── Quota Collector Trait ───────────────────────────────────────────

pub trait QuotaCollector {
    /// Returns `(snapshot, is_stale)` — stale means a background fetch should be triggered.
    fn collect_quota(&self) -> (QuotaSnapshot, bool);
}

// ── Cached File Collector (production) ──────────────────────────────

/// Reads quota data from a cache file. Never performs network I/O.
#[derive(Debug, Default)]
pub struct CachedFileQuotaCollector;

impl QuotaCollector for CachedFileQuotaCollector {
    fn collect_quota(&self) -> (QuotaSnapshot, bool) {
        let path = quota_cache_path();
        let contents = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return (QuotaSnapshot::default(), true), // no cache → stale
        };
        let cache: QuotaCacheFile = match serde_json::from_str(&contents) {
            Ok(c) => c,
            Err(_) => return (QuotaSnapshot::default(), true),
        };

        let now = crate::state::cache::now_epoch_ms();
        let age_ms = now.saturating_sub(cache.fetched_at_ms);

        // Check TTL based on success/failure
        let ttl = if cache.snapshot.error.is_some() {
            QUOTA_FAILURE_TTL_MS
        } else {
            QUOTA_SUCCESS_TTL_MS
        };

        if age_ms > ttl {
            // Stale — return snapshot but mark as stale (trigger re-fetch)
            let mut snapshot = cache.snapshot;
            snapshot.available = false;
            return (snapshot, true);
        }

        (cache.snapshot, false)
    }
}

// ── Stub Collector (testing) ────────────────────────────────────────

#[derive(Debug, Default)]
pub struct StubQuotaCollector {
    pub snapshot: QuotaSnapshot,
}

impl QuotaCollector for StubQuotaCollector {
    fn collect_quota(&self) -> (QuotaSnapshot, bool) {
        (self.snapshot.clone(), false)
    }
}

// ── Cache File Format ───────────────────────────────────────────────

pub const QUOTA_SUCCESS_TTL_MS: u64 = 60_000;
pub const QUOTA_FAILURE_TTL_MS: u64 = 15_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaCacheFile {
    pub fetched_at_ms: u64,
    pub snapshot: QuotaSnapshot,
}

/// Compute the quota cache file path (per-user, based on HOME hash).
pub fn quota_cache_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    home.hash(&mut hasher);
    let hash = hasher.finish();
    std::env::temp_dir().join(format!("cc-pulseline-quota-{hash:x}.json"))
}

/// Spawn the background quota fetch as a detached child process.
/// Fire-and-forget — errors are silently ignored.
pub fn spawn_background_fetch() {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };

    let _ = std::process::Command::new(exe)
        .arg("--fetch-quota")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_quota_collector_returns_preset() {
        let stub = StubQuotaCollector {
            snapshot: QuotaSnapshot {
                plan_type: Some("pro".to_string()),
                five_hour_pct: Some(42.0),
                available: true,
                ..Default::default()
            },
        };
        let (result, is_stale) = stub.collect_quota();
        assert_eq!(result.plan_type.as_deref(), Some("pro"));
        assert_eq!(result.five_hour_pct, Some(42.0));
        assert!(result.available);
        assert!(!is_stale);
    }

    #[test]
    fn default_snapshot_is_unavailable() {
        let snap = QuotaSnapshot::default();
        assert!(!snap.available);
        assert!(snap.plan_type.is_none());
        assert!(snap.five_hour_pct.is_none());
    }
}
