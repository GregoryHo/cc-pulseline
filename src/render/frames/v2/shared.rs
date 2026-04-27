//! Shared building blocks for v2 layouts.
//!
//! Each helper returns a string fragment (already colorized when the config
//! enables it) so layouts compose them with their own separators / framing.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::RenderConfig;
use crate::render::color::{colorize, ThemePalette};
use crate::render::fmt::{
    format_agent_elapsed, format_number, format_reset_duration, format_speed,
};
use crate::render::icons::{glyph, ICON_AGENT};
use crate::render::layout;
use crate::render::widgets;
use crate::types::{
    AgentSummary, CompletedToolCount, Line1Metrics, Line3Metrics, QuotaMetrics, RenderFrame,
    TodoSummary, ToolSummary,
};

/// CTX sparkline (braille mini-chart) is opt-in and Nerd Font only — there
/// is no ASCII fallback that conveys the same trend information, so we hide
/// it cleanly under `icons = false` rather than emitting boxes.
pub fn sparkline_enabled(config: &RenderConfig) -> bool {
    config.show_ctx_sparkline && config.glyph_mode.is_icon()
}

/// Whether at least one config-row segment is enabled.
///
/// In v2 the L2 row is opt-in: it stays hidden unless the user has flipped
/// any `show_*` for the config segments. v1 still renders L2 always-on
/// (handled by the legacy code path).
pub fn config_row_enabled(config: &RenderConfig) -> bool {
    config.show_claude_md
        || config.show_rules
        || config.show_memory
        || config.show_hooks
        || config.show_mcp
        || config.show_skills
        || config.show_plugins
}

/// Compact identity headline used by Cockpit & Flightstrip.
///
/// Format: `<model>  <branch>[*][↑n] <project>` — hand-tuned spacing so the
/// eye finds the branch quickly. Each segment honours its `show_*` toggle.
pub fn identity_headline(line1: &Line1Metrics, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;
    let mut parts: Vec<String> = Vec::new();

    if config.show_model {
        parts.push(colorize(&line1.model, &p.primary, color));
    }

    if config.show_agent {
        if let Some(name) = &line1.agent_name {
            parts.push(colorize(name, &p.stable_blue, color));
        }
    }

    if config.show_git {
        let mut s = colorize(&line1.git_branch, p.git_green(), color);
        if line1.git_dirty {
            s.push_str(&colorize("*", p.git_modified(), color));
        }
        if line1.git_ahead > 0 {
            s.push_str(&colorize(
                &format!(" ↑{}", line1.git_ahead),
                p.git_ahead(),
                color,
            ));
        }
        if line1.git_behind > 0 {
            s.push_str(&colorize(
                &format!(" ↓{}", line1.git_behind),
                p.git_behind(),
                color,
            ));
        }
        if config.show_git_stats {
            let stats: Vec<String> = [
                ('!', line1.git_modified, p.git_modified()),
                ('+', line1.git_added, p.git_added()),
                ('✘', line1.git_deleted, p.git_deleted()),
                ('?', line1.git_untracked, &p.structural),
            ]
            .iter()
            .filter(|(_, count, _)| *count > 0)
            .map(|(prefix, count, c)| colorize(&format!("{prefix}{count}"), c, color))
            .collect();
            if !stats.is_empty() {
                s.push(' ');
                s.push_str(&stats.join(" "));
            }
        }
        if config.show_worktree && line1.in_worktree {
            s.push_str(&colorize(" (WT)", &p.structural, color));
        }
        parts.push(s);
    }

    if config.show_project {
        parts.push(colorize(&line1.project_path, &p.secondary, color));
    }

    parts.join("  ")
}

/// Right-edge "CTX 43%·86k" pill for Cockpit's L1.
pub fn ctx_pill(line3: &Line3Metrics, p: &ThemePalette, color_enabled: bool) -> String {
    match (line3.context_used_percentage, line3.context_window_size) {
        (Some(pct), Some(size)) => {
            let pct_color = p.color_for_ctx_pct(pct, Some(size));
            let used = ((size as f64) * (pct as f64) / 100.0) as u64;
            let pct_str = colorize(&format!("{pct}%"), pct_color, color_enabled);
            let dot = colorize("\u{00B7}", &p.separator, color_enabled);
            let tokens = colorize(&format_number(used), &p.primary, color_enabled);
            format!("{pct_str}{dot}{tokens}")
        }
        _ => String::new(),
    }
}

