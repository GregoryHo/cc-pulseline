use std::ops::Range;

use crate::render::color::visible_width;
use crate::render::pane::{LineKind, PaneConfig, PaneWidth};

use super::shared::FrameGlyphs;

pub fn render(
    grouped: &[(&str, Vec<&str>)],
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

fn resolve_inner_width(grouped: &[(&str, Vec<&str>)], cfg: &PaneConfig) -> usize {
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
    const PREFIX_DASHES: usize = 3;
    let label_w = visible_width(label);
    let overhead = PREFIX_DASHES + 1 + label_w + 1; // "─── label "
    let fill = ruler_width.saturating_sub(overhead);
    let head = g.h.repeat(PREFIX_DASHES);
    let tail = g.h.repeat(fill);
    format!("{head} {label} {tail}")
}
