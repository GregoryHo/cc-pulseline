//! Shared building blocks for layout frames.
//!
//! Two non-overlapping concerns live here:
//! 1. **Box-drawing glyphs + label/content padding** — used by every framed
//!    layout (`zones`, `grid`, `sections`, `console`).
//! 2. **CTX dispatch hub + per-cell builders** — `render_context_visual`
//!    composes a CTX cell from the user's `*_visual` spec by routing to
//!    the relevant cell builder (`ctx_text_cell`, `ctx_gauge_cell`,
//!    `ctx_sparkline`).
//!
//! Each helper returns a string fragment (already colorized when the config
//! enables it) so layouts compose them with their own separators / framing.

use std::ops::Range;

use crate::config::{GlyphMode, RenderConfig};
use crate::render::color::{colorize, visible_width, ThemePalette};
use crate::render::fmt::format_number;
use crate::render::icons::{glyph, ICON_EFFORT, ICON_THINKING};
use crate::render::layout;
use crate::render::pane::{LineKind, PaneConfig, PaneGroup};
use crate::render::widgets;
use crate::types::{Line1Metrics, Line3Metrics, RenderFrame};

// ============================================================================
// Box-drawing glyphs and frame helpers
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
// Identity headline + config-row helpers (shared by ledger / console paths)
// ============================================================================

/// Whether at least one config-row segment is enabled. Used by ledger to
/// decide whether to emit the ENV row at all.
pub fn config_row_enabled(config: &RenderConfig) -> bool {
    config.show_claude_md
        || config.show_rules
        || config.show_memory
        || config.show_hooks
        || config.show_mcp
        || config.show_skills
        || config.show_plugins
        || config.show_duration
}

/// Prefix-less identity headline used by Console (in the top frame title)
/// and Ledger (in the top frame title).
///
/// `separator` controls the visual rhythm — Console / Ledger pass `" · "`
/// (middle-dot) for a "title format" feel. Each segment honours its
/// `show_*` toggle.
pub fn identity_headline(
    line1: &Line1Metrics,
    config: &RenderConfig,
    p: &ThemePalette,
    separator: &str,
) -> String {
    let color = config.color_enabled;
    let coloured_sep = colorize(separator, &p.structural, color);
    let mut parts: Vec<String> = Vec::new();

    if config.show_model {
        parts.push(colorize(&line1.model, &p.primary, color));
    }

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

    if config.show_project {
        parts.push(colorize(&line1.project_path, &p.secondary, color));
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

    parts.join(&coloured_sep)
}

/// Compact L2 config row, width-aware. Delegates to `format_line2` in
/// `layout.rs` so the icons, counts, and toggles stay in lockstep across
/// layouts. When the assembled row exceeds `max_width`, segments are
/// progressively turned off in low-value-first order until the row fits.
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

    let body = layout::format_line2(frame, config, "  ", p);
    if body.is_empty() {
        return String::new();
    }
    if visible_width(&body) <= max_width {
        return body;
    }

    let mut shrunk = config.clone();
    let mut last_body = String::new();
    for drop in DROP_ORDER {
        drop(&mut shrunk);
        last_body = layout::format_line2(frame, &shrunk, "  ", p);
        if last_body.is_empty() {
            return String::new();
        }
        if visible_width(&last_body) <= max_width {
            return last_body;
        }
    }
    last_body
}

// ============================================================================
// CTX dispatch hub + per-cell builders
// ============================================================================

/// Compose a CTX cell from a `+`-separated visual spec.
///
/// Recognized widget names: `gauge`, `sparkline`, `text`. Unknown names are
/// silently dropped (forward-compat: a future widget added to the registry
/// can be referenced from a config that older binaries simply ignore).
///
/// Empty input returns an empty string. Multiple widgets are joined with one
/// space.
///
/// Single dispatch hub for context composability: any layout that calls
/// it instead of `ctx_gauge_cell`/`ctx_sparkline` directly inherits the
/// per-segment override capability for free.
pub fn render_context_visual(
    spec: &str,
    line3: &Line3Metrics,
    history: &[(u8, u64)],
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
            WIDGET_GAUGE => ctx_gauge_cell(line3, gauge_width, mode, p, color_enabled),
            WIDGET_SPARKLINE => ctx_sparkline(history, mode, p, color_enabled),
            WIDGET_TEXT => ctx_text_cell(line3, mode, p, color_enabled),
            _ => String::new(), // unknown widget — silently drop
        };
        if !cell.is_empty() {
            parts.push(cell);
        }
    }
    parts.join(" ")
}

/// CTX text cell — `<icon> 8% 80.0k/1.0M`. Icon + percentage + concrete
/// `used/total` numbers. Used color matches the threshold; total stays
/// in `secondary` so the eye reads the active number first.
pub fn ctx_text_cell(
    line3: &Line3Metrics,
    mode: GlyphMode,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    use crate::render::icons::{glyph, ICON_CONTEXT};
    let icon_glyph = glyph(mode, ICON_CONTEXT, "CTX");
    match (line3.context_used_percentage, line3.context_window_size) {
        (Some(pct), Some(size)) => {
            let pct_color = p.color_for_ctx_pct(pct);
            let icon = colorize(&icon_glyph, pct_color, color_enabled);
            let pct_str = colorize(&format!(" {pct}%"), pct_color, color_enabled);
            let used = ((size as f64) * (pct as f64) / 100.0) as u64;
            let used_str = colorize(
                &format!(" {}", format_number(used)),
                &p.primary,
                color_enabled,
            );
            let slash = colorize("/", &p.separator, color_enabled);
            let total_str = colorize(&format_number(size), &p.secondary, color_enabled);
            format!("{icon}{pct_str}{used_str}{slash}{total_str}")
        }
        _ => {
            let icon = colorize(&icon_glyph, &p.structural, color_enabled);
            let dash = colorize(" --% --/--", &p.structural, color_enabled);
            format!("{icon}{dash}")
        }
    }
}

