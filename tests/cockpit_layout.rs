//! Integration tests for v2 Cockpit layout.
//!
//! These exercise the full PulseLineRunner pipeline (stdin → providers →
//! frame → render) so a regression in any seam shows up here. The unit
//! tests in `src/render/frames/v2/cockpit.rs` cover narrower invariants.

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
        "session_id": "cockpit-test-session",
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

fn cockpit_cfg(width: usize) -> RenderConfig {
    RenderConfig {
        color_enabled: false,
        terminal_width: Some(width),
        pane_style: LayoutStyle::Cockpit,
        // v2 default: opt-in config row
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
fn cockpit_renders_identity_and_cluster_at_full_width() {
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));

    let lines = runner
        .run_from_str(&payload(cwd), cockpit_cfg(140))
        .expect("render");

    // Identity row + cluster row (no config row, no activity)
    assert!(lines.len() >= 2, "expected >=2 rows, got {lines:?}");
    assert!(lines[0].contains("Opus 4.7"), "identity row missing model");
    // Cluster row identified by the `used/total` numbers (CTX label and
    // `%` were dropped from the row format — the gauge already conveys
    // the ratio visually; the numbers add the precision `%` can't).
    assert!(
        lines
            .iter()
            .any(|l| l.contains("/200.0k") && !l.starts_with("Opus")),
        "expected CTX cluster row with used/total: {lines:?}"
    );
}

#[test]
fn cockpit_collapses_to_single_row_under_80_cols() {
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));

    let lines = runner
        .run_from_str(&payload(cwd), cockpit_cfg(70))
        .expect("render");
    assert_eq!(
        lines.len(),
        1,
        "expected single-row degraded mode: {lines:?}"
    );
    assert!(lines[0].contains("43%"));
    assert!(lines[0].contains("$3.50"));
}

#[test]
fn cockpit_inserts_optional_config_row_when_toggle_on() {
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut cfg = cockpit_cfg(140);
    cfg.show_claude_md = true;
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));

    let lines = runner.run_from_str(&payload(cwd), cfg).expect("render");
    // L2 row no longer carries a `CFG ` prefix — segments stand on their
    // own icons + counts. Identify by content.
    assert!(
        lines.iter().any(|l| l.contains("CLAUDE.md")),
        "expected config row with CLAUDE.md count, got {lines:?}"
    );
}

#[test]
fn cockpit_pushes_ctx_history_across_invocations() {
    // Two ticks at the same session key should produce a sparkline that grows
    // — but only when the resolved `context_visual` includes "sparkline".
    let tmp = build_fake_home();
    let cwd = tmp.path().to_str().unwrap();
    let mut runner = PulseLineRunner::default().with_user_home(tmp.path().join("fake_home"));

    let mut cfg = cockpit_cfg(140);
    cfg.context_visual = "gauge+sparkline".to_string();
    cfg.glyph_mode = cc_pulseline::config::GlyphMode::Icon;

    let _ = runner
        .run_from_str(&payload(cwd), cfg.clone())
        .expect("first render");

    let lines2 = runner
        .run_from_str(&payload(cwd), cfg)
        .expect("second render");

    let cluster = lines2
        .iter()
        .find(|l| l.contains("/200.0k") && !l.starts_with("Opus"))
        .expect("cluster row");
    assert!(
        cluster
            .chars()
            .any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
        "cluster row should contain braille sparkline cells: {cluster:?}"
    );
}

#[test]
fn cockpit_layout_name_resolves_to_v2cockpit() {
    use cc_pulseline::config::{build_render_config, PulselineConfig};
    let toml_input = r#"
[layout]
name = "cockpit"
"#;
    let parsed: PulselineConfig = toml::from_str(toml_input).expect("toml parse");
    let rendered = build_render_config(&parsed);
    assert_eq!(rendered.pane_style, LayoutStyle::Cockpit);
}
