use std::ops::Range;

use crate::config::GlyphMode;

use super::frames;

/// User-facing layout style. Each variant maps 1:1 to a `name = "..."` value
/// in `[layout]` config and to a render fn in `super::frames`.
///
/// All layouts are flat-row at the data level: layout.rs assembles
/// `(lines, groups)`, then `apply_pane()` decorates them with chrome. The
/// only exception is `Ledger`, which owns its full pipeline because the
/// TAG-column rhythm doesn't compose cleanly via `apply_pane`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutStyle {
    /// Flat output, no decoration. Default.
    None,
    /// 2–3 row micro layout: packed identity row, packed budget + compact
    /// quota row, and a single packed activity ticker that only appears
    /// when there is activity (idle footprint = 2 rows). No chrome.
    Compact,
    /// Flat dashboard of three column-aligned, equal-weight gauges —
    /// CONTEXT / 5H QUOTA / 7D QUOTA — stacked under the identity row, so
    /// burn across the three budget windows reads on a shared spatial
    /// axis. Reuses the bracketless `▰─·` gauge at a wider width; the
    /// percentage is the D2 "anchor" (hero metric in its threshold color).
    /// A TOKENS row closes it. No chrome — routes through `apply_pane`
    /// like `none`.
    Budgets,
    /// Single outer `╭─...─╮` wrapper with a `├─┼─┤` separator emitted
    /// between every pair of non-empty groups, and the Identity row
    /// hoisted into the top frame title:
    /// `╭─ <model · path · branch> ───╮`. Best when statusline is
    /// ≥110 cols.
    Console,
    /// Label-value pairs aligned in a fixed left column, like an
    /// accounting ledger. Owns its own pipeline (framed). One TAG per
    /// metric (ENV / CTX / TOK / COST / 5h / 7d / TOOL / AGENT / TODO);
    /// blank rows separate groups. Tallest layout — favours typographic
    /// rhythm over density.
    Ledger,
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

/// Cols subtracted from the detected `terminal_width` before layout.
/// Claude Code allocates the statusline a sub-region that is ~1-4 cols
/// narrower than the raw terminal; lines at exactly the raw width trigger
/// wrap and collapse multi-line rendering to a single visible line. 4 is
/// the empirically verified safe default on CC 2.1.119.
pub const DEFAULT_PANE_CC_MARGIN: usize = 4;

#[derive(Debug, Clone)]
pub struct PaneConfig {
    pub style: LayoutStyle,
    pub min_width: usize,
    pub max_width: usize,
    pub groups: Vec<PaneGroup>,
    pub glyph_mode: GlyphMode,
    pub terminal_width: Option<usize>,
    pub cc_margin: usize,
}

/// Apply the configured pane style to `lines`.
///
/// Returns `lines` unchanged when `cfg.style == LayoutStyle::None` (no
/// chrome to apply) or `Ledger` (renders independently upstream of this
/// function), or when the terminal can't fit `cfg.min_width`.
pub fn apply_pane(
    lines: Vec<String>,
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
) -> Vec<String> {
    match cfg.style {
        LayoutStyle::None | LayoutStyle::Compact | LayoutStyle::Budgets | LayoutStyle::Ledger => {
            return lines
        }
        LayoutStyle::Console => {}
    }
    if lines.is_empty() || cfg.groups.is_empty() {
        return lines;
    }

    if let Some(term) = cfg.terminal_width {
        if term < cfg.min_width {
            return lines;
        }
    }

    // No group has content → nothing to frame; pass through unchanged.
    let grouped = collect_grouped_lines(&lines, groups, &cfg.groups);
    if grouped.is_empty() {
        return lines;
    }

    let g = frames::shared::glyphs(cfg.glyph_mode);
    frames::console::render(&lines, groups, cfg, g)
}

pub(super) fn collect_grouped_lines<'a>(
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
