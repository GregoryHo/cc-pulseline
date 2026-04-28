//! Shared building blocks for layout frames.
//!
//! Two non-overlapping concerns live here:
//! 1. **Box-drawing glyphs + label/content padding** — used by the flat-row
//!    decorating layouts (`zones`, `grid`, `cards`, `sections`).
//! 2. **Widget call helpers + cluster row builders** — used by the
//!    instrument-cluster layouts (`cockpit`, `console`, `flightstrip`,
//!    `auto`).
//!
//! Each helper returns a string fragment (already colorized when the config
//! enables it) so layouts compose them with their own separators / framing.

use std::ops::Range;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{GlyphMode, RenderConfig};
use crate::render::color::{colorize, visible_width, ThemePalette};
use crate::render::fmt::{
    format_agent_elapsed, format_number, format_reset_duration, format_speed,
};
use crate::render::icons::{glyph, ICON_AGENT, ICON_EFFORT, ICON_THINKING};
use crate::render::layout;
use crate::render::pane::{LineKind, PaneConfig, PaneGroup};
use crate::render::widgets;
use crate::types::{
    AgentSummary, CompletedToolCount, Line1Metrics, Line3Metrics, RenderFrame, TodoSummary,
    ToolSummary,
};

// ============================================================================
// Box-drawing glyphs and frame helpers (flat-row layouts)
// ============================================================================

pub struct FrameGlyphs {
    pub h: &'static str,
    pub v: &'static str,
    pub tl: &'static str,
    pub tr: &'static str,
    pub bl: &'static str,
    pub br: &'static str,
    pub tee_t: &'static str,
    pub tee_b: &'static str,
    pub tee_l: &'static str,
    pub tee_r: &'static str,
    pub cross: &'static str,
}

const UNICODE_GLYPHS: FrameGlyphs = FrameGlyphs {
    h: "─",
    v: "│",
    tl: "╭",
    tr: "╮",
    bl: "╰",
    br: "╯",
    tee_t: "┬",
    tee_b: "┴",
    tee_l: "├",
    tee_r: "┤",
    cross: "┼",
};

const ASCII_GLYPHS: FrameGlyphs = FrameGlyphs {
    h: "-",
    v: "|",
    tl: "+",
    tr: "+",
    bl: "+",
    br: "+",
    tee_t: "+",
    tee_b: "+",
    tee_l: "+",
    tee_r: "+",
    cross: "+",
};

pub fn glyphs(mode: GlyphMode) -> &'static FrameGlyphs {
    match mode {
        GlyphMode::Icon => &UNICODE_GLYPHS,
        GlyphMode::Ascii => &ASCII_GLYPHS,
    }
}

pub fn max_label_width(groups: &[PaneGroup]) -> usize {
    groups
        .iter()
        .map(|pg| visible_width(&pg.label))
        .max()
        .unwrap_or(0)
}

pub fn max_content_width(lines: &[String]) -> usize {
    lines.iter().map(|l| visible_width(l)).max().unwrap_or(0)
}

pub fn label_for_kind(cfg: &PaneConfig, kind: LineKind) -> &str {
    cfg.groups
        .iter()
        .find(|pg| pg.kinds.contains(&kind))
        .map(|pg| pg.label.as_str())
        .unwrap_or("")
}

/// Right-pad `s` with spaces to reach `width` visible cells.
/// Uses `visible_width` (which strips ANSI) so styled strings pad correctly —
/// `format!("{:<width$}")` would over-pad by counting escape bytes.
pub fn pad_to(s: &str, width: usize) -> String {
    let pad = width.saturating_sub(visible_width(s));
    let mut out = String::with_capacity(s.len() + pad);
    out.push_str(s);
    for _ in 0..pad {
        out.push(' ');
    }
    out
}

pub struct FrameBorders {
    pub top: String,
    pub mid: String,
    pub bot: String,
}

