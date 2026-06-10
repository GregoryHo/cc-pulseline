//! Height-degradation ladder (`max_total_lines` + `HeightDegradeStrategy`).
//!
//! Builds busy `RenderFrame`s directly and calls `render_frame` so every
//! assertion is deterministic — no transcript plumbing. Color is disabled
//! and glyphs are ASCII throughout so rows can be matched by prefix
//! (`T:` running tools, `✓`/`[done]` completed, `A:` agents, `TODO:`).

use cc_pulseline::config::{GlyphMode, HeightDegradeStrategy, RenderConfig};
use cc_pulseline::render::color::resolve_palette;
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::types::{
    AgentSummary, CompletedToolCount, QuotaMetrics, RenderFrame, TodoInProgressItem, TodoSummary,
    ToolSummary,
};

fn cfg() -> RenderConfig {
    RenderConfig {
        color_enabled: false,
        glyph_mode: GlyphMode::Ascii,
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        show_quota: true,
        show_quota_five_hour: true,
        max_tool_lines: 2,
        max_completed_tools: 8,
        max_completed_lines: 2,
        max_agent_lines: 3,
        max_todo_lines: 2,
        ..RenderConfig::default()
    }
}

/// Busy mid-session frame: completed tools, running tools, sequential
/// agents, multi-item todo, quota with data.
fn busy_frame() -> RenderFrame {
    let mut frame = RenderFrame::default();
    frame.line1.model = "Opus".to_string();
    frame.line1.output_style = "concise".to_string();
    frame.line1.claude_code_version = "2.2.0".to_string();
    frame.line1.project_path = "~/proj".to_string();
    frame.line1.git_branch = "main".to_string();

    frame.completed_tools = (0..6)
        .map(|i| CompletedToolCount {
            name: format!("Tool{i}"),
            count: 10 + i as u32,
            last_completed_at: None,
            failed: 0,
        })
        .collect();
    frame.tools = vec![
        ToolSummary {
            id: "t1".to_string(),
            name: "Bash".to_string(),
            target: Some("cargo test".to_string()),
        },
        ToolSummary {
            id: "t2".to_string(),
            name: "Read".to_string(),
            target: Some("src/main.rs".to_string()),
        },
    ];
    frame.agents = (0..3)
        .map(|i| AgentSummary {
            id: format!("a{i}"),
            description: format!("agent number {i}"),
            agent_type: Some("Explore".to_string()),
            started_at: Some(1_000),
            model: None,
            completed_at: if i == 0 { None } else { Some(61_000) },
            message_id: Some(format!("msg_{i}")),
            agent_id: None,
            total_duration_ms: None,
            total_tokens: None,
            total_tool_use_count: None,
        })
        .collect();
    frame.todo = Some(TodoSummary {
        text: String::new(),
        pending: 2,
        completed: 1,
        total: 5,
        in_progress_items: vec![
            TodoInProgressItem {
                text: "fix the parser".to_string(),
                started_at: None,
            },
            TodoInProgressItem {
                text: "write the docs".to_string(),
                started_at: None,
            },
        ],
        all_done: false,
        is_task_api: true,
        sub_agent_count: None,
    });
    frame.quota = QuotaMetrics {
        five_hour_pct: Some(62.0),
        five_hour_reset_minutes: Some(120),
        seven_day_pct: None,
        seven_day_reset_minutes: None,
    };
    frame
}

fn count_activity_rows(lines: &[String]) -> usize {
    lines
        .iter()
        .filter(|l| {
            l.starts_with("T:")
                || l.starts_with('\u{2713}')
                || l.starts_with("A:")
                || l.starts_with("TODO:")
        })
        .count()
}

#[test]
fn no_cap_renders_everything() {
    let lines = render_frame(&busy_frame(), &cfg());
    // L1 + L2 + L3 + quota + completed + running + 3 agents + 2 todo.
    assert!(
        lines.len() >= 9,
        "uncapped busy frame should be tall: {} rows\n{}",
        lines.len(),
        lines.join("\n")
    );
}

