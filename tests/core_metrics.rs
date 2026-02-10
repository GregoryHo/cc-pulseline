use std::{fs, path::Path, process::Command};

use cc_pulseline::{
    config::{GlyphMode, RenderConfig},
    render::color::{
        visible_width, COMPLETED_CHECK, COST_HIGH_RATE, COST_LOW_RATE, COST_MED_RATE,
        INDICATOR_CLAUDE_MD, INDICATOR_DURATION, INDICATOR_HOOKS, INDICATOR_MCP, INDICATOR_RULES,
        INDICATOR_SKILLS,
    },
    run_from_str, PulseLineRunner,
};
use serde_json::json;
use tempfile::TempDir;

fn run_cmd(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-C", dir.to_str().expect("utf-8 path")])
        .args(args)
        .status()
        .expect("git command should run");
    assert!(
        status.success(),
        "git command failed: git {}",
        args.join(" ")
    );
}

/// Build a fixture workspace with a fake home directory for test isolation.
/// Returns (workspace_tempdir, fake_home_path inside workspace).
fn build_core_fixture_workspace() -> TempDir {
    let tmp = TempDir::new().expect("tempdir should be created");
    let root = tmp.path();

    // Fake home (empty — no user-level config files)
    let fake_home = root.join("fake_home");
    fs::create_dir_all(fake_home.join(".claude")).expect("fake home .claude dir");

    fs::create_dir_all(root.join(".claude/rules")).expect("rules dir");
    fs::create_dir_all(root.join(".claude/skills/checks")).expect("skill checks dir");
    fs::create_dir_all(root.join(".claude/skills/review")).expect("skill review dir");

    fs::write(root.join("CLAUDE.md"), "# Claude\n").expect("claude md");
    fs::write(root.join(".claude/rules/rule-a.md"), "A\n").expect("rule a");
    fs::write(root.join(".claude/rules/rule-b.md"), "B\n").expect("rule b");
    fs::write(
        root.join(".claude/settings.json"),
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"check"}]}]}}"#,
    )
    .expect("hooks in settings");
    fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers":{"local":{},"remote":{}}}"#,
    )
    .expect("mcp config");

    run_cmd(root, &["init"]);
    run_cmd(root, &["checkout", "-b", "pulseline-test"]);
    fs::write(root.join("dirty.txt"), "dirty\n").expect("dirty marker");

    tmp
}

#[test]
fn renders_core_metrics_from_stdin_and_local_sources() {
    let workspace = build_core_fixture_workspace();
    let cwd = workspace
        .path()
        .to_str()
        .expect("workspace path should be utf-8")
        .to_string();

    let input = json!({
        "session_id": "core-metrics-session",
        "cwd": cwd,
        "workspace": {"current_dir": workspace.path()},
        "model": {"display_name": "Opus 4.6"},
        "output_style": {"name": "explanatory"},
        "version": "2.1.37",
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
        "cost": {
            "total_cost_usd": 3.5,
            "total_duration_ms": 3600000
        }
    })
    .to_string();

    let fake_home = workspace.path().join("fake_home");
    let mut runner = PulseLineRunner::default().with_user_home(fake_home);
    let lines = runner
        .run_from_str(&input, RenderConfig::default())
        .expect("render should succeed");

    assert_eq!(
        lines.len(),
        3,
        "core rendering should produce exactly three lines"
    );

    assert!(
        lines[0].contains("M:Opus 4.6"),
        "line1 should include model"
    );
    assert!(
        lines[0].contains("S:explanatory"),
        "line1 should include output style"
    );
    assert!(
        lines[0].contains("CC:2.1.37"),
        "line1 should include version"
    );
    assert!(lines[0].contains("P:"), "line1 should include project path");
    assert!(
        lines[0].contains("G:pulseline-test"),
        "line1 should include git branch"
    );
    assert!(
        lines[0].contains('*'),
        "line1 should mark dirty working tree"
    );

    assert!(
        lines[1].contains("1 CLAUDE.md"),
        "line2 should show CLAUDE.md count in value-first format"
    );
    assert!(lines[1].contains(" rules"), "line2 should show rules label");
    assert!(
        lines[1].contains("1 hooks"),
        "line2 should show hooks count"
    );
    assert!(lines[1].contains("2 MCPs"), "line2 should show MCP count");
    assert!(
        lines[1].contains(" skills"),
        "line2 should show skills label"
    );
    assert!(
        lines[1].ends_with("1h"),
        "line2 should show elapsed time as 1h"
    );
    assert_eq!(
        lines[2],
        "CTX:43% (86.0k/200.0k) | TOK I: 10 O: 20 C:30/40 | $3.50 ($3.50/h)"
    );
}

