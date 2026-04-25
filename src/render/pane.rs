use std::ops::Range;

use crate::config::GlyphMode;

use super::frames;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneStyle {
    /// v1: flat output, no decoration. Default.
    V1None,
    /// v1: two strata separated by a single labelled rule (echoes CC's own
    /// horizontal rules above/below the input box). State (Identity/Config/
    /// Budget) above, `──── activity ────` rule, then live Activity below.
    V1Zones,
    /// v1: table layout with a fixed label column + `│` divider + right-padded
    /// content. Every line begins and ends at the same visual position —
    /// solves jagged right edges and makes group boundaries explicit without
    /// adding rows. Activity continuation rows span the label column.
    V1Grid,
    /// v1: one independent `╭─┬─╮ / ╰─┴─╯` frame per group, stacked vertically.
    /// Each group (Identity / Config / Budget / Activity) becomes its own
    /// self-contained card. All cards share a global `max_label_width` and
    /// `max_content_width` so they line up when stacked. Adds 2 rows per
    /// non-empty group (top + bottom of each card).
    V1Cards,
    /// v1: single outer `╭─┬─╮ / ╰─┴─╯` wrapper with a `├─┼─┤` separator
    /// emitted between every pair of non-empty groups. Reads as one
    /// container with explicit internal dividers — cheaper than Cards
    /// (no double-border gaps).
    V1Sections,
    /// v2: 3-row instrument cluster (default after the v2 flip).
    /// Identity headline + cluster (gauge, sparkline, rate, cost, quota)
    /// + activity ticker.
    V2Cockpit,
    /// v2: 4-5 row framed dashboard (highest "quality feel"). Best when
    /// statusline is ≥130 cols. Wraps content in `╭─╮ │ ╰─╯`.
    V2Console,
    /// v2: dense 2-row strip for narrow IDE statuslines.
    V2Flightstrip,
    /// v2: width-bracket resolver — picks console/cockpit/flightstrip per
    /// terminal width on every render tick.
    V2Auto,
}

impl PaneStyle {
    /// Returns true for the v2 instrument-cluster layouts (cockpit, console,
    /// flightstrip, auto). v2 layouts own the entire rendering pipeline and
    /// bypass `apply_pane`.
    pub fn is_v2(self) -> bool {
        matches!(
            self,
            PaneStyle::V2Cockpit
                | PaneStyle::V2Console
                | PaneStyle::V2Flightstrip
                | PaneStyle::V2Auto
        )
    }
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
/// Returns `lines` unchanged when `cfg.style == PaneStyle::V1None` or when the
/// terminal can't fit `cfg.min_width`.
pub fn apply_pane(
    lines: Vec<String>,
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
) -> Vec<String> {
    if matches!(cfg.style, PaneStyle::V1None) || cfg.style.is_v2() {
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

    let g = frames::v1::shared::glyphs(cfg.glyph_mode);
    match cfg.style {
        PaneStyle::V1Zones => frames::v1::zones::render(&grouped, &lines, groups, cfg, g),
        PaneStyle::V1Grid => frames::v1::grid::render(&lines, groups, cfg, g),
        PaneStyle::V1Cards => frames::v1::cards::render(&lines, groups, cfg, g),
        PaneStyle::V1Sections => frames::v1::sections::render(&lines, groups, cfg, g),
        // v2 styles bypass apply_pane (handled at the top of this fn) and
        // V1None means "no decoration" — both paths return raw lines.
        PaneStyle::V1None
        | PaneStyle::V2Cockpit
        | PaneStyle::V2Console
        | PaneStyle::V2Flightstrip
        | PaneStyle::V2Auto => lines,
    }
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
