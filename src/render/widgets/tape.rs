//! Recent-tools tape — horizontal strip summarising the latest tool uses.
//!
//! Two modes selected via `with_target`:
//! - **brief** (`tools_visual = "tape"`): `▶ Read · ▶ Bash`.
//!   Just an "is running" indicator with the running-arrow icon.
//! - **detailed** (`tools_visual = "tape+detail"`):
//!   `<icon> Read: src/main.rs · <icon> Bash: cargo test` — same per-tool
//!   format as the flat-row layouts' `tools_visual = "list"` activity rows.
//!
//! Tape semantics: this widget is for **running / very recent** tools
//! (`SessionState::recent_tools`). Completed-count summaries (`✓ Read ×8`)
//! live in a separate widget (`completed_tool_chips`) and stay description-
//! free — the count is the whole point. The split keeps "what's happening
//! now" (target-rich, in detail mode) visually distinct from "what's been
//! done" (count-only).
//!
//! ## Widget contract
//!
//! `render` returns a `Vec<Cell>` rather than a finished string. The
//! caller (typically `render::frames::shared::render_tools_visual_inline`)
//! is the row composer — it knows the pane's actual width budget and
//! feeds the cells through `pack_with_separator`. This is the same
//! widget→cells→budgeter→string flow the flat-row activity builder uses,
//! so cluster layouts inherit the same width-aware truncation.
//!
//! Color in brief mode: `aurora_mid` for the icon (the eye learns the
//! shape, not a colour per tool) and `secondary` for the text. The whole
//! tape is one visual cluster — separators stay in `separator` tone.
//! Detail mode adopts `tool_blue` + `secondary` (matching the flat-row
//! per-tool format).

use crate::config::GlyphMode;
use crate::render::activity::cell::{Cell, CellPriority};
use crate::render::color::{colorize, ThemePalette};
use crate::types::ToolSummary;

const ICON_RUNNING: &str = "\u{25B6}"; // ▶ (Nerd Font / wide-coverage Unicode)
const ASCII_RUNNING: &str = ">"; // plain-text fallback under display.icons=false
/// Visible separator the dispatch hub uses when packing tape cells. Exposed so
/// `render_tools_visual_inline` and tests don't need to re-encode it.
pub const SEPARATOR: &str = " \u{00B7} "; // ' · ' — middle dot, broad font support
pub const SEPARATOR_W: usize = 3;

/// Build per-tool cells for the tape strip, capped at `max_items`.
/// Returns the trailing window so the most recent tool sits at the right
/// edge of the rendered row. Empty input or `max_items == 0` returns an
/// empty vector — caller is expected to skip the segment.
///
/// `with_target` selects between brief (`▶ Read`) and detailed
/// (`<icon> Read: src/main.rs`) per-tool format. Detailed cells go
/// through the shared `recent_tool::build_recent_tool_cell` so they
/// inherit `min_width = 8`, the per-tool `target_strategy_for(name)`
/// truncation, and `Required` priority — the row budgeter compresses
/// targets under width pressure instead of dropping cells.
pub fn render(
    tools: &[ToolSummary],
    max_items: usize,
    mode: GlyphMode,
    palette: &ThemePalette,
    color_enabled: bool,
    with_target: bool,
) -> Vec<Cell> {
    if tools.is_empty() || max_items == 0 {
        return Vec::new();
    }
    let start = tools.len().saturating_sub(max_items);
    let slice = &tools[start..];

    slice
        .iter()
        .map(|t| {
            if with_target {
                super::recent_tool::build_recent_tool_cell(t, mode, palette, color_enabled)
            } else {
                build_brief_cell(t, mode, palette, color_enabled)
            }
        })
        .collect()
}

