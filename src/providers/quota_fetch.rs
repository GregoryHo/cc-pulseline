//! Background quota fetch logic. Called via `cc-pulseline --fetch-quota`.
//!
//! Reads OAuth credentials, calls the Anthropic usage API, and writes
//! a quota cache file. This module is designed to run as a short-lived
//! subprocess — it does network I/O and then exits.

use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use serde::Deserialize;

use super::quota::{quota_cache_path, QuotaCacheFile, QuotaSnapshot};

const KEYCHAIN_BACKOFF_SECS: u64 = 60;
const CURL_TIMEOUT_SECS: u64 = 10;

/// Entry point for `--fetch-quota`. Runs synchronously and exits.
pub fn run_fetch_quota() {
    let snapshot = match fetch_quota_snapshot() {
        Ok(s) => s,
        Err(err) => QuotaSnapshot {
            error: Some(err),
            ..Default::default()
        },
    };
    write_quota_cache(&snapshot);
}

fn fetch_quota_snapshot() -> Result<QuotaSnapshot, String> {
    let creds = read_credentials()?;
    let oauth = creds
        .claude_ai_oauth
        .as_ref()
        .ok_or_else(|| "no OAuth credentials found".to_string())?;

    let access_token = oauth
        .access_token
        .clone()
        .ok_or_else(|| "no access token found".to_string())?;

    // Skip API users — they don't have subscription quotas
    if oauth.subscription_type.as_deref() == Some("api") {
        return Ok(QuotaSnapshot {
            error: Some("api user — no quota".to_string()),
            ..Default::default()
        });
    }

    // Check token expiry
    if let Some(expires_at) = oauth.expires_at {
        if crate::state::cache::now_epoch_ms() > expires_at {
            return Err("access token expired".to_string());
        }
    }

    let plan_type = oauth
        .subscription_type
        .as_deref()
        .map(normalize_plan_type)
        .map(String::from);

    let response_json = call_usage_api(&access_token)?;
    let usage: UsageApiResponse =
        serde_json::from_str(&response_json).map_err(|e| format!("parse usage response: {e}"))?;

    Ok(QuotaSnapshot {
        plan_type,
        five_hour_pct: usage.five_hour.as_ref().map(|h| h.utilization),
        five_hour_reset_at: usage
            .five_hour
            .as_ref()
            .and_then(|h| h.resets_at.as_deref())
            .and_then(parse_iso_to_epoch_ms),
        seven_day_pct: usage.seven_day.as_ref().map(|d| d.utilization),
        seven_day_reset_at: usage
            .seven_day
            .as_ref()
            .and_then(|d| d.resets_at.as_deref())
            .and_then(parse_iso_to_epoch_ms),
        available: true,
        error: None,
    })
}

// ── Credential Reading ──────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OAuthCredentials>,
}

#[derive(Debug, Default, Deserialize)]
struct OAuthCredentials {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "refreshToken")]
    refresh_token: Option<String>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<u64>,
}

fn read_credentials() -> Result<Credentials, String> {
    // Try macOS Keychain first (only on macOS)
    #[cfg(target_os = "macos")]
    {
        if let Some(creds) = try_keychain_credentials() {
            return Ok(creds);
        }
    }

    // Fallback to credentials file
    try_file_credentials()
}

#[cfg(target_os = "macos")]
fn try_keychain_credentials() -> Option<Credentials> {
    // Check backoff file to avoid re-prompting after Keychain dialog failure
    let backoff_path = std::env::temp_dir().join("cc-pulseline-keychain-backoff");
    if let Ok(metadata) = fs::metadata(&backoff_path) {
        if let Ok(modified) = metadata.modified() {
            let elapsed = std::time::SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default()
                .as_secs();
            if elapsed < KEYCHAIN_BACKOFF_SECS {
                return None; // Still in backoff period
            }
        }
    }

    let output = Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        // Write backoff file to avoid re-prompting
        let _ = fs::write(&backoff_path, "");
        return None;
    }

    let json_str = String::from_utf8(output.stdout).ok()?;
    serde_json::from_str(json_str.trim()).ok()
}

