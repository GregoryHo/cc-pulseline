//! `--columns`, `--rows`, `--layout` and `--no-icons`: the flags a caller that owns the
//! box it draws into uses instead of COLUMNS, LINES and the config file.

use std::{
    io::Write,
    process::{Command, Stdio},
};

use cc_pulseline::render::color::strip_ansi;

/// Runs the binary in a sandbox HOME so no local config leaks in.
fn render(flags: &[&str]) -> Vec<String> {
    render_with(flags, true)
}

/// `render`, with colour left on when `no_color` is false.
fn render_with(flags: &[&str], no_color: bool) -> Vec<String> {
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

    let mut command = Command::new(env!("CARGO_BIN_EXE_cc-pulseline"));
    command
        .args(flags)
        .current_dir(sandbox.path())
        .env("HOME", sandbox.path())
        .env("USERPROFILE", sandbox.path())
        .env_remove("NO_COLOR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if no_color {
        command.env("NO_COLOR", "1");
    }
    let mut child = command.spawn().expect("binary should start");

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

#[test]
fn layout_also_selects_that_layout_s_visual_defaults() {
    // `--layout` has to be applied before the config is built. Every
    // `*_visual` field the caller left empty — which is every one of them in a
    // default config — is resolved against the layout name once and written in
    // as a concrete string, so an override applied to `pane_style` afterwards
    // drew the new layout with the old layout's widget choices. The ledger's
    // own CTX default is `text+sparkline`; `none`'s is `text`.
    let ledger = render(&["--rows", "14", "--columns", "100", "--layout", "ledger"]);
    let ctx = ledger
        .iter()
        .find(|line| line.contains("CTX"))
        .expect("the ledger labels a CTX row");

    assert!(
        ctx.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c)),
        "the ledger's CTX row carries its braille sparkline: {ctx}"
    );
}

/// Whether a line holds a Private Use Area code point — a Nerd Font glyph.
fn has_private_use(line: &str) -> bool {
    line.chars()
        .any(|c| ('\u{e000}'..='\u{f8ff}').contains(&c) || ('\u{f0000}'..='\u{ffffd}').contains(&c))
}

#[test]
fn no_icons_draws_every_layout_without_nerd_font_glyphs() {
    // The desktop app draws the band in its own face, which has none of the
    // Nerd Font's Private Use Area glyphs: each would be a box. Colour stays
    // on here, because the powerline seams are coloured PUA glyphs and only a
    // coloured render would emit them.
    let icons = render_with(
        &["--rows", "6", "--columns", "120", "--layout", "rail"],
        false,
    );
    assert!(
        icons.iter().any(|line| has_private_use(&strip_ansi(line))),
        "the default render draws Nerd Font glyphs, or this test proves nothing"
    );

    for layout in ["compact", "ledger", "budgets", "rail", "anchor", "activity"] {
        let lines = render_with(
            &[
                "--rows",
                "14",
                "--columns",
                "120",
                "--layout",
                layout,
                "--no-icons",
            ],
            false,
        );
        assert!(!lines.is_empty(), "{layout} still renders under --no-icons");
        for line in &lines {
            let plain = strip_ansi(line);
            assert!(
                !has_private_use(&plain),
                "{layout} drew a Nerd Font glyph under --no-icons: {plain}"
            );
        }
    }
}
