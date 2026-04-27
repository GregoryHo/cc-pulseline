//! Width-bracket resolver tests for `pane.style = "auto"`.
//!
//! Each tick resolves to one of cockpit / console / flightstrip based on
//! `terminal_width`. We assert the *visible signature* of each resolved
//! layout rather than calling the resolver directly, which keeps the test
//! honest if the brackets shift in the future.

use std::fs;

use cc_pulseline::config::RenderConfig;
use cc_pulseline::render::icons::FRAME_TL;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::PulseLineRunner;
use serde_json::json;
use tempfile::TempDir;

fn fake_home() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    fs::create_dir_all(tmp.path().join(".claude")).expect("fake home .claude");
    tmp
}

fn payload(cwd: &str) -> String {
    json!({
        "session_id": "auto-test-session",
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
        pane_style: LayoutStyle::Auto,
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
fn auto_picks_console_at_130_plus() {
    let tmp = fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));
    let lines = runner
        .run_from_str(&payload(cwd), cfg(140))
        .expect("render");
    assert!(
        lines.first().unwrap().starts_with(FRAME_TL),
        "expected console (framed) at 140 cols, got {lines:?}"
    );
}

#[test]
fn auto_picks_cockpit_in_110_to_129_band() {
    let tmp = fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));
    let lines = runner
        .run_from_str(&payload(cwd), cfg(120))
        .expect("render");
    assert!(
        !lines.first().unwrap().starts_with(FRAME_TL),
        "expected unframed cockpit at 120 cols"
    );
    // Cockpit puts CTX on its own (cluster) row separately from identity.
    assert!(
        lines.iter().any(|l| l.contains("CTX") && l.contains("43%")),
        "expected CTX 43% cluster row at 120 cols, got {lines:?}"
    );
}

#[test]
fn auto_picks_flightstrip_in_90_to_109_band() {
    let tmp = fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));
    let lines = runner
        .run_from_str(&payload(cwd), cfg(100))
        .expect("render");
    // Flightstrip's L1 packs identity + pct + cost on one line.
    assert!(lines[0].contains("43%"));
    assert!(lines[0].contains("$3.50"));
    assert!(lines[0].contains("Opus 4.7"));
}

#[test]
fn auto_picks_cockpit_below_90_cols_with_collapse() {
    let tmp = fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));
    let lines = runner.run_from_str(&payload(cwd), cfg(75)).expect("render");
    assert_eq!(
        lines.len(),
        1,
        "below-80 cockpit collapses to one row, got {lines:?}"
    );
}
