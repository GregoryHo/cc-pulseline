use std::ops::Range;

use crate::config::GlyphMode;

use super::frames;

/// User-facing layout style. Each variant maps 1:1 to a `name = "..."` value
/// in `[layout]` config and to a render fn in `super::frames`.
///
/// Two flavours coexist:
/// - **Flat-row layouts** (`None`, `Zones`, `Grid`, `Cards`, `Sections`) emit
///   v1-style line output and may be wrapped by `apply_pane()` chrome.
/// - **Instrument-cluster layouts** (`Cockpit`, `Console`, `Flightstrip`,
///   `Auto`) own the full render pipeline and bypass `apply_pane()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutStyle {
    /// Flat output, no decoration. Default.
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
    /// One independent `╭─┬─╮ / ╰─┴─╯` frame per group, stacked vertically.
    /// Each group (Identity / Config / Budget / Activity) becomes its own
    /// self-contained card. All cards share a global `max_label_width` and
    /// `max_content_width` so they line up when stacked. Adds 2 rows per
    /// non-empty group (top + bottom of each card).
    Cards,
    /// Single outer `╭─┬─╮ / ╰─┴─╯` wrapper with a `├─┼─┤` separator
    /// emitted between every pair of non-empty groups. Reads as one
    /// container with explicit internal dividers — cheaper than Cards
    /// (no double-border gaps).
    Sections,
    /// 3-row instrument cluster (default after the v2 flip).
    /// Identity headline + cluster (gauge, sparkline, rate, cost, quota)
    /// + activity ticker.
    Cockpit,
    /// 4-5 row framed dashboard (highest "quality feel"). Best when
    /// statusline is ≥130 cols. Wraps content in `╭─╮ │ ╰─╯`.
    Console,
    /// Dense 2-row strip for narrow IDE statuslines.
    Flightstrip,
    /// Width-bracket resolver — picks console/cockpit/flightstrip per
    /// terminal width on every render tick.
    Auto,
    /// Label-value pairs aligned in a fixed left column, like an
    /// accounting ledger. Owns its own pipeline (framed). One TAG per
    /// metric (ENV / CTX / TOK / COST / 5h / 7d / TOOL / AGENT / TODO);
    /// blank rows separate groups. Tallest layout — favours typographic
    /// rhythm over density.
    Ledger,
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
    pub style: LayoutStyle,
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
/// Returns `lines` unchanged when `cfg.style == LayoutStyle::None` or when
/// the style is an instrument-cluster layout (which renders independently
/// upstream of this function), or when the terminal can't fit `cfg.min_width`.
pub fn apply_pane(
    lines: Vec<String>,
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
) -> Vec<String> {
    match cfg.style {
        LayoutStyle::None
        | LayoutStyle::Cockpit
        | LayoutStyle::Flightstrip
        | LayoutStyle::Auto
        | LayoutStyle::Ledger => return lines,
        // Console flows through here now (sections + identity-in-title).
        LayoutStyle::Zones
        | LayoutStyle::Grid
        | LayoutStyle::Cards
        | LayoutStyle::Sections
        | LayoutStyle::Console => {}
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

    let g = frames::shared::glyphs(cfg.glyph_mode);
    match cfg.style {
        LayoutStyle::Zones => frames::zones::render(&grouped, &lines, groups, cfg, g),
        LayoutStyle::Grid => frames::grid::render(&lines, groups, cfg, g),
        LayoutStyle::Cards => frames::cards::render(&lines, groups, cfg, g),
        LayoutStyle::Sections => frames::sections::render(&lines, groups, cfg, g),
        LayoutStyle::Console => frames::console::render(&lines, groups, cfg, g),
        // Decoration-bypassing styles short-circuited at the top of this fn.
        LayoutStyle::None
        | LayoutStyle::Cockpit
        | LayoutStyle::Flightstrip
        | LayoutStyle::Auto
        | LayoutStyle::Ledger => lines,
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
