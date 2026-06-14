//! In-process layout preview for `--preview-layouts`.
//!
//! Renders every shipping layout against a synthetic busy/idle frame at the
//! requested widths — no config-file mutation, no transcript, no env/git
//! collection. Lives outside `main.rs` to keep the binary entry point under
//! the file-size norm.

use crate::config::{build_render_config, load_merged_config, PulselineConfig, RenderConfig};
use crate::render::layout::render_frame;
use crate::render::pane::LayoutStyle;
use crate::types::{
    AgentSummary, CompletedToolCount, Line1Metrics, Line2Metrics, Line3Metrics, QuotaMetrics,
    RenderFrame, TodoInProgressItem, TodoSummary, ToolSummary,
};

/// Layouts rendered by `--preview-layouts`, minimal → maximal density.
/// Must stay in sync with `config::parse_layout_name`'s valid list
/// (`none | compact | console | ledger | badge`).
const PREVIEW_LAYOUTS: &[(LayoutStyle, &str)] = &[
    (LayoutStyle::None, "none"),
    (LayoutStyle::Compact, "compact"),
    (LayoutStyle::Console, "console"),
    (LayoutStyle::Ledger, "ledger"),
    (LayoutStyle::Badge, "badge"),
];

const DEFAULT_WIDTH: usize = 140;

/// Render every layout × width × (busy | idle) combination to stdout.
/// Empty `widths` falls back to `[DEFAULT_WIDTH]`.
pub fn preview_layouts(widths: &[usize]) {
    let widths: Vec<usize> = if widths.is_empty() {
        vec![DEFAULT_WIDTH]
    } else {
        widths.to_vec()
    };

    // Same config path as a normal render: merged user + project TOML.
    // No stdin payload here, so the project root resolves from cwd (the
    // same fallback `--check`/`--print` use). Color follows the normal
    // `build_render_config` NO_COLOR resolution.
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(String::from));
    let pulseline = load_merged_config(cwd.as_deref());
    let base = build_render_config(&pulseline);

    let busy = busy_frame();
    let idle = idle_frame();

    for width in widths {
        for (style, name) in PREVIEW_LAYOUTS {
            for (variant, frame) in [("busy", &busy), ("idle", &idle)] {
                println!("── {name} @ {width} cols ({variant}) ──");
                let cfg = layout_config(&base, &pulseline, *style, width);
                for line in render_frame(frame, &cfg) {
                    println!("{line}");
                }
                println!();
            }
        }
    }
}

/// Clone the effective config, point it at `style` × `width`, and force
/// every segment toggle on (kitchen sink) so no layout difference hides
/// behind a disabled segment.
fn layout_config(
    base: &RenderConfig,
    pulseline: &PulselineConfig,
    style: LayoutStyle,
    width: usize,
) -> RenderConfig {
    let mut cfg = base.clone();
    cfg.pane_style = style;
    cfg.terminal_width = Some(width);
    // Visual specs: reset to the *raw* user TOML values so empty fields
    // fall back to each previewed layout's own default via
    // `effective_*_visual()`. `build_render_config` resolved them against
    // the user's configured layout, which isn't the one being previewed.
    cfg.context_visual = pulseline.segments.budget.context_visual.clone();
    cfg.quota_visual = pulseline.segments.quota.visual.clone();
    cfg.agents_visual = pulseline.segments.agents.visual.clone();
    cfg.tools_visual = pulseline.segments.tools.visual.clone();
    cfg.todo_visual = pulseline.segments.todo.visual.clone();
    // Kitchen sink: every show_* toggle on.
    cfg.show_model = true;
    cfg.show_style = true;
    cfg.show_version = true;
    cfg.show_project = true;
    cfg.show_git = true;
    cfg.show_git_stats = true;
    cfg.show_agent = true;
    cfg.show_worktree = true;
    cfg.show_effort = true;
    cfg.show_thinking = true;
    cfg.show_claude_md = true;
    cfg.show_rules = true;
    cfg.show_memory = true;
    cfg.show_hooks = true;
    cfg.show_mcp = true;
    cfg.show_skills = true;
    cfg.show_plugins = true;
    cfg.show_duration = true;
    cfg.show_context = true;
    cfg.show_tokens = true;
    cfg.show_cost = true;
    cfg.show_speed = true;
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    cfg.show_quota_seven_day = true;
    cfg.show_tools = true;
    cfg.show_agents = true;
    cfg.show_todo = true;
    cfg
}

