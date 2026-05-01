use cc_pulseline::{
    config::RenderConfig,
    render::color::{CTX_CRITICAL, CTX_GOOD, CTX_WARN},
    types::{QuotaMetrics, RateLimitWindow, RateLimits, RenderFrame, StdinPayload},
};
use serde_json::json;

fn make_payload() -> StdinPayload {
    let input = json!({
        "session_id": "quota-test",
        "model": {"display_name": "Opus"},
        "version": "1.0",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 30,
            "current_usage": {
                "input_tokens": 5000,
                "output_tokens": 100,
                "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 0
            }
        },
        "cost": {"total_cost_usd": 1.0, "total_duration_ms": 60000}
    })
    .to_string();
    serde_json::from_str(&input).unwrap()
}

fn render_with_quota(quota: QuotaMetrics, config: RenderConfig) -> Vec<String> {
    let payload = make_payload();
    let mut frame = RenderFrame::from_payload(&payload);
    frame.quota = quota;
    cc_pulseline::render::layout::render_frame(&frame, &config)
}

#[test]
fn quota_hidden_by_default() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(50.0),
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: false,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    // Should only have L1-L3 (3 lines), no quota line
    assert_eq!(
        lines.len(),
        3,
        "should have exactly 3 lines when quota disabled"
    );
    for line in &lines {
        assert!(!line.contains("Q:"), "no quota prefix when disabled");
    }
}

#[test]
fn quota_shows_pct_at_75pct() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(75.0),
        five_hour_reset_minutes: Some(120), // 2 hours
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: true,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    assert!(lines.len() > 3, "should have quota line after L3");
    let quota_line = &lines[3];
    assert!(quota_line.contains("75%"), "should show percentage");
    assert!(quota_line.contains(CTX_WARN), "75% should use warn color");
    assert!(
        quota_line.contains("resets 2h 0m"),
        "should show reset time"
    );
}

#[test]
fn quota_shows_limit_reached_at_100pct() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(100.0),
        five_hour_reset_minutes: Some(15),
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains("Limit reached"),
        "100% should show limit text, got: {quota_line}"
    );
    assert!(
        quota_line.contains("resets 15m"),
        "should show reset time, got: {quota_line}"
    );
}

#[test]
fn quota_hidden_when_no_data() {
    let quota = QuotaMetrics::default(); // all None
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    assert_eq!(lines.len(), 3, "no quota data should produce no quota line");
}

#[test]
fn quota_kept_when_only_activity_is_dropped() {
    // Quota belongs to the Budget group, not activity. The
    // DropActivityLinesFirst strategy must NOT drop quota — it only drops
    // rows past `activity_start`. With no activity rows at all, quota
    // remains as the 4th line even at narrow widths.
    let quota = QuotaMetrics {
        five_hour_pct: Some(50.0),
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        terminal_width: Some(20),
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    assert_eq!(
        lines.len(),
        4,
        "quota stays as part of Budget group; only activity rows get dropped"
    );
}

#[test]
fn quota_green_at_low_usage() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(25.0),
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: true,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains(CTX_GOOD),
        "25% should use good (green) color"
    );
}

#[test]
fn quota_reset_shows_days() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(40.0),
        five_hour_reset_minutes: Some(2880), // 2 days
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains("resets 2d 0h 0m"),
        "should show days for reset ≥24h, got: {quota_line}"
    );
}

#[test]
fn quota_shows_seven_day_when_enabled() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(30.0),
        five_hour_reset_minutes: Some(60),
        seven_day_pct: Some(55.0),
        seven_day_reset_minutes: Some(2880), // 2 days
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        show_quota_seven_day: true,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains("5h:"),
        "should show 5h label, got: {quota_line}"
    );
    assert!(
        quota_line.contains("7d:"),
        "should show 7d label, got: {quota_line}"
    );
    assert!(
        quota_line.contains("55%"),
        "should show 7d percentage, got: {quota_line}"
    );
}

#[test]
fn quota_hides_seven_day_when_disabled() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(40.0),
        seven_day_pct: Some(60.0),
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        show_quota_seven_day: false, // default
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains("5h:"),
        "should show 5h, got: {quota_line}"
    );
    assert!(
        !quota_line.contains("7d:"),
        "should NOT show 7d when disabled, got: {quota_line}"
    );
}

#[test]
fn quota_red_at_critical_usage() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(95.0),
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: true,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains(CTX_CRITICAL),
        "95% should use critical (red) color"
    );
}

