//! `--columns`, `--rows` and `--layout`: the flags a caller that owns the
//! box it draws into uses instead of COLUMNS, LINES and the config file.

use std::{
    io::Write,
    process::{Command, Stdio},
};

use cc_pulseline::render::color::strip_ansi;

/// Runs the binary in a sandbox HOME so no local config leaks in.
fn render(flags: &[&str]) -> Vec<String> {
    let sandbox = tempfile::TempDir::new().expect("temp sandbox");
    let path = sandbox.path().display().to_string();
    let payload = serde_json::json!({
        "session_id": "box-overrides",
        "cwd": path,
        "workspace": {"current_dir": path},
        "model": {"display_name": "Opus 5"},
        "version": "2.1.278",
        "context_window": {"context_window_size": 1_000_000, "used_percentage": 23},
        "rate_limits": {
            "five_hour": {"used_percentage": 33, "resets_at": 1_789_000_000u64},
            "seven_day": {"used_percentage": 31, "resets_at": 1_789_500_000u64}
        },
        "activity": {
            "tools": [{"id": "t1", "name": "Bash", "target": "cargo test"}],
            "completed_counts": [{"name": "Read", "count": 12, "failed": 0}],
            "completed_total": 12,
            "todo": {
                "text": "Wiring the pane",
                "pending": 1,
                "completed": 1,
                "total": 3,
                "is_task_api": true,
                "in_progress_items": [{"text": "Wiring the pane"}]
            }
        }
    })
    .to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"))
        .args(flags)
        .current_dir(sandbox.path())
        .env("HOME", sandbox.path())
        .env("USERPROFILE", sandbox.path())
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("binary should start");

    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(payload.as_bytes())
        .expect("payload should write");

    let output = child.wait_with_output().expect("binary should finish");
    assert!(output.status.success(), "binary should exit 0");

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect()
}

#[test]
fn rows_caps_the_render_and_folds_rather_than_truncates() {
    let three = render(&["--rows", "3", "--columns", "120"]);
    let one = render(&["--rows", "1", "--columns", "120"]);

    assert_eq!(three.len(), 3, "three rows requested: {three:#?}");
    assert_eq!(one.len(), 1, "one row requested: {one:#?}");
    assert!(
        one[0].contains("Opus 5") && one[0].contains("23%"),
        "the single row folds identity and budget together: {}",
        one[0]
    );
}

#[test]
fn columns_bounds_every_row() {
    for width in [70, 100] {
        let lines = render(&["--rows", "4", "--columns", &width.to_string()]);
        for line in &lines {
            let visible = strip_ansi(line).chars().count();
            assert!(
                visible <= width,
                "row is {visible} cells wide, over the {width} asked for: {line}"
            );
        }
    }
}

#[test]
fn layout_selects_the_renderer() {
    let ledger = render(&["--rows", "14", "--columns", "100", "--layout", "ledger"]);
    let budgets = render(&["--rows", "14", "--columns", "100", "--layout", "budgets"]);

    assert!(
        ledger.iter().any(|line| line.contains("CTX")),
        "ledger labels its rows: {ledger:#?}"
    );
    assert!(
        ledger.first().is_some_and(|line| line.starts_with('╭')),
        "ledger is framed: {ledger:#?}"
    );
    assert!(
        budgets.iter().any(|line| line.contains("CONTEXT")),
        "budgets leads with its gauges: {budgets:#?}"
    );
    assert!(
        budgets.first().is_some_and(|line| !line.starts_with('╭')),
        "budgets is frameless: {budgets:#?}"
    );
}

#[test]
fn an_unparseable_value_leaves_the_config_alone() {
    let lines = render(&["--rows", "abc", "--columns", "0", "--layout", "nonsense"]);
    assert!(
        !lines.is_empty(),
        "a bad flag value must not kill the render"
    );
}
