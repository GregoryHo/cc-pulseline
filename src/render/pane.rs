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

#[derive(Debug, Clone)]
pub struct PaneConfig {
    pub style: PaneStyle,
    pub width_mode: PaneWidth,
    pub min_width: usize,
    pub max_width: usize,
    pub groups: Vec<PaneGroup>,
    pub glyph_mode: GlyphMode,
    pub terminal_width: Option<usize>,
}

/// Wrap `lines` in a Unicode-box frame with `├─ Label ─┤` dividers between groups,
/// right-padded so every interior line aligns to the same visible width.
/// Returns `lines` unchanged when `cfg.style == PaneStyle::None` or when the
/// resolved inner width would fall below `cfg.min_width`.
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

    // Safety gate: skip framing when the terminal can't fit the user's min_width + border cost.
    if let Some(term) = cfg.terminal_width {
        if term < cfg.min_width + 4 {
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
    let inner_width = resolve_inner_width(grouped, cfg);
    if inner_width < cfg.min_width {
        return lines.to_vec();
    }

    let mut out: Vec<String> = Vec::with_capacity(grouped.len() * 2 + 1 + lines.len());
    for (i, (label, group_lines)) in grouped.iter().enumerate() {
        out.push(render_divider(i == 0, label, inner_width, g));
        for line in group_lines {
            out.push(render_content_line(line, inner_width, g));
        }
    }
    out.push(render_bottom_border(inner_width, g));
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
            let term = cfg.terminal_width.unwrap_or(max_width);
            term.saturating_sub(4).max(label_max)
        }
    };
    raw.clamp(cfg.min_width, max_width)
}

fn render_divider(is_top: bool, label: &str, inner_width: usize, g: &FrameGlyphs) -> String {
    let (left, right) = if is_top {
        (g.tl, g.tr)
    } else {
        (g.tee_l, g.tee_r)
    };
    let label_w = visible_width(label);
    // `{left}{h} {label} ` + fill × `{h}` + `{right}`, total visible chars = inner_width + 4.
    if label_w + 2 > inner_width {
        let fill = inner_width + 2;
        let dashes = g.h.repeat(fill);
        return format!("{left}{dashes}{right}");
    }
    let fill = inner_width - label_w - 1;
    let dashes = g.h.repeat(fill);
    format!("{left}{h} {label} {dashes}{right}", h = g.h)
}

fn render_bottom_border(inner_width: usize, g: &FrameGlyphs) -> String {
    let fill = inner_width + 2;
    let dashes = g.h.repeat(fill);
    format!("{l}{dashes}{r}", l = g.bl, r = g.br)
}

fn render_content_line(line: &str, inner_width: usize, g: &FrameGlyphs) -> String {
    let w = visible_width(line);
    let padding = inner_width.saturating_sub(w);
    let pad = " ".repeat(padding);
    format!("{v} {line}{pad} {v}", v = g.v)
}

struct FrameGlyphs {
    tl: &'static str,
    tr: &'static str,
    bl: &'static str,
    br: &'static str,
    h: &'static str,
    v: &'static str,
    tee_l: &'static str,
    tee_r: &'static str,
}

const UNICODE_GLYPHS: FrameGlyphs = FrameGlyphs {
    tl: "╭",
    tr: "╮",
    bl: "╰",
    br: "╯",
    h: "─",
    v: "│",
    tee_l: "├",
    tee_r: "┤",
};

const ASCII_GLYPHS: FrameGlyphs = FrameGlyphs {
    tl: "+",
    tr: "+",
    bl: "+",
    br: "+",
    h: "-",
    v: "|",
    tee_l: "+",
    tee_r: "+",
};

fn glyphs(mode: GlyphMode) -> &'static FrameGlyphs {
    match mode {
        GlyphMode::Icon => &UNICODE_GLYPHS,
        GlyphMode::Ascii => &ASCII_GLYPHS,
    }
}