#[test]
fn quota_shows_q_prefix() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(30.0),
        five_hour_reset_minutes: Some(60),
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains("Q:"),
        "should show Q: prefix, got: {quota_line}"
    );
}

#[test]
fn quota_shows_reset_placeholder_when_none() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(25.0),
        five_hour_reset_minutes: None, // reset unknown
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        !quota_line.contains("resets"),
        "should NOT show reset info when None, got: {quota_line}"
    );
}

#[test]
fn quota_shows_reset_imminent() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(90.0),
        five_hour_reset_minutes: Some(0), // imminent
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: false,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains("resets <1m"),
        "should show imminent reset, got: {quota_line}"
    );
}

#[test]
fn quota_warn_at_50pct() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(50.0),
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: true,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains(CTX_WARN),
        "50% should use warn color (threshold at 50%), got: {quota_line}"
    );
}

#[test]
fn quota_critical_at_85pct() {
    let quota = QuotaMetrics {
        five_hour_pct: Some(85.0),
        ..Default::default()
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        color_enabled: true,
        ..Default::default()
    };
    let lines = render_with_quota(quota, config);
    let quota_line = &lines[3];
    assert!(
        quota_line.contains(CTX_CRITICAL),
        "85% should use critical color (threshold at 85%), got: {quota_line}"
    );
}

// ── Stdin parsing tests ─────────────────────────────────────────────

#[test]
fn quota_from_rate_limits_deterministic() {
    let rate_limits = RateLimits {
        five_hour: Some(RateLimitWindow {
            used_percentage: Some(75.0),
            resets_at: Some(1_000_000 + 7200), // now + 2 hours
        }),
        seven_day: Some(RateLimitWindow {
            used_percentage: Some(30.0),
            resets_at: Some(1_000_000 + 172800), // now + 2 days
        }),
    };
    let quota = QuotaMetrics::from_rate_limits(&rate_limits, 1_000_000);

    assert_eq!(quota.five_hour_pct, Some(75.0));
    assert_eq!(quota.five_hour_reset_minutes, Some(120)); // 7200s / 60
    assert_eq!(quota.seven_day_pct, Some(30.0));
    assert_eq!(quota.seven_day_reset_minutes, Some(2880)); // 172800s / 60
    assert!(quota.has_data());
}

#[test]
fn quota_from_stdin_deserializes() {
    let input = json!({
        "session_id": "test",
        "version": "2.1.80",
        "rate_limits": {
            "five_hour": {
                "used_percentage": 75.0,
                "resets_at": 9999999999_u64
            },
            "seven_day": {
                "used_percentage": 30.0,
                "resets_at": 9999999999_u64
            }
        }
    })
    .to_string();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();
    let frame = RenderFrame::from_payload(&payload);

    assert_eq!(frame.quota.five_hour_pct, Some(75.0));
    assert_eq!(frame.quota.seven_day_pct, Some(30.0));
    assert!(frame.quota.has_data());
}

#[test]
fn quota_from_stdin_absent() {
    let input = json!({
        "session_id": "test",
        "version": "2.1.80"
    })
    .to_string();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();
    let frame = RenderFrame::from_payload(&payload);

    assert!(!frame.quota.has_data());
    assert_eq!(frame.quota.five_hour_pct, None);
    assert_eq!(frame.quota.seven_day_pct, None);
}

#[test]
fn quota_from_rate_limits_partial_windows() {
    let rate_limits = RateLimits {
        five_hour: Some(RateLimitWindow {
            used_percentage: Some(50.0),
            resets_at: None,
        }),
        seven_day: None,
    };
    let quota = QuotaMetrics::from_rate_limits(&rate_limits, 1_000_000);

    assert_eq!(quota.five_hour_pct, Some(50.0));
    assert_eq!(quota.five_hour_reset_minutes, None, "no resets_at → None");
    assert_eq!(quota.seven_day_pct, None, "seven_day absent → None");
    assert!(quota.has_data(), "five_hour data alone is sufficient");
}

#[test]
fn quota_from_rate_limits_past_reset_saturates() {
    let rate_limits = RateLimits {
        five_hour: Some(RateLimitWindow {
            used_percentage: Some(100.0),
            resets_at: Some(500), // in the past relative to now_secs=1000
        }),
        seven_day: None,
    };
    let quota = QuotaMetrics::from_rate_limits(&rate_limits, 1000);

    assert_eq!(
        quota.five_hour_reset_minutes,
        Some(0),
        "past reset → 0 minutes (saturating)"
    );
}
