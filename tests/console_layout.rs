//! Integration tests for v2 Console (framed dashboard) layout.

use std::fs;

use cc_pulseline::config::RenderConfig;
use cc_pulseline::render::icons::{FRAME_BL, FRAME_TL};
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::PulseLineRunner;
use serde_json::json;
use tempfile::TempDir;

fn build_fake_home() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    fs::create_dir_all(tmp.path().join(".claude")).expect("fake home .claude");
    tmp
}

fn payload(cwd: &str) -> String {
    json!({
        "session_id": "console-test-session",
        "cwd": cwd,
        "workspace": {"current_dir": cwd},
        "model": {"display_name": "Opus 4.7"},
        "output_style": {"name": "default"},
        "version": "2.1.119",
        "context_window": {
            "context_window_size": 200_000,
            "used_percentage": 43,
            "current_usage": {
                "input_tokens": 1_000,
                "output_tokens": 2_000,
                "cache_creation_input_tokens": 30,
                "cache_read_input_tokens": 40
            }
        },
        "cost": { "total_cost_usd": 3.5, "total_duration_ms": 3_600_000 }
    })
    .to_string()
}

fn cfg(width: usize) -> RenderConfig {
    RenderConfig {
        color_enabled: false,
        terminal_width: Some(width),
        pane_style: LayoutStyle::Console,
        show_claude_md: false,
        show_rules: false,
        show_memory: false,
        show_hooks: false,
        show_mcp: false,
        show_skills: false,
        show_plugins: false,
        ..RenderConfig::default()
    }
}

#[test]
fn console_emits_framed_borders_at_full_width() {
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));

    let lines = runner
        .run_from_str(&payload(cwd), cfg(150))
        .expect("render");
    assert!(
        lines.first().unwrap().starts_with(FRAME_TL),
        "expected ╭ top frame, got {:?}",
        lines.first()
    );
    assert!(
        lines.last().unwrap().starts_with(FRAME_BL),
        "expected ╰ bottom frame, got {:?}",
        lines.last()
    );
    // CTX gauge row inside the frame
    assert!(lines.iter().any(|l| l.contains("CTX")));
    assert!(lines.iter().any(|l| l.contains("$3.50")));
}

#[test]
fn console_falls_back_to_cockpit_below_110_cols() {
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));

    let lines = runner
        .run_from_str(&payload(cwd), cfg(100))
        .expect("render");
    assert!(
        !lines.first().unwrap().starts_with(FRAME_TL),
        "expected no frame at 100 cols (cockpit fallback), got {lines:?}"
    );
}

#[test]
fn console_includes_quota_when_enabled() {
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut cfg_q = cfg(160);
    cfg_q.show_quota = true;

    // Payload with rate_limits → quota gets surfaced via QuotaMetrics.
    let payload = json!({
        "session_id": "console-quota-session",
        "cwd": cwd,
        "workspace": {"current_dir": cwd},
        "model": {"display_name": "Opus 4.7"},
        "output_style": {"name": "default"},
        "version": "2.1.119",
        "context_window": {
            "context_window_size": 200_000,
            "used_percentage": 43,
            "current_usage": {
                "input_tokens": 1_000,
                "output_tokens": 2_000,
                "cache_creation_input_tokens": 30,
                "cache_read_input_tokens": 40
            }
        },
        "cost": { "total_cost_usd": 3.5, "total_duration_ms": 3_600_000 },
        "rate_limits": {
            "five_hour": { "used_percentage": 75.0, "resets_at": 9_999_999_999u64 }
        }
    })
    .to_string();

    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));
    let lines = runner.run_from_str(&payload, cfg_q).expect("render");

    assert!(
        lines.iter().any(|l| l.contains("5h ")),
        "console quota row missing 5h label, got {lines:?}"
    );
}
