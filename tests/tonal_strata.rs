//! Tonal strata: per-line separator tint that splits "state" rows
//! (Identity/Config/Budget/Quota) from "activity" rows (Tools/Agents/Todos)
//! using two hand-authored chrome tiers on every shipped theme.
//!
//! Mapping (wired in `render::layout::tinted_palette`):
//! - LineKind::Activity → `strata_activity`
//! - everything else    → `strata_state`
//!
//! See `designs/tonal-strata-redesign.md` for the design contract and per-theme
//! seed values. Reference codes here are tokyo-night dark:
//!   strata_state    = 238
//!   strata_activity = 103

use std::{
    fs::{self, OpenOptions},
    io::Write,
};

use cc_pulseline::{config::RenderConfig, render::color::extract_ansi_code, PulseLineRunner};
use serde_json::json;
use tempfile::TempDir;

fn append_line(path: &std::path::Path, line: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("transcript file should open");
    writeln!(file, "{line}").expect("line should append");
}

/// Minimal fixture: isolates env collection via a fake HOME. No git repo —
/// these tests only care about separator ANSI codes on lines, not git status.
fn build_fixture() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path();
    let fake_home = root.join("fake_home");
    fs::create_dir_all(fake_home.join(".claude")).expect("fake home .claude");
    tmp
}

fn payload(cwd: &str) -> String {
    json!({
        "session_id": "tonal-strata-session",
        "cwd": cwd,
        "workspace": {"current_dir": cwd},
        "model": {"display_name": "Opus 4.7"},
        "output_style": {"name": "default"},
        "version": "2.1.119",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 43,
            "current_usage": {
                "input_tokens": 10,
                "output_tokens": 20,
                "cache_creation_input_tokens": 30,
                "cache_read_input_tokens": 40
            }
        },
        "cost": { "total_cost_usd": 3.5, "total_duration_ms": 3600000 }
    })
    .to_string()
}

fn base_config() -> RenderConfig {
    RenderConfig {
        color_enabled: true,
        ..RenderConfig::default()
    }
}

/// Extract the ANSI 38;5;N color code immediately preceding the first `|` on
/// a line. Returns None when there is no `|` separator or no ANSI run precedes it.
fn first_separator_color_code(line: &str) -> Option<u8> {
    let bar_pos = line.find('|')?;
    let prefix = &line[..bar_pos];
    let start = prefix.rfind("\x1b[38;5;")?;
    let m_off = prefix[start..].find('m')?;
    extract_ansi_code(&prefix[start..start + m_off + 1])
}

#[test]
fn tonal_strata_on_uses_strata_state_for_all_state_lines() {
    let tmp = build_fixture();
    let cwd = tmp.path().to_str().unwrap().to_string();
    let fake_home = tmp.path().join("fake_home");

    let mut cfg = base_config();
    cfg.pane_tonal_strata = true;

    let mut runner = PulseLineRunner::default().with_user_home(fake_home);
    let lines = runner.run_from_str(&payload(&cwd), cfg).expect("render");

    assert!(lines.len() >= 3, "expected at least 3 core lines");

    // L1 (Identity), L2 (Config), L3 (Budget) all map to strata_state.
    // tokyo-night dark strata_state = 238.
    for (idx, line) in lines.iter().take(3).enumerate() {
        let sep = first_separator_color_code(line)
            .unwrap_or_else(|| panic!("line {idx} missing | separator"));
        assert_eq!(
            sep, 238,
            "line {idx} (state row) should use strata_state (238); got {sep}"
        );
    }
}

#[test]
fn tonal_strata_on_uses_strata_activity_for_activity_lines() {
    let tmp = build_fixture();
    let cwd = tmp.path().to_str().unwrap().to_string();
    let fake_home = tmp.path().join("fake_home");
    let transcript = tmp.path().join("activity.jsonl");

    // Two tool_use events → recent-tools line will join two segments with ` | `.
    append_line(
        &transcript,
        r#"{"type":"tool_use","tool_use_id":"t1","name":"Read"}"#,
    );
    append_line(
        &transcript,
        r#"{"type":"tool_use","tool_use_id":"t2","name":"Bash"}"#,
    );

    let payload = json!({
        "session_id": "tonal-strata-activity",
        "cwd": &cwd,
        "workspace": {"current_dir": &cwd},
        "model": {"display_name": "Opus 4.7"},
        "output_style": {"name": "default"},
        "version": "2.1.119",
        "transcript_path": transcript.to_str().unwrap(),
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 43,
            "current_usage": {
                "input_tokens": 10,
                "output_tokens": 20,
                "cache_creation_input_tokens": 30,
                "cache_read_input_tokens": 40
            }
        },
        "cost": { "total_cost_usd": 3.5, "total_duration_ms": 3600000 }
    })
    .to_string();

    let mut cfg = base_config();
    cfg.pane_tonal_strata = true;
    cfg.transcript_poll_throttle_ms = 0;
    cfg.max_tool_lines = 2;

    let mut runner = PulseLineRunner::default().with_user_home(fake_home);
    let lines = runner.run_from_str(&payload, cfg).expect("render");

    // Find the recent-tool line (has the `T:` prefix on plain text). Use the
    // ANSI-stripped form so the prefix is detectable regardless of color.
    let activity_line = lines
        .iter()
        .find(|l| !l.contains("CTX") && l.contains("T:"))
        .expect("recent-tools line with T:Read | T:Bash should be present");

    let sep = first_separator_color_code(activity_line)
        .expect("activity line should have a colored | separator");

    // tokyo-night dark strata_activity = 103.
    assert_eq!(
        sep, 103,
        "activity line separator should use strata_activity (103); got {sep}"
    );
}

#[test]
fn tonal_strata_off_keeps_default_separator_on_every_line() {
    let tmp = build_fixture();
    let cwd = tmp.path().to_str().unwrap().to_string();
    let fake_home = tmp.path().join("fake_home");

    let mut cfg = base_config();
    cfg.pane_tonal_strata = false;

    let mut runner = PulseLineRunner::default().with_user_home(fake_home);
    let lines = runner.run_from_str(&payload(&cwd), cfg).expect("render");

    for (idx, line) in lines.iter().take(3).enumerate() {
        let sep = first_separator_color_code(line)
            .unwrap_or_else(|| panic!("line {idx} missing separator ANSI"));
        assert_eq!(
            sep, 238,
            "line {idx} separator should be default emphasis_separator (238) when strata off; got {sep}"
        );
    }
}

#[test]
fn tonal_strata_no_color_emits_no_ansi_regardless_of_flag() {
    let tmp = build_fixture();
    let cwd = tmp.path().to_str().unwrap().to_string();
    let fake_home = tmp.path().join("fake_home");

    let mut cfg = base_config();
    cfg.color_enabled = false;
    cfg.pane_tonal_strata = true;

    let mut runner = PulseLineRunner::default().with_user_home(fake_home);
    let lines = runner.run_from_str(&payload(&cwd), cfg).expect("render");

    for (idx, line) in lines.iter().take(3).enumerate() {
        assert!(
            !line.contains("\x1b["),
            "line {idx} should contain no ANSI escapes under NO_COLOR; got {line:?}"
        );
    }
}