#[test]
fn renders_with_nerd_font_icons() {
    let input = json!({
        "model": {"display_name": "Opus"},
        "output_style": {"name": "concise"},
        "version": "2.0.0",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 50,
            "current_usage": {
                "input_tokens": 100,
                "output_tokens": 200,
                "cache_creation_input_tokens": 300,
                "cache_read_input_tokens": 400
            }
        },
        "cost": {
            "total_cost_usd": 1.0,
            "total_duration_ms": 3600000
        }
    })
    .to_string();

    let config = RenderConfig {
        glyph_mode: GlyphMode::Icon,
        ..RenderConfig::default()
    };

    let lines = run_from_str(&input, config).expect("render should succeed");
    assert!(
        lines[0].contains('\u{e26d}'),
        "line1 should contain model icon"
    );
    assert!(
        lines[1].contains('\u{f0219}'),
        "line2 should contain claude_md icon"
    );
}

#[test]
fn renders_with_colors() {
    let input = json!({
        "model": {"display_name": "Opus"},
        "output_style": {"name": "concise"},
        "version": "2.0.0",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 50,
            "current_usage": {
                "input_tokens": 100,
                "output_tokens": 200,
                "cache_creation_input_tokens": 300,
                "cache_read_input_tokens": 400
            }
        },
        "cost": {
            "total_cost_usd": 1.0,
            "total_duration_ms": 3600000
        }
    })
    .to_string();

    let lines = run_from_str(&input, colored_config()).expect("render should succeed");
    assert!(
        lines[0].contains("\x1b["),
        "line1 should contain ANSI escape codes"
    );
    assert!(
        lines[1].contains("\x1b["),
        "line2 should contain ANSI escape codes"
    );
    assert!(
        lines[2].contains("\x1b["),
        "line3 should contain ANSI escape codes"
    );
}

#[test]
fn formats_large_numbers_with_suffixes() {
    let input = json!({
        "model": {"display_name": "Opus"},
        "output_style": {"name": "concise"},
        "version": "2.0.0",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 50,
            "current_usage": {
                "input_tokens": 1_500_000,
                "output_tokens": 250_000,
                "cache_creation_input_tokens": 500,
                "cache_read_input_tokens": 75_000
            }
        },
        "cost": {
            "total_cost_usd": 1.0,
            "total_duration_ms": 3600000
        }
    })
    .to_string();

    let lines = run_from_str(&input, RenderConfig::default()).expect("render should succeed");
    assert!(
        lines[2].contains("1.5M"),
        "input_tokens should use M suffix"
    );
    assert!(
        lines[2].contains("250.0k"),
        "output_tokens should use k suffix"
    );
    assert!(
        lines[2].contains("C:500/75.0k"),
        "cache should show create/read as combined pair"
    );
}

#[test]
fn width_degradation_respects_ansi_codes() {
    let input = json!({
        "model": {"display_name": "Opus"},
        "output_style": {"name": "concise"},
        "version": "2.0.0",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 50,
            "current_usage": {
                "input_tokens": 100,
                "output_tokens": 200,
                "cache_creation_input_tokens": 300,
                "cache_read_input_tokens": 400
            }
        },
        "cost": {
            "total_cost_usd": 1.0,
            "total_duration_ms": 3600000
        }
    })
    .to_string();

    let config = RenderConfig {
        terminal_width: Some(60),
        ..colored_config()
    };

    let lines = run_from_str(&input, config).expect("render should succeed");
    for line in &lines {
        let vis = visible_width(line);
        assert!(vis <= 60, "visible width {vis} exceeds 60 for line");
    }
}