/// Brief-mode cell: `<arrow> <name>` as a single label cell.
///
/// `Optional` priority — under extreme width pressure the budgeter drops
/// rightmost (newest) tools, which matches tape's "recent at right" axis
/// reading: when the row can't fit, hide the very latest entries rather
/// than overflow into a second statusline row.
fn build_brief_cell(
    tool: &ToolSummary,
    mode: GlyphMode,
    palette: &ThemePalette,
    color_enabled: bool,
) -> Cell {
    let arrow = match mode {
        GlyphMode::Icon => ICON_RUNNING,
        GlyphMode::Ascii => ASCII_RUNNING,
    };
    let icon = colorize(&format!("{arrow} "), &palette.aurora_mid, color_enabled);
    let name = colorize(&tool.name, &palette.secondary, color_enabled);
    let head = format!("{icon}{name}");
    // arrow (1 col) + space + name chars
    let head_w = 2 + tool.name.chars().count();
    Cell::label(head, head_w, CellPriority::Optional)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity::budget::pack_with_separator;
    use crate::render::widgets::test_support::aurora_marker_palette;

    fn t(name: &str) -> ToolSummary {
        ToolSummary {
            id: format!("id-{name}"),
            name: name.to_string(),
            target: None,
        }
    }

    fn render_packed(
        tools: &[ToolSummary],
        max_items: usize,
        max_width: usize,
        mode: GlyphMode,
        p: &ThemePalette,
        color: bool,
        with_target: bool,
    ) -> String {
        let cells = render(tools, max_items, mode, p, color, with_target);
        let sep = colorize(SEPARATOR, &p.separator, color);
        pack_with_separator(&cells, max_width, &sep, SEPARATOR_W, color)
    }

    #[test]
    fn empty_tools_renders_empty() {
        let p = aurora_marker_palette();
        let cells = render(&[], 5, GlyphMode::Icon, &p, false, false);
        assert!(cells.is_empty());
    }

    #[test]
    fn max_items_zero_renders_empty() {
        let p = aurora_marker_palette();
        let cells = render(&[t("Read")], 0, GlyphMode::Icon, &p, false, false);
        assert!(cells.is_empty());
    }

    #[test]
    fn most_recent_appear_at_right_edge() {
        let p = aurora_marker_palette();
        let tools = vec![t("Read"), t("Grep"), t("Edit"), t("Bash")];
        let s = render_packed(&tools, 2, 200, GlyphMode::Icon, &p, false, false);
        assert!(s.contains("Edit"));
        assert!(s.contains("Bash"));
        assert!(!s.contains("Read"));
        assert!(!s.contains("Grep"));
        let edit_pos = s.find("Edit").unwrap();
        let bash_pos = s.find("Bash").unwrap();
        assert!(edit_pos < bash_pos);
    }

    #[test]
    fn separator_between_items() {
        let p = aurora_marker_palette();
        let tools = vec![t("A"), t("B"), t("C")];
        let s = render_packed(&tools, 5, 200, GlyphMode::Icon, &p, false, false);
        assert_eq!(s.matches(SEPARATOR).count(), 2);
    }

    #[test]
    fn single_tool_has_no_separator() {
        let p = aurora_marker_palette();
        let s = render_packed(&[t("Bash")], 5, 200, GlyphMode::Icon, &p, false, false);
        assert!(!s.contains(SEPARATOR));
        assert!(s.contains("Bash"));
        assert!(s.contains(ICON_RUNNING));
    }

    #[test]
    fn color_uses_aurora_mid_for_icon() {
        let mut p = aurora_marker_palette();
        p.secondary = "SEC".to_string();
        let s = render_packed(&[t("Read")], 5, 200, GlyphMode::Icon, &p, true, false);
        assert!(s.contains("MID"));
        assert!(s.contains("SEC"));
    }

    #[test]
    fn ascii_mode_uses_gt_arrow_not_unicode_triangle() {
        let p = aurora_marker_palette();
        let s = render_packed(&[t("Bash")], 5, 200, GlyphMode::Ascii, &p, false, false);
        assert!(s.contains(ASCII_RUNNING), "expected '>' in {s:?}");
        assert!(
            !s.contains(ICON_RUNNING),
            "did not expect U+25B6 (▶) in {s:?}"
        );
        assert!(s.contains("Bash"));
    }

    #[test]
    fn ascii_mode_keeps_middle_dot_separator() {
        let p = aurora_marker_palette();
        let s = render_packed(
            &[t("Read"), t("Bash")],
            5,
            200,
            GlyphMode::Ascii,
            &p,
            false,
            false,
        );
        assert_eq!(s.matches(SEPARATOR).count(), 1);
    }

    #[test]
    fn detail_long_bash_target_compresses_under_width_pressure() {
        // Detail-mode cells go through `build_recent_tool_cell` with
        // `Required` priority + `min_width = 8`; the budgeter compresses
        // the target to fit even when ideal would overflow. This is the
        // regression test for the "tape+detail breaks Console multi-line"
        // bug — under a 60-col budget, two long Bash cells must still
        // pack into a single line that's `<= 60` visible chars wide.
        use crate::render::color::visible_width;
        let p = aurora_marker_palette();
        let mut tools = Vec::new();
        for _ in 0..2 {
            tools.push(ToolSummary {
                id: "x".to_string(),
                name: "Bash".to_string(),
                target: Some(
                    "cargo test --release --no-default-features --features experimental_quota"
                        .to_string(),
                ),
            });
        }
        let s = render_packed(&tools, 2, 60, GlyphMode::Ascii, &p, false, true);
        let w = visible_width(&s);
        assert!(w <= 60, "row exceeds budget: w={w}, s={s:?}");
        assert!(s.contains("Bash"), "names preserved under pressure: {s:?}");
    }
}