#[test]
fn cap_is_enforced_on_flat_layout() {
    let config = RenderConfig {
        max_total_lines: Some(5),
        ..cfg()
    };
    let lines = render_frame(&busy_frame(), &config);
    assert!(
        lines.len() <= 5,
        "cap=5 exceeded: {} rows\n{}",
        lines.len(),
        lines.join("\n")
    );
    // Core metrics survive: identity first, budget row present.
    assert!(
        lines[0].contains("M:Opus"),
        "L1 must survive: {:?}",
        lines[0]
    );
    assert!(
        lines.iter().any(|l| l.contains("CTX")),
        "L3 must survive:\n{}",
        lines.join("\n")
    );
}

#[test]
fn first_rung_drops_running_tools_only() {
    let uncapped = render_frame(&busy_frame(), &cfg());
    // Cap exactly one row under the natural height: the first rung
    // (DropRunningTools) must be sufficient, leaving everything else.
    let config = RenderConfig {
        max_total_lines: Some(uncapped.len() - 1),
        ..cfg()
    };
    let lines = render_frame(&busy_frame(), &config);
    assert!(
        !lines.iter().any(|l| l.starts_with("T:")),
        "running-tools row must be dropped first:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|l| l.starts_with('\u{2713}')),
        "completed counts must survive the first rung:\n{}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|l| l.starts_with("A:")),
        "agents must survive the first rung:\n{}",
        lines.join("\n")
    );
}

#[test]
fn merge_quota_rung_appends_compact_quota_to_l3() {
    let mut config = cfg();
    config.height_degrade_order = vec![HeightDegradeStrategy::MergeQuotaIntoL3];
    let uncapped = render_frame(&busy_frame(), &config);
    config.max_total_lines = Some(uncapped.len() - 1);
    let lines = render_frame(&busy_frame(), &config);

    let l3 = lines
        .iter()
        .find(|l| l.contains("CTX"))
        .expect("L3 present");
    assert!(
        l3.contains("5h:62%"),
        "compact quota must ride on L3: {l3:?}"
    );
    assert!(
        !lines.iter().any(|l| l.starts_with("Q:")),
        "standalone quota row must be gone:\n{}",
        lines.join("\n")
    );
}

#[test]
fn merge_activity_rung_fuses_activity_into_one_row() {
    let mut config = cfg();
    config.height_degrade_order = vec![HeightDegradeStrategy::MergeActivity];
    config.max_total_lines = Some(5); // forces the single configured rung
    let lines = render_frame(&busy_frame(), &config);

    assert_eq!(
        count_activity_rows(&lines),
        1,
        "activity must fuse into one row:\n{}",
        lines.join("\n")
    );
    let merged = lines
        .iter()
        .find(|l| l.starts_with('\u{2713}') || l.starts_with("T:") || l.starts_with("A:"))
        .expect("merged activity row");
    // Grand total of completed counts: 10+11+...+15 = 75.
    assert!(
        merged.contains("75 tools"),
        "fused row must carry the completed grand total: {merged:?}"
    );
}

#[test]
fn cap_counts_frame_chrome_rows() {
    let config = RenderConfig {
        max_total_lines: Some(6),
        terminal_width: Some(120),
        pane_style: cc_pulseline::render::pane::LayoutStyle::Sections,
        ..cfg()
    };
    let lines = render_frame(&busy_frame(), &config);
    assert!(
        lines.len() <= 6,
        "chrome rows must count against the cap: {} rows\n{}",
        lines.len(),
        lines.join("\n")
    );
}

#[test]
fn ladder_exhausted_hard_truncates_from_bottom() {
    let config = RenderConfig {
        max_total_lines: Some(2),
        ..cfg()
    };
    let lines = render_frame(&busy_frame(), &config);
    assert_eq!(lines.len(), 2, "hard cap: {}", lines.join("\n"));
    assert!(
        lines[0].contains("M:Opus"),
        "identity row survives at the top: {:?}",
        lines[0]
    );
}