/// Compact L2 config row for v2 (opt-in). Delegates to v1's `format_line2` so
/// the icons, counts, and toggles stay in lockstep across layouts; v2 only
/// adds a leading `CFG  ` label and uses two spaces between segments.
///
/// Width-aware: when the assembled row exceeds `max_width`, segments are
/// progressively turned off in low-value-first order (duration, plugins,
/// skills, mcp, hooks, memory, rules, claude_md) until the row fits or
/// nothing remains.
pub fn config_row(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    max_width: usize,
) -> String {
    use crate::render::color::visible_width;

    const DROP_ORDER: &[fn(&mut RenderConfig)] = &[
        |c| c.show_duration = false,
        |c| c.show_plugins = false,
        |c| c.show_skills = false,
        |c| c.show_mcp = false,
        |c| c.show_hooks = false,
        |c| c.show_memory = false,
        |c| c.show_rules = false,
        |c| c.show_claude_md = false,
    ];

    let prefix = colorize("CFG  ", &p.structural, config.color_enabled);
    let prefix_w = visible_width(&prefix);

    // Fast path: try with the user's config first; only clone when shrinking.
    let body = layout::format_line2(frame, config, "  ", p);
    if body.is_empty() {
        return String::new();
    }
    if prefix_w + visible_width(&body) <= max_width {
        return format!("{prefix}{body}");
    }

    let mut shrunk = config.clone();
    let mut last_body = String::new();
    for drop in DROP_ORDER {
        drop(&mut shrunk);
        last_body = layout::format_line2(frame, &shrunk, "  ", p);
        if last_body.is_empty() {
            return String::new();
        }
        if prefix_w + visible_width(&last_body) <= max_width {
            return format!("{prefix}{last_body}");
        }
    }
    // Even after dropping every optional segment we still overflow — emit the
    // last (still over-budget) body and let CC clip it visibly. Better than a
    // blank row.
    format!("{prefix}{last_body}")
}

/// CTX gauge cell — `gauge_width` cells, followed by `% used` text.
pub fn ctx_gauge_cell(
    line3: &Line3Metrics,
    gauge_width: usize,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    let pct = line3.context_used_percentage.unwrap_or(0);
    let label = colorize("CTX  ", &p.structural, color_enabled);
    let bar = widgets::gauge::render(pct, gauge_width, p, color_enabled);
    let pct_color = p.color_for_ctx_pct(pct, line3.context_window_size);
    let pct_str = colorize(&format!(" {pct}%"), pct_color, color_enabled);
    format!("{label}{bar}{pct_str}")
}

/// CTX sparkline glyph strip (no label) — empty when history is empty.
pub fn ctx_sparkline(history: &[u8], p: &ThemePalette, color_enabled: bool) -> String {
    widgets::sparkline::render(history, p, color_enabled)
}

/// Token-rate widget — "TOK 1.2K/s" with `↗` only when `speed.is_some()`.
/// When the speed is absent, returns a dimmed `TOK --` placeholder so the
/// cluster keeps a stable column.
pub fn token_rate_cell(
    line3: &Line3Metrics,
    speed: Option<f64>,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    let label = colorize("TOK ", &p.structural, color_enabled);
    match speed.or(line3.output_speed_toks_per_sec) {
        Some(s) if s > 0.0 => {
            let val = colorize(&format_speed(s), &p.primary, color_enabled);
            format!("{label}{val}")
        }
        _ => {
            let dash = colorize("--", &p.structural, color_enabled);
            format!("{label}{dash}")
        }
    }
}

/// Cost cell — `$3.50 ◐` with the burn arc when icons are on; falls back to
/// `$3.50 ($1.5/h)` text under `icons = false` so non-Nerd-Font terminals
/// still get the same information.
pub fn cost_cell(line3: &Line3Metrics, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;
    let total = line3.total_cost_usd.unwrap_or(0.0);
    let per_hour = line3
        .total_duration_ms
        .filter(|d| *d > 0)
        .map(|d| total / ((d as f64) / 3_600_000.0))
        .unwrap_or(0.0);

    let total_str = colorize(&format!("${total:.2}"), &p.cost_base, color);
    if config.glyph_mode.is_icon() {
        let arc = widgets::arc::render(per_hour, p, color);
        return format!("{total_str} {arc}");
    }
    if per_hour > 0.0 {
        let rate = colorize(&format!("(${per_hour:.1}/h)"), &p.structural, color);
        format!("{total_str} {rate}")
    } else {
        total_str
    }
}

