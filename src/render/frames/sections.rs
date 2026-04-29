use std::ops::Range;

use crate::render::color::visible_width;
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
    render_with_options(lines, groups, cfg, g, None)
}

/// Sections with optional identity-in-title.
///
/// When `identity_title` is `Some(text)`:
/// - The top frame border becomes `╭─ <text> ──...──╮` (no `tee_t` join,
///   plain dashes filling out the same total visible width as the
///   bottom border).
/// - Groups whose `kind == LineKind::Identity` are skipped during body
///   walling — the title carries that information instead.
///
/// Console layout drives this; plain Sections passes `None` so the
/// shared border helpers run unchanged.
pub fn render_with_options(
    lines: &[String],
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
    identity_title: Option<&str>,
) -> Vec<String> {
    let max_label = max_label_width(&cfg.groups);
    let content_width = max_content_width(lines);
    let borders = frame_borders(max_label, content_width, g);

    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 2 + groups.len());
    out.push(match identity_title {
        Some(text) => identity_top_border(text, max_label, content_width, g),
        None => borders.top,
    });

    let mut emitted_first_group = false;
    for (kind, range) in groups {
        if range.start >= range.end {
            continue;
        }
        // When identity is hoisted into the title, skip its body rows.
        if identity_title.is_some() && *kind == LineKind::Identity {
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

/// Build a top border that bakes the identity into the dashes:
/// `╭─ <title> ───────────╮`. Uses *plain* dashes (no `tee_t`) so the
/// title reads as a single continuous frame title — the inner-wall
/// alignment with `cross` and `tee_b` further down is preserved by the
/// fact that the wall starts at the first body row, not the top frame.
fn identity_top_border(
    title: &str,
    max_label: usize,
    content_width: usize,
    g: &FrameGlyphs,
) -> String {
    // Total visible width of the bottom border (which includes `tee_b`):
    // 1 (bl) + (max_label+2) + 1 (tee_b) + (content_width+2) + 1 (br).
    let total = max_label + content_width + 7;
    let title_w = visible_width(title);
    // Layout: tl + dash + space + title + space + dashes + tr.
    // Fixed overhead = 1 (tl) + 1 (─) + 1 (sp) + 1 (sp) + 1 (tr) = 5.
    let dashes_after = total.saturating_sub(5 + title_w).max(1);
    format!(
        "{tl}{h} {title} {dashes}{tr}",
        tl = g.tl,
        h = g.h,
        dashes = g.h.repeat(dashes_after),
        tr = g.tr,
    )
}