#[test]
fn falls_back_when_stdin_fields_are_missing() {
    let input = fs::read_to_string("tests/fixtures/missing_statusline_input.json")
        .expect("missing field fixture should exist");

    let lines = run_from_str(&input, RenderConfig::default()).expect("render should succeed");

    assert_eq!(
        lines.len(),
        3,
        "fallback render should keep core line count"
    );
    assert!(
        lines[0].contains("M:unknown"),
        "model should fall back to unknown"
    );
    assert!(
        lines[0].contains("S:unknown"),
        "style should fall back to unknown"
    );
    assert!(
        lines[0].contains("CC:unknown"),
        "version should fall back to unknown"
    );
    assert!(
        lines[1].contains("0 CLAUDE.md"),
        "missing env should render zero CLAUDE.md"
    );
    assert!(
        lines[1].contains("0 rules"),
        "missing env should render zero rules"
    );
    assert!(
        lines[1].contains("0 hooks"),
        "missing env should render zero hooks"
    );
    assert!(
        lines[1].contains("0 MCPs"),
        "missing env should render zero MCPs"
    );
    assert!(
        lines[1].contains("0 skills"),
        "missing env should render zero skills"
    );
    assert_eq!(
        lines[2],
        "CTX:--% (--/--) | TOK I: -- O: -- C:--/-- | $0.00 ($0.00/h)"
    );
}

fn cost_payload(total_cost_usd: f64) -> String {
    json!({
        "model": {"display_name": "Opus"},
        "output_style": {"name": "concise"},
        "version": "2.0.0",
        "cost": { "total_cost_usd": total_cost_usd, "total_duration_ms": 3600000 }
    })
    .to_string()
}

fn colored_config() -> RenderConfig {
    RenderConfig {
        color_enabled: true,
        ..RenderConfig::default()
    }
}

#[test]
fn cost_coloring_low_rate() {
    // $3.50 over 1 hour = $3.50/h (< $10/h)
    let lines = run_from_str(&cost_payload(3.5), colored_config()).expect("render should succeed");
    assert!(
        lines[2].contains(COST_LOW_RATE),
        "rate <$10/h should use COST_LOW_RATE"
    );
}

#[test]
fn cost_coloring_med_rate() {
    // $25 over 1 hour = $25/h ($10-50/h)
    let lines = run_from_str(&cost_payload(25.0), colored_config()).expect("render should succeed");
    assert!(
        lines[2].contains(COST_MED_RATE),
        "rate $10-50/h should use COST_MED_RATE"
    );
}

#[test]
fn cost_coloring_high_rate() {
    // $100 over 1 hour = $100/h (> $50/h)
    let lines =
        run_from_str(&cost_payload(100.0), colored_config()).expect("render should succeed");
    assert!(
        lines[2].contains(COST_HIGH_RATE),
        "rate >$50/h should use COST_HIGH_RATE"
    );
}

#[test]
fn line2_indicator_colors_with_nerd_font() {
    let input = json!({
        "model": {"display_name": "Opus"},
        "output_style": {"name": "concise"},
        "version": "2.0.0",
        "cost": { "total_cost_usd": 1.0, "total_duration_ms": 3600000 }
    })
    .to_string();

    let config = RenderConfig {
        glyph_mode: GlyphMode::Icon,
        ..colored_config()
    };

    let lines = run_from_str(&input, config).expect("render should succeed");
    let l2 = &lines[1];
    assert!(
        l2.contains(INDICATOR_CLAUDE_MD),
        "L2 icon should use INDICATOR_CLAUDE_MD color"
    );
    assert!(
        l2.contains(INDICATOR_RULES),
        "L2 icon should use INDICATOR_RULES color"
    );
    assert!(
        l2.contains(INDICATOR_HOOKS),
        "L2 icon should use INDICATOR_HOOKS color"
    );
    assert!(
        l2.contains(INDICATOR_MCP),
        "L2 icon should use INDICATOR_MCP color"
    );
    assert!(
        l2.contains(INDICATOR_SKILLS),
        "L2 icon should use INDICATOR_SKILLS color"
    );
    assert!(
        l2.contains(INDICATOR_DURATION),
        "L2 icon should use INDICATOR_DURATION color"
    );
}

#[test]
fn completed_tools_use_check_color() {
    // This test needs a tool completion in the transcript.
    // Use run_from_str with activity — but since that's stateless and won't
    // have tools, let's directly test the format_tool_line via RenderFrame.
    use cc_pulseline::render::layout::render_frame;
    use cc_pulseline::types::{CompletedToolCount, RenderFrame};

    let mut frame = RenderFrame::default();
    frame.completed_tools.push(CompletedToolCount {
        name: "Read".to_string(),
        count: 5,
    });

    let lines = render_frame(&frame, &colored_config());
    // Tool line should be the 4th line (after L1/L2/L3)
    assert!(lines.len() >= 4, "should have a tool line");
    assert!(
        lines[3].contains(COMPLETED_CHECK),
        "completed tool checkmark should use COMPLETED_CHECK color"
    );
}
