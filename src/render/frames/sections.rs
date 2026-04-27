use std::ops::Range;

use crate::render::pane::{LineKind, PaneConfig};

use super::shared::{
    frame_borders, max_content_width, max_label_width, push_walled_group_rows, FrameGlyphs,
};

pub fn render(
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