/// Quota text cell — `Q5h 75% 02h 0m`. Returns empty if no five-hour data.
pub fn quota_text_cell(quota: &QuotaMetrics, p: &ThemePalette, color_enabled: bool) -> String {
    quota_text_cell_for(
        "Q5h ",
        quota.five_hour_pct,
        quota.five_hour_reset_minutes,
        p,
        color_enabled,
    )
}

/// Q7d sibling — same shape as `quota_text_cell` but driven by the seven-day
/// window. Returns empty when CC didn't supply Q7d data (API users) or the
/// user has `show_quota_seven_day = false`.
pub fn quota_seven_day_cell(quota: &QuotaMetrics, p: &ThemePalette, color_enabled: bool) -> String {
    quota_text_cell_for(
        "Q7d ",
        quota.seven_day_pct,
        quota.seven_day_reset_minutes,
        p,
        color_enabled,
    )
}

fn quota_text_cell_for(
    label_text: &str,
    pct: Option<f64>,
    reset_min: Option<u64>,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    let pct = match pct {
        Some(v) => v,
        None => return String::new(),
    };
    let label = colorize(label_text, &p.structural, color_enabled);
    let pct_str = colorize(
        &format!("{pct:.0}%"),
        p.color_for_quota_pct(pct),
        color_enabled,
    );
    let reset_part = reset_min
        .map(|m| {
            colorize(
                &format!(" {}", format_reset_duration(m)),
                &p.structural,
                color_enabled,
            )
        })
        .unwrap_or_default();
    format!("{label}{pct_str}{reset_part}")
}

/// Activity ticker: tools tape + completed counts + agents + todo summary —
/// joined by two spaces. Empty cells are skipped.
pub fn activity_ticker(frame: &RenderFrame, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;
    let mut parts: Vec<String> = Vec::new();

    if config.show_tools {
        if !frame.tools.is_empty() {
            let tape = widgets::tape::render(&frame.tools, config.max_tool_lines.max(1), p, color);
            if !tape.is_empty() {
                parts.push(tape);
            }
        }
        if !frame.completed_tools.is_empty() {
            parts.push(format_completed_summary(&frame.completed_tools, p, color));
        }
    }

    if config.show_agents {
        for agent in frame.agents.iter().take(config.max_agent_lines) {
            parts.push(format_agent_chip(agent, config, p, color));
        }
    }

    if config.show_todo {
        if let Some(todo) = &frame.todo {
            parts.push(format_todo_chip(todo, p, color));
        }
    }

    parts.join("   ")
}

fn format_completed_summary(
    completed: &[CompletedToolCount],
    p: &ThemePalette,
    color: bool,
) -> String {
    let total: u32 = completed.iter().map(|c| c.count).sum();
    let check = colorize("\u{2713}", &p.completed_check, color);
    let count_str = colorize(&format!(" ×{total}"), &p.secondary, color);
    format!("{check}{count_str}")
}

/// `glyph(ICON_AGENT) | "A:"` colored with `agent_purple` — shared between
/// cockpit (`format_agent_chip`) and console (`agent_todo_row`) so the
/// glyph-vs-ascii decision lives in one place and `display.icons = false`
/// degrades both layouts identically.
pub fn agent_prefix(config: &RenderConfig, p: &ThemePalette) -> String {
    colorize(
        &glyph(config.glyph_mode, ICON_AGENT, "A:"),
        p.agent_purple(),
        config.color_enabled,
    )
}

fn format_agent_chip(
    agent: &AgentSummary,
    config: &RenderConfig,
    p: &ThemePalette,
    color: bool,
) -> String {
    let prefix = agent_prefix(config, p);
    let name = match &agent.agent_type {
        Some(t) => t.clone(),
        None => agent
            .description
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(20)
            .collect::<String>(),
    };
    let name_str = colorize(&name, p.agent_purple(), color);
    let model_part = agent
        .model
        .as_ref()
        .map(|m| colorize(&format!(" [{m}]"), &p.structural, color))
        .unwrap_or_default();
    let elapsed = agent.started_at.map(|start| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let secs = now.saturating_sub(start) / 1000;
        format!(" {}", format_agent_elapsed(secs))
    });
    let elapsed_str = elapsed
        .as_deref()
        .map(|e| colorize(e, &p.structural, color))
        .unwrap_or_default();
    format!("{prefix}{name_str}{model_part}{elapsed_str}")
}

