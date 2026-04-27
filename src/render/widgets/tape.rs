//! Recent-tools tape — horizontal strip summarising the latest tool uses.
//!
//! Reads `recent_tools` chronologically (oldest → newest) and emits one cell
//! per tool: `▶ Read`, `▶ Bash`, etc., separated by a tonal middle-dot. Used
//! by the v2 Cockpit / Flightstrip activity rows where the standard
//! per-line tool listing is too tall.
//!
//! Color is uniformly `aurora_mid` for the icon (the eye learns the shape,
//! not a color per tool) and `secondary` for the text. The whole tape is one
//! visual cluster — separators stay in `separator` tone.

use crate::config::GlyphMode;
use crate::render::color::{colorize, ThemePalette};
use crate::types::ToolSummary;

const ICON_RUNNING: &str = "\u{25B6}"; // ▶ (Nerd Font / wide-coverage Unicode)
const ASCII_RUNNING: &str = ">"; // plain-text fallback under display.icons=false
const SEP: &str = " \u{00B7} "; // ' · ' — middle dot, broad font support

/// Render a tape from `tools`, capped at `max_items`. When `tools` is empty
/// returns an empty string (caller is expected to skip the segment).
///
/// The leading per-tool arrow uses U+25B6 (▶) under `GlyphMode::Icon` and
/// `>` under `GlyphMode::Ascii` — the rest of the cell is identical.
pub fn render(
    tools: &[ToolSummary],
    max_items: usize,
    mode: GlyphMode,
    palette: &ThemePalette,
    color_enabled: bool,
) -> String {
    if tools.is_empty() || max_items == 0 {
        return String::new();
    }
    // Most recent at the right edge — trailing window of length max_items.
    let start = tools.len().saturating_sub(max_items);
    let slice = &tools[start..];

    let sep = colorize(SEP, &palette.separator, color_enabled);
    let parts: Vec<String> = slice
        .iter()
        .map(|t| format_one(t, mode, palette, color_enabled))
        .collect();
    parts.join(&sep)
}

fn format_one(
    tool: &ToolSummary,
    mode: GlyphMode,
    palette: &ThemePalette,
    color_enabled: bool,
) -> String {
    let arrow = match mode {
        GlyphMode::Icon => ICON_RUNNING,
        GlyphMode::Ascii => ASCII_RUNNING,
    };
    let icon = colorize(&format!("{arrow} "), &palette.aurora_mid, color_enabled);
    let name = colorize(&tool.name, &palette.secondary, color_enabled);
    format!("{icon}{name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::widgets::test_support::aurora_marker_palette;

    fn t(name: &str) -> ToolSummary {
        ToolSummary {
            id: format!("id-{name}"),
            name: name.to_string(),
            target: None,
        }
    }

    #[test]
    fn empty_tools_renders_empty_string() {
        let p = aurora_marker_palette();
        assert_eq!(render(&[], 5, GlyphMode::Icon, &p, false), "");
    }

    #[test]
    fn max_items_zero_renders_empty_string() {
        let p = aurora_marker_palette();
        assert_eq!(render(&[t("Read")], 0, GlyphMode::Icon, &p, false), "");
    }

    #[test]
    fn most_recent_appear_at_right_edge() {
        // tools[0] is oldest; tape with max_items=2 should show the two
        // newest (Edit, Bash), in chronological order.
        let p = aurora_marker_palette();
        let tools = vec![t("Read"), t("Grep"), t("Edit"), t("Bash")];
        let s = render(&tools, 2, GlyphMode::Icon, &p, false);
        assert!(s.contains("Edit"));
        assert!(s.contains("Bash"));
        assert!(!s.contains("Read"));
        assert!(!s.contains("Grep"));
        // Edit should appear before Bash.
        let edit_pos = s.find("Edit").unwrap();
        let bash_pos = s.find("Bash").unwrap();
        assert!(edit_pos < bash_pos);
    }

    #[test]
    fn separator_between_items() {
        let p = aurora_marker_palette();
        let tools = vec![t("A"), t("B"), t("C")];
        let s = render(&tools, 5, GlyphMode::Icon, &p, false);
        // Two separators for three items.
        assert_eq!(s.matches(SEP).count(), 2);
    }

    #[test]
    fn single_tool_has_no_separator() {
        let p = aurora_marker_palette();
        let s = render(&[t("Bash")], 5, GlyphMode::Icon, &p, false);
        assert!(!s.contains(SEP));
        assert!(s.contains("Bash"));
        assert!(s.contains(ICON_RUNNING));
    }

    #[test]
    fn color_uses_aurora_mid_for_icon() {
        let mut p = aurora_marker_palette();
        p.secondary = "SEC".to_string();
        let s = render(&[t("Read")], 5, GlyphMode::Icon, &p, true);
        assert!(s.contains("MID"));
        assert!(s.contains("SEC"));
    }

    #[test]
    fn ascii_mode_uses_gt_arrow_not_unicode_triangle() {
        let p = aurora_marker_palette();
        let s = render(&[t("Bash")], 5, GlyphMode::Ascii, &p, false);
        assert!(s.contains(ASCII_RUNNING), "expected '>' in {s:?}");
        assert!(
            !s.contains(ICON_RUNNING),
            "did not expect U+25B6 (▶) in {s:?}"
        );
        assert!(s.contains("Bash"));
    }

    #[test]
    fn ascii_mode_keeps_middle_dot_separator() {
        // The mid-dot separator (U+00B7 ·) has broad font coverage even on
        // plain-text terminals, so we keep it under Ascii mode rather than
        // collapsing to spaces.
        let p = aurora_marker_palette();
        let s = render(&[t("Read"), t("Bash")], 5, GlyphMode::Ascii, &p, false);
        assert_eq!(s.matches(SEP).count(), 1);
    }
}
