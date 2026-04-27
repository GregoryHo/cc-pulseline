use std::ops::Range;

use crate::render::pane::{LineKind, PaneConfig};

use super::shared::{label_for_kind, max_content_width, max_label_width, pad_to, FrameGlyphs};

pub fn render(
    lines: &[String],
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
) -> Vec<String> {
    // +2 cells form the visual gap before the `│` divider. Cards/Sections
    // don't need this since their `│ ` wall already provides gap spacing.
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