#[test]
fn cap_larger_than_render_is_a_no_op() {
    let base = render_frame(&busy_frame(), &cfg());
    let config = RenderConfig {
        max_total_lines: Some(50),
        ..cfg()
    };
    let capped = render_frame(&busy_frame(), &config);
    assert_eq!(base, capped, "roomy cap must not alter the render");
}

// ── Compact layout ───────────────────────────────────────────────────

#[test]
fn compact_layout_is_two_rows_when_busy() {
    let config = RenderConfig {
        pane_style: cc_pulseline::render::pane::LayoutStyle::Compact,
        ..cfg()
    };
    let lines = render_frame(&busy_frame(), &config);
    assert_eq!(
        lines.len(),
        2,
        "compact busy frame must be exactly 2 rows:\n{}",
        lines.join("\n")
    );
    // Row 1 fuses identity + budget + compact quota.
    assert!(
        lines[0].contains("M:Opus"),
        "identity on row 1: {:?}",
        lines[0]
    );
    assert!(lines[0].contains("CTX"), "budget on row 1: {:?}", lines[0]);
    assert!(
        lines[0].contains("5h:62%"),
        "compact quota on row 1: {:?}",
        lines[0]
    );
    // Row 2 is the packed activity ticker.
    assert!(
        lines[1].contains("75 tools") && lines[1].contains("TODO:1/5"),
        "ticker on row 2: {:?}",
        lines[1]
    );
}

#[test]
fn compact_layout_is_one_row_when_idle() {
    let mut frame = busy_frame();
    frame.completed_tools.clear();
    frame.tools.clear();
    frame.agents.clear();
    frame.todo = None;
    let config = RenderConfig {
        pane_style: cc_pulseline::render::pane::LayoutStyle::Compact,
        ..cfg()
    };
    let lines = render_frame(&frame, &config);
    assert_eq!(
        lines.len(),
        1,
        "idle compact frame must be exactly 1 row:\n{}",
        lines.join("\n")
    );
}

#[test]
fn compact_layout_fits_terminal_width() {
    let config = RenderConfig {
        pane_style: cc_pulseline::render::pane::LayoutStyle::Compact,
        terminal_width: Some(80),
        ..cfg()
    };
    let lines = render_frame(&busy_frame(), &config);
    for line in &lines {
        assert!(
            cc_pulseline::render::color::visible_width(line) <= 80,
            "row exceeds width 80: {line:?}"
        );
    }
}

#[test]
fn compact_head_keeps_cost_and_quota_under_width_pressure() {
    // Regression for the PR-review screenshot: row 1 used to be a blind
    // right-tail truncation, cutting cost/quota (the right-side
    // essentials) while style/version/path survived. The packed form
    // must drop Optional reference cells first.
    let mut frame = busy_frame();
    frame.line1.project_path = "~/GitHub/AI/cc-pulseline".to_string();
    frame.line1.claude_code_version = "2.1.170".to_string();
    frame.line1.output_style = "default".to_string();
    let config = RenderConfig {
        pane_style: cc_pulseline::render::pane::LayoutStyle::Compact,
        terminal_width: Some(80),
        ..cfg()
    };
    let lines = render_frame(&frame, &config);
    let head = &lines[0];
    assert!(
        cc_pulseline::render::color::visible_width(head) <= 80,
        "head must fit width 80: {head:?}"
    );
    assert!(
        head.contains("$") || head.contains("CTX"),
        "budget cells must survive: {head:?}"
    );
    assert!(
        head.contains("5h:62%"),
        "quota must survive width pressure: {head:?}"
    );
    assert!(
        !head.contains("cc-pulseline") && !head.contains("2.1.170"),
        "Optional reference cells must drop first: {head:?}"
    );
}