fn format_todo_chip(todo: &TodoSummary, p: &ThemePalette, color: bool) -> String {
    if todo.all_done {
        let check = colorize("\u{2713}", &p.completed_check, color);
        let txt = colorize(
            &format!(" {}/{}", todo.completed, todo.total),
            &p.completed_check,
            color,
        );
        return format!("{check}{txt}");
    }
    let bullet = colorize("\u{2022}", p.todo_teal(), color);
    let txt = colorize(
        &format!(
            " {}/{} todos",
            todo.completed.max(todo.total - todo.pending),
            todo.total
        ),
        p.todo_teal(),
        color,
    );
    format!("{bullet}{txt}")
}

/// Compact list of completed tool names — used by Console for the wider row.
pub fn completed_tool_chips(
    completed: &[CompletedToolCount],
    max: usize,
    p: &ThemePalette,
    color: bool,
) -> String {
    let chips: Vec<String> = completed
        .iter()
        .take(max)
        .map(|c| {
            let check = colorize("\u{2713}", &p.completed_check, color);
            let name = colorize(&format!(" {}", c.name), &p.completed_check, color);
            let count = colorize(&format!(" ×{}", c.count), &p.secondary, color);
            format!("{check}{name}{count}")
        })
        .collect();
    chips.join("  ")
}

/// Render the recent-tools tape with up to `max_items`.
pub fn tools_tape(
    tools: &[ToolSummary],
    max_items: usize,
    p: &ThemePalette,
    color: bool,
) -> String {
    widgets::tape::render(tools, max_items, p, color)
}

/// Bare cost text — `$X.XX` colored with `cost_base`. Shared between Cockpit
/// (when too narrow for the arc) and Flightstrip's L1.
pub fn cost_text_only(line3: &Line3Metrics, p: &ThemePalette, color_enabled: bool) -> String {
    let total = line3.total_cost_usd.unwrap_or(0.0);
    colorize(&format!("${total:.2}"), &p.cost_base, color_enabled)
}

/// Single-row fallback used by Cockpit (<80 cols) and Flightstrip (<70 cols).
/// Identity + CTX% + cost — the irreducible minimum.
///
/// Width-aware: when the assembled row exceeds `config.terminal_width`,
/// progressively trim head segments (project path → git stats → version /
/// style) until it fits. Model + branch + CTX% + cost are always preserved.
pub fn degraded_single_row(frame: &RenderFrame, config: &RenderConfig, p: &ThemePalette) -> String {
    use crate::render::color::visible_width;

    let color = config.color_enabled;
    let pct = frame.line3.context_used_percentage.unwrap_or(0);
    let pct_color = p.color_for_ctx_pct(pct, frame.line3.context_window_size);
    let pct_str = colorize(&format!("{pct}%"), pct_color, color);
    let cost = cost_text_only(&frame.line3, p, color);
    let tail_w = visible_width(&pct_str) + visible_width(&cost) + 4; // two "  " seps
    let max_head = config
        .terminal_width
        .map(|w| w.saturating_sub(tail_w))
        .unwrap_or(usize::MAX);

    // Drop order: trim from least-essential to most. We never drop model + branch.
    const TRIMMERS: &[fn(&mut RenderConfig)] = &[
        |c| c.show_project = false,
        |c| c.show_git_stats = false,
        |c| c.show_version = false,
        |c| c.show_style = false,
        |c| c.show_agent = false,
        |c| c.show_worktree = false,
    ];

    let mut head = identity_headline(&frame.line1, config, p);
    if visible_width(&head) <= max_head {
        return format!("{head}  {pct_str}  {cost}");
    }

    let mut shrunk = config.clone();
    for trim in TRIMMERS {
        trim(&mut shrunk);
        head = identity_headline(&frame.line1, &shrunk, p);
        if visible_width(&head) <= max_head {
            return format!("{head}  {pct_str}  {cost}");
        }
    }
    // Even bare model+branch overflows — let CC clip rather than emit nothing.
    format!("{head}  {pct_str}  {cost}")
}
