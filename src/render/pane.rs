use std::ops::Range;

use crate::config::GlyphMode;

use super::color::visible_width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneStyle {
    None,
    Rail,
    Box,
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

/// Insert group separators around `lines`:
/// - Box: labelled horizontal dividers (`─── Config ──────`) between groups,
///   skipping the first so Claude Code's own divider above the statusline
///   acts as the top edge. No side or bottom borders.
/// - Rail: left-side `│` guide with `├ Label` shoulder between groups.
///
/// Returns `lines` unchanged when `cfg.style == PaneStyle::None` or when the
/// terminal can't fit `cfg.min_width` plus border cost.
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

    // Safety gate: skip framing when the terminal can't fit min_width. The old
    // `+ 4` was for side-border cost; the current box mode has none and rail
    // eats only 2 cols, so gate on min_width alone (conservative enough).
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
        PaneStyle::Box => render_box(&grouped, &lines, cfg, g),
        PaneStyle::Rail => render_rail(&grouped, g),
        PaneStyle::None => lines,
    }
}

type Grouped<'a> = [(&'a str, Vec<&'a str>)];

fn render_box(
    grouped: &Grouped<'_>,
    lines: &[String],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
) -> Vec<String> {
    let ruler_width = resolve_inner_width(grouped, cfg);
    if ruler_width < cfg.min_width {
        return lines.to_vec();
    }

    let mut out: Vec<String> = Vec::with_capacity(grouped.len() + lines.len());
    for (i, (label, group_lines)) in grouped.iter().enumerate() {
        // Skip the divider above the first group — Claude Code's own separator
        // already draws a line above the statusline.
        if i > 0 {
            out.push(render_section_divider(label, ruler_width, g));
        }
        for line in group_lines {
            out.push((*line).to_string());
        }
    }
    out
}

fn render_rail(grouped: &Grouped<'_>, g: &FrameGlyphs) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(grouped.len() * 2 + 1);
    for (i, (label, group_lines)) in grouped.iter().enumerate() {
        let shoulder = if i == 0 { g.tl } else { g.tee_l };
        out.push(format!("{shoulder} {label}"));
        for line in group_lines {
            out.push(format!("{v} {line}", v = g.v));
        }
    }
    out.push(g.bl.to_string());
    out
}

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
    tl: &'static str,
    bl: &'static str,
    h: &'static str,
    v: &'static str,
    tee_l: &'static str,
}

const UNICODE_GLYPHS: FrameGlyphs = FrameGlyphs {
    tl: "╭",
    bl: "╰",
    h: "─",
    v: "│",
    tee_l: "├",
};

const ASCII_GLYPHS: FrameGlyphs = FrameGlyphs {
    tl: "+",
    bl: "+",
    h: "-",
    v: "|",
    tee_l: "+",
};

fn glyphs(mode: GlyphMode) -> &'static FrameGlyphs {
    match mode {
        GlyphMode::Icon => &UNICODE_GLYPHS,
        GlyphMode::Ascii => &ASCII_GLYPHS,
    }
}