/// Top / mid / bot share the same dash widths so `┬`/`┼`/`┴` joints align
/// vertically across all three.
pub fn frame_borders(max_label: usize, content_width: usize, g: &FrameGlyphs) -> FrameBorders {
    let dash_l = g.h.repeat(max_label + 2);
    let dash_r = g.h.repeat(content_width + 2);
    FrameBorders {
        top: format!(
            "{tl}{dash_l}{tee_t}{dash_r}{tr}",
            tl = g.tl,
            tr = g.tr,
            tee_t = g.tee_t,
        ),
        mid: format!(
            "{tee_l}{dash_l}{cross}{dash_r}{tee_r}",
            tee_l = g.tee_l,
            tee_r = g.tee_r,
            cross = g.cross,
        ),
        bot: format!(
            "{bl}{dash_l}{tee_b}{dash_r}{br}",
            bl = g.bl,
            br = g.br,
            tee_b = g.tee_b,
        ),
    }
}

/// First row of the group shows the label; continuation rows blank it so the
/// `│` divider column lines up vertically.
#[allow(clippy::too_many_arguments)]
pub fn push_walled_group_rows(
    out: &mut Vec<String>,
    lines: &[String],
    kind: LineKind,
    range: Range<usize>,
    cfg: &PaneConfig,
    g: &FrameGlyphs,
    max_label: usize,
    content_width: usize,
) {
    let label = label_for_kind(cfg, kind);
    let mut first = true;
    for idx in range.start..range.end {
        let Some(line) = lines.get(idx) else { continue };
        let lbl = if first { label } else { "" };
        let lbl_field = pad_to(lbl, max_label);
        let line_field = pad_to(line, content_width);
        out.push(format!("{v} {lbl_field} {v} {line_field} {v}", v = g.v));
        first = false;
    }
}

// ============================================================================
// Widget call helpers + cluster row builders (instrument-cluster layouts)
// ============================================================================

/// CTX sparkline (braille mini-chart) is opt-in and Nerd Font only — there
/// is no ASCII fallback that conveys the same trend information, so we hide
/// it cleanly under `icons = false` rather than emitting boxes.
///
/// Reads the resolved `context_visual` spec: any `+`-separated component
/// equal to `"sparkline"` opts in. Compare to the legacy
/// `show_ctx_sparkline = true` (now removed) which mapped to
/// `context_visual = "gauge+sparkline"`.
pub fn sparkline_enabled(config: &RenderConfig) -> bool {
    config.glyph_mode.is_icon()
        && config
            .effective_context_visual()
            .split('+')
            .any(|w| w.trim() == "sparkline")
}

