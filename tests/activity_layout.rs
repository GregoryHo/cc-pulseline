//! `activity` layout — the band's canvas: activity rows and nothing else.
//!
//! Two contracts are asserted here, and they are the whole reason the layout
//! exists:
//!
//! 1. **No core rows.** Every other layout leads with identity and budget,
//!    and the height ladder's floor keeps one fused core row however hard it
//!    is squeezed. On the band that row is a second copy of the statusline.
//! 2. **The running tool is row 0.** The generic ladder's first rung is
//!    `DropRunningTools`, so on the default layout a two-row band loses the
//!    tool in flight before anything else. Here the order is inverted, and a
//!    cap cuts the cumulative counts instead.
//!
//! Frames are built directly and `render_frame` is called, so assertions are
//! deterministic. Colour off, Ascii glyphs.

use cc_pulseline::config::{GlyphMode, RenderConfig};
use cc_pulseline::render::color::resolve_palette;
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{
    AgentSummary, CompletedToolCount, QuotaMetrics, RenderFrame, TodoSummary, ToolSummary,
};

fn cfg(rows: Option<usize>) -> RenderConfig {
    RenderConfig {
        color_enabled: false,
        glyph_mode: GlyphMode::Ascii,
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        pane_style: LayoutStyle::Activity,
        terminal_width: Some(100),
        max_total_lines: rows,
        show_quota: true,
        show_quota_five_hour: true,
        ..RenderConfig::default()
    }
}

/// Core metrics AND activity, so a leaked core row is visible as a leak
/// rather than as missing data.
fn busy_frame() -> RenderFrame {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus".to_string();
    f.line1.claude_code_version = "2.2.0".to_string();
    f.line1.project_path = "~/proj".to_string();
    f.line1.git_branch = "main".to_string();
    f.line3.context_window_size = Some(200_000);
    f.line3.context_used_percentage = Some(43);
    f.line3.total_cost_usd = Some(3.50);
    f.quota = QuotaMetrics {
        five_hour_pct: Some(75.0),
        five_hour_reset_minutes: Some(120),
        ..QuotaMetrics::default()
    };
    f.tools = vec![ToolSummary {
        id: "t1".to_string(),
        name: "Bash".to_string(),
        target: Some("cargo test --release".to_string()),
    }];
    f.completed_tools = vec![
        CompletedToolCount {
            name: "Read".to_string(),
            count: 42,
            ..CompletedToolCount::default()
        },
        CompletedToolCount {
            name: "Bash".to_string(),
            count: 31,
            failed: 2,
            ..CompletedToolCount::default()
        },
    ];
    f.completed_tool_total = 91;
    f.failed_tool_total = 2;
    f.agents = vec![AgentSummary {
        id: "a1".to_string(),
        description: "Investigate the collector".to_string(),
        agent_type: Some("Explore".to_string()),
        ..AgentSummary::default()
    }];
    f.todo = Some(TodoSummary {
        text: "Wire the hub".to_string(),
        pending: 2,
        completed: 1,
        total: 3,
        ..TodoSummary::default()
    });
    f
}

/// Steady state present, nothing running. What the band sees between turns.
fn idle_frame() -> RenderFrame {
    let mut f = busy_frame();
    f.tools.clear();
    f.completed_tools.clear();
    f.completed_tool_total = 0;
    f.failed_tool_total = 0;
    f.agents.clear();
    f.todo = None;
    f
}

#[test]
fn draws_no_core_rows() {
    let lines = render_frame(&busy_frame(), &cfg(None));
    let out = lines.join("\n");
    for leak in [
        "Opus", "2.2.0", "~/proj", "main", "43%", "200.0k", "$3.50", "75%",
    ] {
        assert!(
            !out.contains(leak),
            "core metric {leak:?} leaked into the activity layout:\n{out}"
        );
    }
}

#[test]
fn leads_with_the_running_tool() {
    let lines = render_frame(&busy_frame(), &cfg(None));
    assert!(
        lines[0].contains("cargo test --release"),
        "row 0 must name the tool in flight, got:\n{}",
        lines.join("\n")
    );
}

#[test]
fn orders_running_then_agents_then_todo_then_counts() {
    let lines = render_frame(&busy_frame(), &cfg(None));
    let index_of = |needle: &str| {
        lines
            .iter()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no row containing {needle:?} in:\n{}", lines.join("\n")))
    };
    let running = index_of("cargo test --release");
    let agent = index_of("Investigate the collector");
    let todo = index_of("Wire the hub");
    let counts = index_of("Read");
    assert!(
        running < agent && agent < todo && todo < counts,
        "expected running < agent < todo < counts, got {running}/{agent}/{todo}/{counts}:\n{}",
        lines.join("\n")
    );
}

/// The band outside fullscreen gets `maxRows == 2`. This is the case the
/// layout was added for: on the default layout the same frame at two rows
/// draws identity and budget and no activity at all.
#[test]
fn two_rows_keep_the_running_tool_and_drop_the_counts() {
    let lines = render_frame(&busy_frame(), &cfg(Some(2)));
    assert!(lines.len() <= 2, "expected ≤2 rows, got {}", lines.len());
    assert!(
        lines[0].contains("cargo test --release"),
        "row 0 must still name the tool in flight:\n{}",
        lines.join("\n")
    );
    assert!(
        !lines.iter().any(|l| l.contains("x42")),
        "the cumulative per-tool counts should be the rows that give way:\n{}",
        lines.join("\n")
    );
}