/// Synthetic kitchen-sink frame: identity, CTX ~43%, cost, 5h/7d quota,
/// two running tools with targets, completed counts with one failure, one
/// active agent with description + model, todo 1/3. Mirrors the frame
/// builders in `tests/display_axes.rs`.
fn busy_frame() -> RenderFrame {
    RenderFrame {
        line1: Line1Metrics {
            model: "Opus 4.7".to_string(),
            output_style: "explanatory".to_string(),
            claude_code_version: "2.1.119".to_string(),
            project_path: "~/proj".to_string(),
            git_branch: "feat/x".to_string(),
            git_dirty: true,
            git_ahead: 2,
            git_modified: 3,
            git_added: 1,
            ..Line1Metrics::default()
        },
        line2: Line2Metrics {
            claude_md_count: 1,
            rules_count: 2,
            memory_count: 3,
            hooks_count: 1,
            mcp_count: 2,
            skills_count: 2,
            elapsed_minutes: 60,
            ..Line2Metrics::default()
        },
        line3: Line3Metrics {
            context_window_size: Some(200_000),
            context_used_percentage: Some(43),
            input_tokens: Some(10),
            output_tokens: Some(20),
            total_cost_usd: Some(3.50),
            total_duration_ms: Some(60_000 * 30), // 30 min → $7/h
            ..Line3Metrics::default()
        },
        ctx_history: vec![
            (10, 1_000),
            (20, 2_000),
            (30, 3_000),
            (35, 4_000),
            (40, 5_000),
            (43, 6_000),
        ],
        quota: QuotaMetrics {
            five_hour_pct: Some(75.0),
            five_hour_reset_minutes: Some(120),
            seven_day_pct: Some(40.0),
            seven_day_reset_minutes: Some(60 * 24 * 3),
        },
        tools: vec![
            ToolSummary {
                id: "1".to_string(),
                name: "Read".to_string(),
                target: Some("src/main.rs".to_string()),
            },
            ToolSummary {
                id: "2".to_string(),
                name: "Bash".to_string(),
                target: Some("cargo test".to_string()),
            },
        ],
        completed_tools: vec![
            CompletedToolCount {
                name: "Read".to_string(),
                count: 12,
                last_completed_at: None,
                failed: 0,
            },
            CompletedToolCount {
                name: "Bash".to_string(),
                count: 8,
                last_completed_at: None,
                failed: 2,
            },
            CompletedToolCount {
                name: "Edit".to_string(),
                count: 5,
                last_completed_at: None,
                failed: 0,
            },
        ],
        agents: vec![AgentSummary {
            id: "a1".to_string(),
            description: "Investigate logic".to_string(),
            agent_type: Some("Explore".to_string()),
            started_at: None,
            model: Some("haiku".to_string()),
            completed_at: None,
            message_id: None,
            agent_id: None,
            total_duration_ms: None,
            total_tokens: None,
            total_tool_use_count: None,
        }],
        todo: Some(TodoSummary {
            text: "Fixing auth bug".to_string(),
            pending: 1,
            completed: 1,
            total: 3,
            in_progress_items: vec![TodoInProgressItem {
                text: "Fixing auth bug".to_string(),
                started_at: None,
            }],
            all_done: false,
            is_task_api: true,
            sub_agent_count: None,
        }),
        ..RenderFrame::default()
    }
}

/// The busy frame with all activity stripped — shows each layout's idle
/// footprint (e.g. compact's 1-row contract).
fn idle_frame() -> RenderFrame {
    let mut f = busy_frame();
    f.tools.clear();
    f.completed_tools.clear();
    f.agents.clear();
    f.todo = None;
    f
}