/// Whether at least one config-row segment is enabled.
///
/// In instrument-cluster layouts the L2 row is opt-in: it stays hidden unless
/// the user has flipped any `show_*` for the config segments. Flat layouts
/// still render L2 always-on (handled by the legacy code path).
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

    // Effort + thinking pills sit between model and agent — same order as
    // v1 `format_line1` so the eye lands on them in the same position
    // regardless of which layout is active.
    if config.show_effort {
        if let Some(level) = &line1.effort_level {
            let effort_color = p.color_for_effort_level(level);
            let label = colorize(
                &glyph(config.glyph_mode, ICON_EFFORT, "E:"),
                effort_color,
                color,
            );
            let val = colorize(level, effort_color, color);
            parts.push(format!("{label}{val}"));
        }
    }

    if config.show_thinking && line1.thinking_enabled == Some(true) {
        // Label-only pill — no value; absent / `enabled: false` → omitted.
        let raw = glyph(config.glyph_mode, ICON_THINKING, "[T]");
        parts.push(colorize(raw.trim_end(), &p.active_purple, color));
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

/// Compact L2 config row for instrument-cluster layouts (opt-in). Delegates to
/// `format_line2` in `layout.rs` so the icons, counts, and toggles stay in
/// lockstep across layouts; this only adds a leading `CFG  ` label and uses
/// two spaces between segments.
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

/// Compose a CTX cell from a `+`-separated visual spec.
///
/// Recognized widget names: `gauge`, `sparkline`, `text`. Unknown names are
/// silently dropped (forward-compat: a future widget added to the registry
/// can be referenced from a config that older binaries simply ignore).
///
/// Empty input returns an empty string. Multiple widgets are joined with one
/// space — the cluster row's existing two-space cell separator stays the
/// boundary between independent cells.
///
/// This is the dispatch hub for Phase 3 composability: any layout that calls
/// it instead of `ctx_gauge_cell`/`ctx_sparkline` directly inherits the
/// per-segment override capability for free.
pub fn render_context_visual(
    spec: &str,
    line3: &Line3Metrics,
    history: &[u8],
    gauge_width: usize,
    mode: GlyphMode,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    for raw in spec.split('+') {
        let widget = raw.trim();
        if widget.is_empty() {
            continue;
        }
        let cell = match widget {
            "gauge" => ctx_gauge_cell(line3, gauge_width, mode, p, color_enabled),
            "sparkline" => {
                // Sparkline is icon-only; widget itself returns "" under
                // Ascii so we don't need to gate here.
                ctx_sparkline(history, mode, p, color_enabled)
            }
            "text" => ctx_text_cell(line3, mode, p, color_enabled),
            _ => String::new(), // unknown widget — silently drop
        };
        if !cell.is_empty() {
            parts.push(cell);
        }
    }
    parts.join(" ")
}

/// CTX text cell — `<glyph>43% (86.0k/200.0k)` form (or `CTX:43%...` under
/// Ascii). No gauge or sparkline. Used when `context_visual` includes
/// `"text"`. Format unified with the legacy v1 `format_context_segment`
/// so flat layouts route through `render_context_visual` without changing
/// their rendered output.
pub fn ctx_text_cell(
    line3: &Line3Metrics,
    mode: GlyphMode,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    use crate::render::icons::{glyph, ICON_CONTEXT};
    match (line3.context_used_percentage, line3.context_window_size) {
        (Some(pct), Some(size)) => {
            let pct_color = p.color_for_ctx_pct(pct, Some(size));
            let label = colorize(&glyph(mode, ICON_CONTEXT, "CTX:"), pct_color, color_enabled);
            let pct_str = colorize(&format!("{pct}%"), pct_color, color_enabled);
            let used = ((size as f64) * (pct as f64) / 100.0) as u64;
            let open = colorize(" (", &p.separator, color_enabled);
            let used_str = colorize(&format_number(used), &p.primary, color_enabled);
            let sep = colorize("/", &p.separator, color_enabled);
            let total = colorize(&format_number(size), &p.primary, color_enabled);
            let close = colorize(")", &p.separator, color_enabled);
            format!("{label}{pct_str}{open}{used_str}{sep}{total}{close}")
        }
        _ => {
            // No data yet: render the same `CTX:--% (--/--)` placeholder the
            // v1 path emits, so dispatch parity holds even before the first
            // context-aware response.
            let label = colorize(
                &glyph(mode, ICON_CONTEXT, "CTX:"),
                &p.structural,
                color_enabled,
            );
            let dash = colorize("--", &p.structural, color_enabled);
            let pct_sign = colorize("%", &p.structural, color_enabled);
            let open = colorize(" (", &p.separator, color_enabled);
            let sep = colorize("/", &p.separator, color_enabled);
            let close = colorize(")", &p.separator, color_enabled);
            format!("{label}{dash}{pct_sign}{open}{dash}{sep}{dash}{close}")
        }
    }
}

/// CTX gauge cell — `gauge_width` cells, followed by `% used` text.
pub fn ctx_gauge_cell(
    line3: &Line3Metrics,
    gauge_width: usize,
    mode: GlyphMode,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    let pct = line3.context_used_percentage.unwrap_or(0);
    let label = colorize("CTX  ", &p.structural, color_enabled);
    let bar = widgets::gauge::render(pct, gauge_width, mode, p, color_enabled);
    let pct_color = p.color_for_ctx_pct(pct, line3.context_window_size);
    let pct_str = colorize(&format!(" {pct}%"), pct_color, color_enabled);
    format!("{label}{bar}{pct_str}")
}

/// CTX sparkline glyph strip (no label) — empty when history is empty *or*
/// when `mode == GlyphMode::Ascii` (sparkline is icon-only).
pub fn ctx_sparkline(
    history: &[u8],
    mode: GlyphMode,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    widgets::sparkline::render(history, mode, p, color_enabled)
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
///
/// Kept as a thin wrapper over `render_cost_visual` so callers that don't
/// expose `cost_visual` to the user (legacy code paths) get the cockpit
/// default behaviour `"text+arc"`.
pub fn cost_cell(line3: &Line3Metrics, config: &RenderConfig, p: &ThemePalette) -> String {
    render_cost_visual(
        "text+arc",
        line3,
        config.glyph_mode,
        p,
        config.color_enabled,
    )
}

/// Compose a cost cell from a `+`-separated visual spec.
///
/// Recognized widget names: `text`, `arc`. Both is the cockpit/console
/// default `"text+arc"`; either alone produces a stripped variant for
/// minimal layouts.
///
/// **Compound behaviour for `text+arc`**: the `arc` widget is icon-only
/// (returns empty under `GlyphMode::Ascii`), so under Ascii mode the `text`
/// widget picks up the `($X.X/h)` rate annotation that arc would normally
/// carry pictographically. Under Icon mode `text` is bare `$X.XX` and `arc`
/// supplies the burn glyph. Net: `text+arc` always conveys both total and
/// burn rate, in whichever form the terminal supports.
///
/// Lone `text` always shows the rate when available (no arc to compensate).
/// Lone `arc` shows only the glyph (returns empty in Ascii — caller should
/// drop the empty cell).
pub fn render_cost_visual(
    spec: &str,
    line3: &Line3Metrics,
    mode: GlyphMode,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    let total = line3.total_cost_usd.unwrap_or(0.0);
    let per_hour = line3
        .total_duration_ms
        .filter(|d| *d > 0)
        .map(|d| total / ((d as f64) / 3_600_000.0))
        .unwrap_or(0.0);

    let widgets: Vec<&str> = spec
        .split('+')
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .collect();
    let want_text = widgets.contains(&"text");
    let want_arc = widgets.contains(&"arc");

    // When arc is requested but will silently return "" (Ascii mode), text
    // picks up the rate annotation as compensation. This preserves the
    // cockpit's "always shows burn rate, in icon or text form" promise.
    let arc_cell = if want_arc {
        widgets::arc::render(per_hour, mode, p, color_enabled)
    } else {
        String::new()
    };
    let arc_will_render = !arc_cell.is_empty();

    let mut parts: Vec<String> = Vec::new();
    if want_text {
        let total_str = colorize(&format!("${total:.2}"), &p.cost_base, color_enabled);
        let with_rate = !want_arc || !arc_will_render;
        if with_rate && per_hour > 0.0 {
            let rate = colorize(&format!("(${per_hour:.1}/h)"), &p.structural, color_enabled);
            parts.push(format!("{total_str} {rate}"));
        } else {
            parts.push(total_str);
        }
    }
    if !arc_cell.is_empty() {
        parts.push(arc_cell);
    }
    parts.join(" ")
}

/// Quota gauge bar width when rendered as `bar`. Tuned to match the console's
/// existing `QUOTA_GAUGE_WIDTH` so visual specs stay consistent across
/// layouts that opt into bar form.
pub const QUOTA_BAR_WIDTH: usize = 12;

/// Compose a quota cell from a visual spec.
///
/// Recognized widget names: `text`, `bar`.
/// - `text` (or empty spec): `5h 75% (2h 0m)` — label + pct + reset.
/// - `bar`: `5h ████████░░░░ 75% (2h 0m)` — adds a gauge before the pct.
/// - `text+bar` / `bar+text`: same as `bar`.
///
/// **pct text and reset annotation are unconditional** when their data is
/// available — they're the irreducible "how full / how long until reset"
/// information. The `bar` widget is purely additional visualization on top.
/// This avoids the trap where opting into `bar` would drop the precise pct
/// number, leaving the user squinting at the gauge to estimate fullness.
///
/// `label` is the leading text (`"5h "` or `"7d "`); the cluster row's cell
/// position provides quota context, so the `Q` prefix is dropped (the v1
/// flat layout uses a single `Q:` group prefix instead of repeating it per
/// window).
///
/// Returns empty string when `pct` is `None` (no data — entire cell drops).
pub fn render_quota_visual(
    spec: &str,
    label: &str,
    pct: Option<f64>,
    reset_min: Option<u64>,
    mode: GlyphMode,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    let pct = match pct {
        Some(v) => v,
        None => return String::new(),
    };
    let want_bar = spec.split('+').map(|w| w.trim()).any(|w| w == "bar");

    let mut body = colorize(label, &p.structural, color_enabled);
    if want_bar {
        let bar = widgets::gauge::render(pct as u64, QUOTA_BAR_WIDTH, mode, p, color_enabled);
        body.push_str(&bar);
        body.push_str(&colorize("  ", &p.structural, color_enabled));
    }
    let pct_str = colorize(
        &format!("{pct:.0}%"),
        p.color_for_quota_pct(pct),
        color_enabled,
    );
    body.push_str(&pct_str);
    if let Some(m) = reset_min {
        let reset = colorize(
            &format!(" (resets {})", format_reset_duration(m)),
            &p.structural,
            color_enabled,
        );
        body.push_str(&reset);
    }
    body
}

/// Activity ticker: tools tape + completed counts + agents + todo summary —
/// joined by three spaces. Empty cells are skipped.
///
/// `width` is the row's pane budget (cockpit/flightstrip clamp this to
/// `pane_max_width`); the tools tape uses it as its row budget so long
/// targets compress instead of overflowing the pane.
pub fn activity_ticker(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    width: usize,
) -> String {
    let color = config.color_enabled;
    let mut parts: Vec<String> = Vec::new();

    if config.show_tools {
        if !frame.tools.is_empty() {
            let tape = render_tools_visual_inline(
                config.effective_tools_visual(),
                &frame.tools,
                config.max_tool_lines.max(1),
                width,
                config.glyph_mode,
                p,
                color,
            );
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

/// Compose an inline (single-line) tools cell from a visual spec.
///
/// Recognized atoms (`+`-joined):
/// - `tape` — required to render anything; without it the cell is empty.
/// - `detail` — opt-in modifier on `tape`; flips per-tool format from
///   brief (`▶ Read · ▶ Bash`) to detailed
///   (`<icon> Read: src/main.rs · <icon> Bash: cargo test`).
/// - `list` — silently dropped here. `list` is multi-row and rendered by
///   the activity row builder via `show_tools` in flat-row layouts; in
///   inline cluster contexts it has no sensible single-line shape.
///
/// Unknown atoms drop silently. Empty `tools` or `max_width == 0`
/// produces an empty string.
///
/// `max_width` is the row's width budget. The hub asks
/// `widgets::tape::render` for cells, then feeds them through
/// `pack_with_separator` so detail cells compress their target under
/// width pressure (the budgeter respects each cell's `min_width = 8`)
/// and brief cells drop rightmost (newest) under extreme pressure
/// rather than overflowing the pane and forcing CC to wrap the
/// statusline into multiple rows.
pub fn render_tools_visual_inline(
    spec: &str,
    tools: &[ToolSummary],
    max_items: usize,
    max_width: usize,
    mode: GlyphMode,
    p: &ThemePalette,
    color: bool,
) -> String {
    if tools.is_empty() || max_items == 0 || max_width == 0 {
        return String::new();
    }
    let atoms: Vec<&str> = spec
        .split('+')
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .collect();
    let want_tape = atoms.contains(&"tape");
    let with_target = atoms.contains(&"detail");
    if !want_tape {
        return String::new();
    }
    let cells = widgets::tape::render(tools, max_items, mode, p, color, with_target);
    if cells.is_empty() {
        return String::new();
    }
    let sep = colorize(widgets::tape::SEPARATOR, &p.separator, color);
    crate::render::activity::budget::pack_with_separator(
        &cells,
        max_width,
        &sep,
        widgets::tape::SEPARATOR_W,
        color,
    )
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