fn try_file_credentials() -> Result<Credentials, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot determine HOME".to_string())?;

    let cred_path = PathBuf::from(&home)
        .join(".claude")
        .join(".credentials.json");

    let contents = fs::read_to_string(&cred_path).map_err(|e| format!("read credentials: {e}"))?;
    serde_json::from_str(&contents).map_err(|e| format!("parse credentials: {e}"))
}

// ── API Call ────────────────────────────────────────────────────────

fn call_usage_api(access_token: &str) -> Result<String, String> {
    // Use curl with auth header piped via stdin to avoid credential leakage in ps
    let mut child = Command::new("curl")
        .args([
            "-s",
            "--max-time",
            &CURL_TIMEOUT_SECS.to_string(),
            "-H",
            "anthropic-beta: oauth-2025-04-20",
            "-H",
            "@-", // read additional header from stdin
            "https://api.anthropic.com/api/oauth/usage",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn curl: {e}"))?;

    // Write Bearer token header to stdin
    if let Some(mut stdin) = child.stdin.take() {
        let header = format!("Authorization: Bearer {access_token}");
        stdin
            .write_all(header.as_bytes())
            .map_err(|e| format!("write to curl stdin: {e}"))?;
        // stdin drops here, closing the pipe
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("curl wait: {e}"))?;

    if !output.status.success() {
        return Err(format!("curl exit code: {:?}", output.status.code()));
    }

    String::from_utf8(output.stdout).map_err(|e| format!("curl output encoding: {e}"))
}

// ── API Response Parsing ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct UsageApiResponse {
    five_hour: Option<UsagePeriod>,
    seven_day: Option<UsagePeriod>,
    // seven_day_opus is ignored for now
}

#[derive(Debug, Deserialize)]
struct UsagePeriod {
    utilization: f64,
    resets_at: Option<String>,
}

/// Normalize subscription type to display-friendly plan name.
fn normalize_plan_type(sub_type: &str) -> &str {
    if sub_type.contains("max") {
        "max"
    } else if sub_type.contains("pro") {
        "pro"
    } else if sub_type.contains("team") {
        "team"
    } else {
        sub_type
    }
}

/// Parse ISO 8601 timestamp to epoch ms. Returns None on parse failure.
/// Handles `YYYY-MM-DDTHH:MM:SSZ` and `YYYY-MM-DDTHH:MM:SS+HH:MM` formats.
fn parse_iso_to_epoch_ms(iso: &str) -> Option<u64> {
    // Strip timezone suffix: Z or +HH:MM / -HH:MM
    let trimmed = iso.trim().trim_end_matches('Z');
    // Strip +/-HH:MM timezone offset (e.g., "+05:30", "-08:00")
    let trimmed = if trimmed.len() > 6 {
        let tail = &trimmed[trimmed.len() - 6..];
        if (tail.starts_with('+') || tail.starts_with('-')) && tail.as_bytes()[3] == b':' {
            &trimmed[..trimmed.len() - 6]
        } else {
            trimmed
        }
    } else {
        trimmed
    };
    // Strip fractional seconds (e.g., "...T04:59:59.123" → "...T04:59:59")
    let trimmed = if let Some(dot_pos) = trimmed.rfind('.') {
        if trimmed[..dot_pos].contains('T') {
            &trimmed[..dot_pos]
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    let parts: Vec<&str> = trimmed.split('T').collect();
    if parts.len() != 2 {
        return None;
    }

    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_parts: Vec<u32> = parts[1].split(':').filter_map(|s| s.parse().ok()).collect();

    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }

    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (hour, min, sec) = (time_parts[0], time_parts[1], time_parts[2]);

    // Convert to days since epoch using a simplified algorithm
    let days = days_from_civil(year as i64, month as i64, day as i64);
    let total_secs = days as u64 * 86400 + hour as u64 * 3600 + min as u64 * 60 + sec as u64;
    Some(total_secs * 1000)
}

/// Days from civil date (year, month, day) to Unix epoch.
/// Algorithm from Howard Hinnant's date library.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

// ── Cache Writing ───────────────────────────────────────────────────

fn write_quota_cache(snapshot: &QuotaSnapshot) {
    let cache = QuotaCacheFile {
        fetched_at_ms: crate::state::cache::now_epoch_ms(),
        snapshot: snapshot.clone(),
    };

    let contents = match serde_json::to_string(&cache) {
        Ok(c) => c,
        Err(_) => return,
    };

    let path = quota_cache_path();
    let tmp_path = path.with_extension("tmp");
    if fs::write(&tmp_path, contents).is_ok() {
        let _ = fs::rename(&tmp_path, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_credentials_json() {
        let json = r#"{
            "claudeAiOauth": {
                "accessToken": "sk-ant-oat01-test",
                "refreshToken": "rt-test",
                "subscriptionType": "claude_max_2024",
                "expiresAt": 1700000000000
            }
        }"#;

        let creds: Credentials = serde_json::from_str(json).expect("should parse");
        let oauth = creds.claude_ai_oauth.expect("should have oauth");
        assert_eq!(oauth.access_token.as_deref(), Some("sk-ant-oat01-test"));
        assert_eq!(oauth.subscription_type.as_deref(), Some("claude_max_2024"));
        assert_eq!(oauth.expires_at, Some(1700000000000));
    }

    #[test]
    fn parse_usage_api_response() {
        let json = r#"{
            "five_hour": {"utilization": 25.0, "resets_at": "2025-11-04T04:59:59Z"},
            "seven_day": {"utilization": 35.0, "resets_at": "2025-11-06T03:59:59Z"},
            "seven_day_opus": {"utilization": 0.0, "resets_at": null}
        }"#;

        let response: UsageApiResponse = serde_json::from_str(json).expect("should parse");
        assert_eq!(response.five_hour.as_ref().unwrap().utilization, 25.0);
        assert_eq!(
            response.five_hour.as_ref().unwrap().resets_at.as_deref(),
            Some("2025-11-04T04:59:59Z")
        );
        assert_eq!(response.seven_day.as_ref().unwrap().utilization, 35.0);
    }

    #[test]
    fn normalize_plan_type_handles_variants() {
        assert_eq!(normalize_plan_type("claude_max_2024"), "max");
        assert_eq!(normalize_plan_type("claude_pro_2024"), "pro");
        assert_eq!(normalize_plan_type("team_enterprise"), "team");
        assert_eq!(normalize_plan_type("unknown"), "unknown");
    }

    #[test]
    fn parse_iso_timestamp() {
        let ms = parse_iso_to_epoch_ms("2025-11-04T04:59:59Z");
        assert!(ms.is_some());
        // 2025-11-04T04:59:59Z should be a reasonable epoch ms value
        let val = ms.unwrap();
        assert!(val > 1_700_000_000_000); // After 2023
        assert!(val < 1_800_000_000_000); // Before ~2027
    }

    #[test]
    fn parse_iso_with_timezone_offset() {
        // Same instant: Z and +00:00 should produce the same result
        let z_val = parse_iso_to_epoch_ms("2025-11-04T04:59:59Z");
        let offset_val = parse_iso_to_epoch_ms("2025-11-04T04:59:59+00:00");
        assert_eq!(z_val, offset_val, "Z and +00:00 should match");

        // Negative offset should also parse without error
        let neg_val = parse_iso_to_epoch_ms("2025-11-04T04:59:59-08:00");
        assert!(neg_val.is_some(), "negative offset should parse");
    }

    #[test]
    fn parse_iso_with_fractional_seconds() {
        let ms = parse_iso_to_epoch_ms("2025-11-04T04:59:59.123Z");
        assert!(ms.is_some(), "should parse .123Z fractional seconds");
        let val = ms.unwrap();
        assert!(val > 1_700_000_000_000);
        assert!(val < 1_800_000_000_000);
    }

    #[test]
    fn parse_iso_with_microsecond_fractional() {
        let ms = parse_iso_to_epoch_ms("2025-11-04T04:59:59.123456Z");
        assert!(ms.is_some(), "should parse .123456Z fractional seconds");
        // Should produce the same epoch value as without fractional part
        let without_frac = parse_iso_to_epoch_ms("2025-11-04T04:59:59Z");
        assert_eq!(ms, without_frac, "fractional stripped — same epoch");
    }

    #[test]
    fn parse_iso_invalid_returns_none() {
        assert!(parse_iso_to_epoch_ms("not-a-date").is_none());
        assert!(parse_iso_to_epoch_ms("").is_none());
    }
}
