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
        PaneStyle::None => lines,
    }
}

/// Grid: fixed-width left label column + `│` + content right-padded to the
/// longest natural row. Activity continuation rows show blank label (span).
fn render_grid(
    lines: &[String],
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
) -> Vec<String> {
    // Label width = longest configured group label + 2 cols padding.
    let label_width = cfg
        .groups
        .iter()
        .map(|pg| visible_width(&pg.label))
        .max()
        .unwrap_or(0)
        + 2;

    // Content width = longest natural line; right-pad every line to this.
    let content_width = lines.iter().map(|l| visible_width(l)).max().unwrap_or(0);

    let label_for = |kind: LineKind| -> &str {
        cfg.groups
            .iter()
            .find(|pg| pg.kinds.contains(&kind))
            .map(|pg| pg.label.as_str())
            .unwrap_or("")
    };

    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (kind, range) in groups {
        let label = label_for(*kind);
        let mut first = true;
        for idx in range.start..range.end {
            let Some(line) = lines.get(idx) else { continue };
            let lbl = if first { label } else { "" };
            let lbl_pad = label_width.saturating_sub(visible_width(lbl));
            let content_pad = content_width.saturating_sub(visible_width(line));
            let lpad = " ".repeat(lbl_pad);
            let rpad = " ".repeat(content_pad);
            out.push(format!("{lbl}{lpad}{v} {line}{rpad}", v = g.v));
            first = false;
        }
    }
    out
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
}

const UNICODE_GLYPHS: FrameGlyphs = FrameGlyphs { h: "─", v: "│" };

const ASCII_GLYPHS: FrameGlyphs = FrameGlyphs { h: "-", v: "|" };

fn glyphs(mode: GlyphMode) -> &'static FrameGlyphs {
    match mode {
        GlyphMode::Icon => &UNICODE_GLYPHS,
        GlyphMode::Ascii => &ASCII_GLYPHS,
    }
}
