//! Block-bar gauge with 1/8 sub-cell precision.
//!
//! Renders a `width`-cell gauge filled to `pct` (0..=100), using the Unicode
//! eighth-blocks (U+2588..=U+258F) for fractional cells. Below the fill, the
//! gauge uses U+2591 (light shade) for visible empty cells.
//!
//! Color is threshold-based:
//!   - pct < 55           → `aurora_mid` (calm, default)
//!   - 55 <= pct < 70     → `active_amber` (warning)
//!   - pct >= 70          → `alert_red`   (critical)
//!
//! Empty cells are dimmed via `structural`. The whole bar is colored with one
//! foreground call so the gradient stays clean — no per-cell color flips.

use crate::render::color::{colorize, ThemePalette};

/// Eighths blocks from 1/8 (▏) to 8/8 (█). Indexed 1..=8.
const EIGHTHS: [char; 9] = [
    ' ', '\u{258F}', '\u{258E}', '\u{258D}', '\u{258C}', '\u{258B}', '\u{258A}', '\u{2589}',
    '\u{2588}',
];
const EMPTY_CELL: char = '\u{2591}';

/// Render a gauge of `width` cells at `pct` (0..=100). Returns ANSI-wrapped
/// string when `color_enabled`. Width 0 returns an empty string.
pub fn render(pct: u64, width: usize, palette: &ThemePalette, color_enabled: bool) -> String {
    let raw = render_glyphs(pct, width);
    if width == 0 {
        return String::new();
    }
    let fill_color = color_for_pct(pct, palette);
    if color_enabled {
        // The empty cells are visually dim already (light shade); we color the
        // whole bar with the fill tone so the eye reads it as one instrument.
        colorize(&raw, fill_color, true)
    } else {
        raw
    }
}

/// Plain glyph rendering — used by tests, by the `Console` quota gauge that
/// composes its own colors, and by sub-bar variants.
pub fn render_glyphs(pct: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let pct_clamped = pct.min(100);
    // Total eighths to fill across all cells.
    let total_eighths = (width as u64 * 8 * pct_clamped) / 100;
    let full_cells = (total_eighths / 8) as usize;
    let partial_eighths = (total_eighths % 8) as usize;

    let mut out = String::with_capacity(width * 3);
    for cell_idx in 0..width {
        if cell_idx < full_cells {
            out.push(EIGHTHS[8]);
        } else if cell_idx == full_cells && partial_eighths > 0 {
            out.push(EIGHTHS[partial_eighths]);
        } else {
            out.push(EMPTY_CELL);
        }
    }
    out
}

fn color_for_pct(pct: u64, palette: &ThemePalette) -> &str {
    if pct >= 70 {
        &palette.alert_red
    } else if pct >= 55 {
        &palette.active_amber
    } else {
        &palette.aurora_mid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::widgets::test_support::aurora_marker_palette;

    #[test]
    fn zero_pct_renders_all_empty_cells() {
        let s = render_glyphs(0, 8);
        assert_eq!(s.chars().count(), 8);
        assert!(s.chars().all(|c| c == EMPTY_CELL));
    }

    #[test]
    fn full_pct_renders_all_full_blocks() {
        let s = render_glyphs(100, 8);
        assert_eq!(s.chars().count(), 8);
        assert!(s.chars().all(|c| c == '\u{2588}'));
    }

    #[test]
    fn half_pct_renders_half_full_half_empty() {
        let s = render_glyphs(50, 8);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 8);
        // 50% of 8 cells = 4 full cells, no partial, 4 empty
        assert_eq!(chars[..4], ['\u{2588}'; 4]);
        for c in &chars[4..] {
            assert_eq!(*c, EMPTY_CELL);
        }
    }

    #[test]
    fn partial_eighth_picks_correct_block() {
        // 1/16 of total = ~6.25%. With width=2 → 16 eighths; 1 eighth filled.
        let s = render_glyphs(7, 2);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 2);
        // 7% of 16 eighths = 1.12 → floor 1 → ▏
        assert_eq!(chars[0], '\u{258F}');
        assert_eq!(chars[1], EMPTY_CELL);
    }

    #[test]
    fn over_100_clamps_to_full() {
        let s = render_glyphs(255, 4);
        assert_eq!(s.chars().count(), 4);
        assert!(s.chars().all(|c| c == '\u{2588}'));
    }

    #[test]
    fn width_zero_renders_empty_string() {
        assert_eq!(render_glyphs(50, 0), "");
        assert_eq!(render(50, 0, &aurora_marker_palette(), true), "");
    }

    #[test]
    fn render_picks_color_by_threshold() {
        let p = aurora_marker_palette();
        let mut pal = p.clone();
        pal.active_amber = "AMBER".to_string();
        pal.alert_red = "RED".to_string();

        assert!(render(40, 4, &pal, true).contains("MID"));
        assert!(render(60, 4, &pal, true).contains("AMBER"));
        assert!(render(80, 4, &pal, true).contains("RED"));
    }
}
