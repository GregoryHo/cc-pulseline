//! Integration tests for v2 Flightstrip (dense 2-row) layout.

use std::fs;

use cc_pulseline::config::RenderConfig;
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
        "session_id": "flightstrip-test-session",
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
        pane_style: LayoutStyle::Flightstrip,
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
fn flightstrip_renders_l1_with_pct_and_cost() {
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));

    let lines = runner
        .run_from_str(&payload(cwd), cfg(120))
        .expect("render");
    assert!(!lines.is_empty(), "expected at least one row");
    assert!(lines[0].contains("Opus 4.7"));
    assert!(lines[0].contains("43%"));
    assert!(lines[0].contains("$3.50"));
}

#[test]
fn flightstrip_drops_cost_on_narrow_width() {
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));

    let lines = runner.run_from_str(&payload(cwd), cfg(80)).expect("render");
    assert!(
        !lines[0].contains("$3.50"),
        "expected cost dropped at 80 cols, got {lines:?}"
    );
    assert!(lines[0].contains("43%"));
}

#[test]
fn flightstrip_collapses_to_single_row_below_70_cols() {
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));

    let lines = runner.run_from_str(&payload(cwd), cfg(60)).expect("render");
    assert_eq!(lines.len(), 1);
}
