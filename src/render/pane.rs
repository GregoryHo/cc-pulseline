use std::ops::Range;

use crate::config::GlyphMode;

use super::color::visible_width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneStyle {
    None,
    /// Two strata separated by a single labelled rule (echoes CC's own
    /// horizontal rules above/below the input box). State (Identity/Config/
    /// Budget) above, `──── activity ────` rule, then live Activity below.
    Zones,
    /// Table layout with a fixed label column + `│` divider + right-padded
    /// content. Every line begins and ends at the same visual position —
    /// solves jagged right edges and makes group boundaries explicit without
    /// adding rows. Activity continuation rows span the label column.
    Grid,
    /// One independent `╭─┬─╮ / ╰─┴─╯` frame per group, stacked vertically.
    /// Each group (Identity / Config / Budget / Activity) becomes its own
    /// self-contained card. All cards share a global `max_label_width` and
    /// `max_content_width` so they line up when stacked. Adds 2 rows per
    /// non-empty group (top + bottom of each card).
    Cards,
    /// Single outer `╭─┬─╮ / ╰─┴─╯` wrapper with a `├─┼─┤` separator
    /// emitted between every pair of non-empty groups. Reads as one
    /// container with explicit internal dividers — cheaper than Cards
    /// (no double-border gaps).
    Sections,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneWidth {
    Auto,
    Terminal,
    Fixed(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineKind {
    Identity,
    Config,
    Budget,
    Activity,
}

#[derive(Debug, Clone)]
pub struct PaneGroup {
    pub label: String,
    pub kinds: Vec<LineKind>,
}

/// Cols subtracted from `terminal_width` in `PaneWidth::Terminal` mode.
/// Claude Code allocates the statusline a sub-region that is ~1-4 cols
/// narrower than the raw terminal; lines at exactly the raw width trigger
/// wrap and collapse multi-line rendering to a single visible line. 4 is
/// the empirically verified safe default on CC 2.1.119.
pub const DEFAULT_PANE_CC_MARGIN: usize = 4;

#[derive(Debug, Clone)]
pub struct PaneConfig {
    pub style: PaneStyle,
    pub width_mode: PaneWidth,
    pub min_width: usize,
    pub max_width: usize,
    pub groups: Vec<PaneGroup>,
    pub glyph_mode: GlyphMode,
    pub terminal_width: Option<usize>,
    pub cc_margin: usize,
}

/// Apply the configured pane style to `lines`.
///
/// Returns `lines` unchanged when `cfg.style == PaneStyle::None` or when the
/// terminal can't fit `cfg.min_width`.
pub fn apply_pane(
    lines: Vec<String>,
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
) -> Vec<String> {
    if matches!(cfg.style, PaneStyle::None) {
        return lines;
    }
    if lines.is_empty() || cfg.groups.is_empty() {
        return lines;
    }

    if let Some(term) = cfg.terminal_width {
        if term < cfg.min_width {
            return lines;
        }
    }

    let grouped = collect_grouped_lines(&lines, groups, &cfg.groups);
    if grouped.is_empty() {
        return lines;
    }

    let g = glyphs(cfg.glyph_mode);
    match cfg.style {
        PaneStyle::Zones => render_zones(&grouped, &lines, groups, cfg, g),
        PaneStyle::Grid => render_grid(&lines, groups, cfg, g),
        PaneStyle::Cards => render_cards(&lines, groups, cfg, g),
        PaneStyle::Sections => render_sections(&lines, groups, cfg, g),
        PaneStyle::None => lines,
    }
}

/// Sections: one outer `╭─┬─╮ / ╰─┴─╯` frame wrapping every group, with a
/// `├─┼─┤` separator between every pair of non-empty groups.
/// Overhead: 2 + (N-1) rows for N non-empty groups.
fn render_sections(
    lines: &[String],
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
) -> Vec<String> {
    let max_label = max_label_width(&cfg.groups);
    let content_width = max_content_width(lines);
    let borders = frame_borders(max_label, content_width, g);

    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 2 + groups.len());
    out.push(borders.top);

    let mut emitted_first_group = false;
    for (kind, range) in groups {
        if range.start >= range.end {
            continue;
        }
        if emitted_first_group {
            out.push(borders.mid.clone());
        }
        push_walled_group_rows(
            &mut out,
            lines,
            *kind,
            range.clone(),
            cfg,
            g,
            max_label,
            content_width,
        );
        emitted_first_group = true;
    }

    out.push(borders.bot);
    out
}

/// Cards: one independent `╭─┬─╮ / ╰─┴─╯` frame per non-empty group,
/// stacked vertically. All cards share the same global label/content widths
/// so the internal divider and outer walls align column-for-column.
/// Overhead: 2 rows per non-empty group.
fn render_cards(
    lines: &[String],
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
) -> Vec<String> {
    let max_label = max_label_width(&cfg.groups);
    let content_width = max_content_width(lines);
    let borders = frame_borders(max_label, content_width, g);

    let mut out: Vec<String> = Vec::with_capacity(lines.len() + groups.len() * 2);
    for (kind, range) in groups {
        if range.start >= range.end {
            continue;
        }
        out.push(borders.top.clone());
        push_walled_group_rows(
            &mut out,
            lines,
            *kind,
            range.clone(),
            cfg,
            g,
            max_label,
            content_width,
        );
        out.push(borders.bot.clone());
    }
    out
}

/// Grid: fixed-width left label column + `│` + content right-padded to the
/// longest natural row. Activity continuation rows show blank label (span).
fn render_grid(
    lines: &[String],
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
) -> Vec<String> {
    // Grid pads the label column to `max_label + 2` cells (the extra 2 cols
    // form the visual gap before the `│` divider). Cards/Sections don't need
    // this since their `│ ` wall already provides gap spacing.
    let label_width = max_label_width(&cfg.groups) + 2;
    let content_width = max_content_width(lines);

    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (kind, range) in groups {
        let label = label_for_kind(cfg, *kind);
        let mut first = true;
        for idx in range.start..range.end {
            let Some(line) = lines.get(idx) else { continue };
            let lbl = if first { label } else { "" };
            let lbl_field = pad_to(lbl, label_width);
            let line_field = pad_to(line, content_width);
            out.push(format!("{lbl_field}{v} {line_field}", v = g.v));
            first = false;
        }
    }
    out
}

// ── Shared helpers for framed pane styles ────────────────────────────

/// Longest configured group label (no trailing pad).
fn max_label_width(groups: &[PaneGroup]) -> usize {
    groups
        .iter()
        .map(|pg| visible_width(&pg.label))
        .max()
        .unwrap_or(0)
}

/// Longest visible line in `lines`. Rows shorter than this get right-padded.
fn max_content_width(lines: &[String]) -> usize {
    lines.iter().map(|l| visible_width(l)).max().unwrap_or(0)
}

/// Look up the display label for `kind` from the configured groups.
fn label_for_kind(cfg: &PaneConfig, kind: LineKind) -> &str {
    cfg.groups
        .iter()
        .find(|pg| pg.kinds.contains(&kind))
        .map(|pg| pg.label.as_str())
        .unwrap_or("")
}

/// Right-pad `s` with spaces to reach `width` visible cells.
fn pad_to(s: &str, width: usize) -> String {
    let pad = width.saturating_sub(visible_width(s));
    let mut out = String::with_capacity(s.len() + pad);
    out.push_str(s);
    for _ in 0..pad {
        out.push(' ');
    }
    out
}

struct FrameBorders {
    top: String,
    mid: String,
    bot: String,
}

/// Build the three horizontal border lines (top / mid / bot) for the
/// two-column framed styles (Cards, Sections). All three share the same
/// dash widths so their `┬`/`┼`/`┴` joints line up vertically.
fn frame_borders(max_label: usize, content_width: usize, g: &FrameGlyphs) -> FrameBorders {
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

/// Emit the body rows for one group in a walled style (Cards / Sections).
/// First row of the group shows the label; continuation rows blank it so the
/// `│` divider column lines up vertically.
#[allow(clippy::too_many_arguments)]
fn push_walled_group_rows(
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

/// Zones: one labelled horizontal rule separating state (Identity/Config/
/// Budget) from activity (Tools/Agents/Todos). Uses CC's own visual idiom
/// (single thin rule) so the statusline reads as an extension of the input
/// box rather than a competing panel.
fn render_zones(
    grouped: &Grouped<'_>,
    lines: &[String],
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
) -> Vec<String> {
    let has_activity = groups
        .iter()
        .any(|(k, r)| matches!(k, LineKind::Activity) && r.start < r.end);

    // No activity ⇒ skip the rule entirely; zones degrades to plain output.
    if !has_activity {
        return lines.to_vec();
    }

    let ruler_width = resolve_inner_width(grouped, cfg);
    if ruler_width < cfg.min_width {
        return lines.to_vec();
    }

    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 1);
    let mut rule_emitted = false;

    for (kind, range) in groups {
        if matches!(kind, LineKind::Activity) && !rule_emitted {
            out.push(render_section_divider("activity", ruler_width, g));
            rule_emitted = true;
        }
        for idx in range.start..range.end {
            if let Some(line) = lines.get(idx) {
                out.push(line.clone());
            }
        }
    }
    out
}

type Grouped<'a> = [(&'a str, Vec<&'a str>)];

fn collect_grouped_lines<'a>(
    lines: &'a [String],
    groups: &[(LineKind, Range<usize>)],
    configured: &'a [PaneGroup],
) -> Vec<(&'a str, Vec<&'a str>)> {
    let mut grouped: Vec<(&str, Vec<&str>)> = Vec::with_capacity(configured.len());
    for pane_group in configured {
        let mut collected: Vec<&str> = Vec::new();
        for (kind, range) in groups {
            if pane_group.kinds.contains(kind) {
                for idx in range.start..range.end {
                    if let Some(line) = lines.get(idx) {
                        collected.push(line.as_str());
                    }
                }
            }
        }
        if !collected.is_empty() {
            grouped.push((pane_group.label.as_str(), collected));
        }
    }
    grouped
}

fn resolve_inner_width(grouped: &Grouped<'_>, cfg: &PaneConfig) -> usize {
    let content_max = grouped
        .iter()
        .flat_map(|(_, ls)| ls.iter())
        .map(|s| visible_width(s))
        .max()
        .unwrap_or(0);
    let label_max = grouped
        .iter()
        .map(|(label, _)| visible_width(label) + 2)
        .max()
        .unwrap_or(0);

    let max_width = cfg.max_width.max(cfg.min_width); // guard against misconfig (min > max)
    let raw = match cfg.width_mode {
        PaneWidth::Auto => content_max.max(label_max),
        PaneWidth::Fixed(w) => w.max(label_max),
        PaneWidth::Terminal => {
            // Span the detected terminal width, minus `cc_margin` cols for
            // Claude Code's statusline padding. CC allocates the statusline a
            // sub-region of the raw terminal — empirically ~1-4 cols narrower;
            // lines at exactly the raw width trigger wrap and collapse the
            // whole multi-line render. Defaults to 4 cols (verified safe on
            // CC 2.1.119); configurable via `pane.cc_margin` for other hosts.
            //
            // When detection failed (`terminal_width = None`), fall back to
            // content-fit (Auto behavior) — NOT to max_width.
            match cfg.terminal_width {
                Some(term) => term.saturating_sub(cfg.cc_margin).max(label_max),
                None => content_max.max(label_max),
            }
        }
    };
    raw.clamp(cfg.min_width, max_width)
}

fn render_section_divider(label: &str, ruler_width: usize, g: &FrameGlyphs) -> String {
    // Pattern: `─── {label} ` + fill × `─`, targeting `ruler_width` total visible chars.
    const PREFIX_DASHES: usize = 3;
    let label_w = visible_width(label);
    let overhead = PREFIX_DASHES + 1 + label_w + 1; // "─── label "
    let fill = ruler_width.saturating_sub(overhead);
    let head = g.h.repeat(PREFIX_DASHES);
    let tail = g.h.repeat(fill);
    format!("{head} {label} {tail}")
}

struct FrameGlyphs {
    h: &'static str,
    v: &'static str,
    tl: &'static str,
    tr: &'static str,
    bl: &'static str,
    br: &'static str,
    tee_t: &'static str,
    tee_b: &'static str,
    tee_l: &'static str,
    tee_r: &'static str,
    cross: &'static str,
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

fn glyphs(mode: GlyphMode) -> &'static FrameGlyphs {
    match mode {
        GlyphMode::Icon => &UNICODE_GLYPHS,
        GlyphMode::Ascii => &ASCII_GLYPHS,
    }
}