/// CTX gauge cell — `<icon> [gauge] 80.0k/1.0M`. Same shape as
/// `ctx_text_cell`, with the gauge bar replacing the `%` text.
///
/// No-data state delegates to `ctx_text_cell` so an empty `[          ]`
/// never becomes the visually heaviest cell on the row.
pub fn ctx_gauge_cell(
    line3: &Line3Metrics,
    gauge_width: usize,
    mode: GlyphMode,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    use crate::render::icons::{glyph, ICON_CONTEXT};
    match (line3.context_used_percentage, line3.context_window_size) {
        (Some(pct), Some(size)) => {
            let icon_glyph = glyph(mode, ICON_CONTEXT, "CTX");
            let fill_color = p.color_for_ctx_pct(pct);
            let icon = colorize(&icon_glyph, fill_color, color_enabled);
            let marks = ThemePalette::ctx_marks();
            let bar = widgets::gauge::render(
                pct,
                gauge_width,
                &marks,
                fill_color,
                p,
                mode,
                color_enabled,
            );
            let used = ((size as f64) * (pct as f64) / 100.0) as u64;
            let used_str = colorize(
                &format!(" {}", format_number(used)),
                &p.primary,
                color_enabled,
            );
            let slash = colorize("/", &p.separator, color_enabled);
            let total_str = colorize(&format_number(size), &p.secondary, color_enabled);
            format!("{icon} {bar}{used_str}{slash}{total_str}")
        }
        _ => ctx_text_cell(line3, mode, p, color_enabled),
    }
}

// ============================================================================
// Quota dispatch hub
// ============================================================================

/// Quota gauge bar width in cells. 14 cells gives clean threshold-mark
/// math (50% lands at cell 7; 85% at cell 12 — both round-half-up
/// from `(threshold * width / 100)`).
pub const QUOTA_BAR_WIDTH: usize = 14;

/// CTX gauge bar width in cells. Shared across every layout that opts
/// into `context_visual = "gauge"` so the bar looks the same regardless
/// of layout chrome. Wider than quota's 14 — CTX has more numeric
/// information competing for space, and the wider bar absorbs that
/// without clipping.
pub const CTX_BAR_WIDTH: usize = 18;

/// Quota threshold marks — fixed `[50, 85]` for the three-bucket
/// good/warn/critical ladder applied by `color_for_quota_pct`.
pub const QUOTA_MARKS: [u64; 2] = [50, 85];

// ── Visual spec keywords (string atoms in `*_visual` config) ──
pub const WIDGET_GAUGE: &str = "gauge";
pub const WIDGET_SPARKLINE: &str = "sparkline";
pub const WIDGET_TEXT: &str = "text";

/// Returns true if the `+`-joined visual spec contains `widget` as a
/// trimmed atom. Centralizes the parse so a typo in one widget name
/// doesn't silently drop the widget.
pub fn spec_has(spec: &str, widget: &str) -> bool {
    spec.split('+').any(|atom| atom.trim() == widget)
}

/// Render the quota gauge bar when `spec` contains `gauge`. Returns
/// an empty string otherwise (caller's text path handles the
/// `5h: 62% (resets ...)` rendering directly).
pub fn render_quota_visual(
    spec: &str,
    pct: f64,
    p: &ThemePalette,
    mode: GlyphMode,
    color_enabled: bool,
) -> String {
    if !spec_has(spec, WIDGET_GAUGE) {
        return String::new();
    }
    let fill = p.color_for_quota_pct(pct);
    widgets::gauge::render(
        pct as u64,
        QUOTA_BAR_WIDTH,
        &QUOTA_MARKS,
        fill,
        p,
        mode,
        color_enabled,
    )
}

/// CTX sparkline glyph strip (no label) — empty when history is empty *or*
/// when `mode == GlyphMode::Ascii` (sparkline is icon-only).
///
/// Picks the aurora fill color from the last sample (the simpler rule for
/// hub-dispatch callers — flat layouts that opt into `+sparkline`). The
/// ledger layout uses `widgets::sparkline::aurora_for_velocity` instead,
/// which carries the velocity signal independent of the absolute value.
pub fn ctx_sparkline(
    history: &[(u8, u64)],
    mode: GlyphMode,
    p: &ThemePalette,
    color_enabled: bool,
) -> String {
    let last = history.last().map(|(pct, _)| *pct).unwrap_or(0);
    let fill = if last >= 67 {
        &p.aurora_high
    } else if last >= 34 {
        &p.aurora_mid
    } else {
        &p.aurora_low
    };
    widgets::sparkline::render(history, fill, mode, color_enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_all_off() -> RenderConfig {
        RenderConfig {
            show_claude_md: false,
            show_rules: false,
            show_memory: false,
            show_hooks: false,
            show_mcp: false,
            show_skills: false,
            show_plugins: false,
            show_duration: false,
            ..RenderConfig::default()
        }
    }

    #[test]
    fn config_row_enabled_respects_show_duration_alone() {
        // All env counters off, only duration on — the row must still render
        // because duration IS part of the L2 config row.
        let mut cfg = cfg_all_off();
        cfg.show_duration = true;
        assert!(config_row_enabled(&cfg));
    }

    #[test]
    fn config_row_enabled_false_when_everything_off() {
        // Sanity guard for the predicate's negative case.
        assert!(!config_row_enabled(&cfg_all_off()));
    }
}