#[test]
fn one_row_is_the_running_tool() {
    let lines = render_frame(&busy_frame(), &cfg(Some(1)));
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("cargo test --release"),
        "the single surviving row must be the live one:\n{}",
        lines[0]
    );
}

/// The cap ladder, which the other cases cannot see: every `max_*_lines`
/// defaults to 1, so on a default config all three tightening steps are
/// no-ops and the bottom truncation does all the work. They earn their keep
/// only where the person raised one.
///
/// Four groups at one row each says more than two groups at three, so
/// tightening runs before truncation — and on the expendable end first, so
/// the live rows are the last to be squeezed.
#[test]
fn over_cap_it_shrinks_groups_before_dropping_any() {
    let mut f = busy_frame();
    f.agents = vec![
        AgentSummary {
            id: "a1".to_string(),
            description: "Investigate the collector".to_string(),
            agent_type: Some("Explore".to_string()),
            ..AgentSummary::default()
        },
        AgentSummary {
            id: "a2".to_string(),
            description: "Audit the ladder".to_string(),
            agent_type: Some("Explore".to_string()),
            ..AgentSummary::default()
        },
        AgentSummary {
            id: "a3".to_string(),
            description: "Map the frames".to_string(),
            agent_type: Some("Explore".to_string()),
            ..AgentSummary::default()
        },
    ];
    let cfg = RenderConfig {
        max_agent_lines: 3,
        max_completed_lines: 2,
        // Narrow enough that two completed-tool cells need two rows.
        terminal_width: Some(40),
        max_total_lines: Some(4),
        ..cfg(None)
    };

    let lines = render_frame(&f, &cfg);
    let out = lines.join("\n");
    assert!(
        lines.len() <= 4,
        "expected ≤4 rows, got {}:\n{out}",
        lines.len()
    );
    assert!(
        lines[0].contains("cargo test"),
        "row 0 is still the live one:\n{out}"
    );
    // All four groups survive, each shrunk. Truncating instead would have
    // kept the running row and three agent rows and lost todo and counts
    // entirely — which is the regression this guards.
    assert!(out.contains("A:Explore"), "agents gone:\n{out}");
    assert!(out.contains("TODO:"), "todo gone:\n{out}");
    assert!(out.contains("Read"), "completed counts gone:\n{out}");
    // One agent row rather than three, with the other two folded into its
    // ` +N` tail: the agent cap was tightened, not the group dropped.
    assert_eq!(
        lines.iter().filter(|l| l.contains("A:Explore")).count(),
        1,
        "agents not collapsed to one row:\n{out}"
    );
    assert!(
        out.contains("+2"),
        "the folded-agent tail is missing:\n{out}"
    );
}

/// The tightening *order*: counts before agents, not the other way round.
///
/// Built so either collapse alone would fit the cap, which makes the two
/// orders produce different pages. Counts-first keeps all three agent rows
/// and folds the counts to one; agents-first does the opposite. The stated
/// priority is running > agents > todo > counts, so the counts are what give
/// up their rows.
#[test]
fn it_shrinks_the_counts_before_the_agents() {
    let mut f = busy_frame();
    f.todo = None;
    f.agents = (1..=3)
        .map(|i| AgentSummary {
            id: format!("a{i}"),
            description: format!("Agent number {i}"),
            agent_type: Some("Explore".to_string()),
            ..AgentSummary::default()
        })
        .collect();
    f.completed_tools = ["Read", "Bash", "Edit", "Grep"]
        .iter()
        .map(|name| CompletedToolCount {
            name: (*name).to_string(),
            count: 40,
            ..CompletedToolCount::default()
        })
        .collect();
    let cfg = RenderConfig {
        max_agent_lines: 3,
        max_completed_lines: 3,
        max_completed_tools: 4,
        // Narrow enough that four completed cells need three rows.
        terminal_width: Some(26),
        max_total_lines: Some(5),
        ..cfg(None)
    };

    let lines = render_frame(&f, &cfg);
    let out = lines.join("\n");
    assert!(
        lines.len() <= 5,
        "expected ≤5 rows, got {}:\n{out}",
        lines.len()
    );
    assert_eq!(
        lines.iter().filter(|l| l.contains("A:Explore")).count(),
        3,
        "all three agent rows should survive — the counts give up their rows \
         first, not the agents:\n{out}"
    );
    assert_eq!(
        lines.iter().filter(|l| l.contains("\u{2713}")).count(),
        1,
        "the completed counts should be folded to one row:\n{out}"
    );
}

#[test]
fn idle_renders_nothing() {
    let lines = render_frame(&idle_frame(), &cfg(Some(2)));
    assert!(
        lines.is_empty(),
        "an idle band must yield rather than draw a blank or a stale row, got:\n{lines:?}"
    );
}

/// A long-running agent with no tool in flight still reaches row 0 — the
/// ordering must not depend on the running-tools row existing.
#[test]
fn an_agent_alone_still_draws() {
    let mut f = idle_frame();
    f.agents = busy_frame().agents;
    let lines = render_frame(&f, &cfg(Some(2)));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("Investigate the collector")),
        "got:\n{lines:?}"
    );
}
