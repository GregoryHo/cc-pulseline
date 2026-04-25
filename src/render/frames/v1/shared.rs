use std::ops::Range;

use crate::config::GlyphMode;
use crate::render::color::visible_width;
use crate::render::pane::{LineKind, PaneConfig, PaneGroup};

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
