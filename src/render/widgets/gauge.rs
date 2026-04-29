//! Marks-on-track gauge — `▰▰▰▰▰▰···──·──` style. Bracketless.
//!
//! Filled cells use `▰` (U+25B0) in the caller-supplied fill colour.
//! Empty cells use `─` (U+2500) in `palette.structural`. Threshold
//! marks fall on the empty portion as `·` (U+00B7) — also `structural`.
//! Marks landing on a filled cell are intentionally hidden by fill: the
//! bar reads "I've crossed this threshold" by the absence of the mark
//! more than by colouring.
//!
//! Ascii mode swaps to plain punctuation: `=` filled / `-` empty /
//! `:` mark.
//!
//! Caller owns thresholds — quota passes `&[50, 85]`, CTX passes the
//! window-aware result of `palette.ctx_marks_for_window(...)`. Cost
//! and other future segments can supply their own. The widget is
//! purely a renderer.
//!
//! Why bracketless: `[` `]` ASCII brackets read as terminal-shell
//! decoration; bracketless `▰─·` reads as one designed track. The
//! frame chrome (`╭─╮│╰─╯`) and field separator `|` already provide
//! enough containment in framed layouts.

use crate::config::GlyphMode;
use crate::render::color::{colorize, ThemePalette};

const FILLED_ICON: char = '\u{25B0}'; // ▰
const EMPTY_ICON: char = '\u{2500}'; // ─
const MARK_ICON: char = '\u{00B7}'; // ·

const FILLED_ASCII: char = '=';
const EMPTY_ASCII: char = '-';
const MARK_ASCII: char = ':';

/// Render a horizontal bar with optional threshold marks.
///
/// `pct`: 0..=100 (clamped). Cells filled = round(pct/100 * width).
/// `width`: cell count; visible width is `width` (no bracket).
/// `marks`: threshold percentages (each clamped to 0..=100). A mark
///          falling on a filled cell is hidden — fill takes precedence.
/// `fill_color`: caller picks via `color_for_ctx_pct` /
///               `color_for_quota_pct` / etc.
///
/// Width 0 returns an empty string. Empty `marks` slice produces a
/// plain bar (no marks at all) — useful when the caller doesn't have
/// thresholds to express.
pub fn render(
    pct: u64,
    width: usize,
    marks: &[u64],
    fill_color: &str,
    palette: &ThemePalette,
    mode: GlyphMode,
    color_enabled: bool,
) -> String {
    if width == 0 {
        return String::new();
    }
    let pct_clamped = pct.min(100);
    // Round-half-up: ((w * pct) + 50) / 100. At pct=50 width=14 this
    // gives 7 (cells 0..6 filled); at pct=54 still 7; at pct=57 → 8.
    let filled_count = (((width as u64) * pct_clamped + 50) / 100) as usize;
    let filled_count = filled_count.min(width);

    let (filled_ch, empty_ch, mark_ch) = match mode {
        GlyphMode::Icon => (FILLED_ICON, EMPTY_ICON, MARK_ICON),
        GlyphMode::Ascii => (FILLED_ASCII, EMPTY_ASCII, MARK_ASCII),
    };

    // Compute mark cell positions once. Same round-half-up rule so
    // `marks=[50, 85]` and `width=14` lands on cells 7 and 12.
    let mark_cells: Vec<usize> = marks
        .iter()
        .map(|m| {
            let m_clamped = (*m).min(100);
            (((width as u64) * m_clamped + 50) / 100) as usize
        })
        .filter(|c| *c < width)
        .collect();

    // Split into filled / empty buffers so each gets its own colour
    // wrapper. (No mid-bar colour change beyond the structural marks.)
    let mut filled_buf = String::with_capacity(filled_count * 4);
    let mut empty_buf = String::with_capacity((width - filled_count) * 4);

    for i in 0..width {
        if i < filled_count {
            filled_buf.push(filled_ch);
        } else if mark_cells.contains(&i) {
            empty_buf.push(mark_ch);
        } else {
            empty_buf.push(empty_ch);
        }
    }

    if !color_enabled {
        return format!("{filled_buf}{empty_buf}");
    }

    let mut out = String::with_capacity(filled_buf.len() + empty_buf.len() + 32);
    if !filled_buf.is_empty() {
        out.push_str(&colorize(&filled_buf, fill_color, true));
    }
    if !empty_buf.is_empty() {
        out.push_str(&colorize(&empty_buf, &palette.structural, true));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::widgets::test_support::aurora_marker_palette;

    fn p() -> ThemePalette {
        aurora_marker_palette()
    }

    #[test]
    fn zero_pct_renders_empty_track_with_marks_visible() {
        let s = render(0, 14, &[50, 85], "FILL", &p(), GlyphMode::Icon, false);
        assert_eq!(s.chars().count(), 14);
        let marks: Vec<usize> = s
            .chars()
            .enumerate()
            .filter_map(|(i, c)| if c == MARK_ICON { Some(i) } else { None })
            .collect();
        // 50% mark at cell 7, 85% mark at cell 12 (both visible since
        // nothing is filled).
        assert_eq!(marks, vec![7, 12], "rendered: {s:?}");
        assert_eq!(s.chars().filter(|c| *c == EMPTY_ICON).count(), 12);
        assert_eq!(s.chars().filter(|c| *c == FILLED_ICON).count(), 0);
    }

    #[test]
    fn at_50_buries_first_mark_keeps_second() {
        // 50%/14 → 7 cells filled (0..6). Cell 7 is the first mark
        // position — visible because filled_count is 7, cell 7 is *not*
        // filled. Mark at cell 12 is also visible.
        let s = render(50, 14, &[50, 85], "FILL", &p(), GlyphMode::Icon, false);
        assert_eq!(s.chars().filter(|c| *c == FILLED_ICON).count(), 7);
        let marks: Vec<usize> = s
            .chars()
            .enumerate()
            .filter_map(|(i, c)| if c == MARK_ICON { Some(i) } else { None })
            .collect();
        // At exactly 50%, both marks still visible. Crossing happens at ~54.
        assert_eq!(marks, vec![7, 12]);
    }

    #[test]
    fn at_57_buries_first_mark() {
        // 57%/14 → 8 cells filled (0..7). Cell 7 is now filled — mark
        // buried.
        let s = render(57, 14, &[50, 85], "FILL", &p(), GlyphMode::Icon, false);
        assert_eq!(s.chars().filter(|c| *c == FILLED_ICON).count(), 8);
        let marks: Vec<usize> = s
            .chars()
            .enumerate()
            .filter_map(|(i, c)| if c == MARK_ICON { Some(i) } else { None })
            .collect();
        // 50% mark gone (cell 7 buried under fill); 85% mark still visible at cell 12.
        assert_eq!(marks, vec![12]);
    }

    #[test]
    fn at_85_keeps_second_mark_at_boundary() {
        // 85%/14 → 12 cells filled (0..11). Cell 12 is the 85% mark
        // position — visible because filled_count is 12, cell 12 is
        // *not* filled. Mark crossed at ~89%.
        let s = render(85, 14, &[50, 85], "FILL", &p(), GlyphMode::Icon, false);
        assert_eq!(s.chars().filter(|c| *c == FILLED_ICON).count(), 12);
        let marks: Vec<usize> = s
            .chars()
            .enumerate()
            .filter_map(|(i, c)| if c == MARK_ICON { Some(i) } else { None })
            .collect();
        // 50% mark gone; 85% mark visible at cell 12.
        assert_eq!(marks, vec![12]);
    }

    #[test]
    fn at_100_buries_both_marks() {
        let s = render(100, 14, &[50, 85], "FILL", &p(), GlyphMode::Icon, false);
        assert_eq!(s.chars().filter(|c| *c == FILLED_ICON).count(), 14);
        assert_eq!(s.chars().filter(|c| *c == MARK_ICON).count(), 0);
    }

    #[test]
    fn empty_marks_slice_produces_plain_bar() {
        // No marks supplied → no `·` characters; just fill + empty.
        let s = render(40, 10, &[], "FILL", &p(), GlyphMode::Icon, false);
        assert_eq!(s.chars().count(), 10);
        assert_eq!(s.chars().filter(|c| *c == MARK_ICON).count(), 0);
        // 40% of 10 cells = 4 filled.
        assert_eq!(s.chars().filter(|c| *c == FILLED_ICON).count(), 4);
        assert_eq!(s.chars().filter(|c| *c == EMPTY_ICON).count(), 6);
    }

    #[test]
    fn over_100_clamps_to_full() {
        let s = render(255, 8, &[50], "FILL", &p(), GlyphMode::Icon, false);
        assert_eq!(s.chars().count(), 8);
        assert!(s.chars().all(|c| c == FILLED_ICON));
    }

    #[test]
    fn marks_outside_0_to_100_are_clamped() {
        // Marks at 200 should clamp to 100, which lands at cell `width`
        // (out of bounds) and gets filtered out — no mark rendered.
        let s = render(0, 10, &[200], "FILL", &p(), GlyphMode::Icon, false);
        assert_eq!(s.chars().filter(|c| *c == MARK_ICON).count(), 0);
    }

    #[test]
    fn width_zero_renders_empty_string() {
        let s = render(50, 0, &[50, 85], "FILL", &p(), GlyphMode::Icon, false);
        assert_eq!(s, "");
    }

    #[test]
    fn ascii_mode_swaps_to_punctuation_ladder() {
        let s = render(50, 14, &[50, 85], "FILL", &p(), GlyphMode::Ascii, false);
        assert_eq!(s.chars().count(), 14);
        // Only `=`, `-`, `:` should appear.
        for c in s.chars() {
            assert!(
                c == FILLED_ASCII || c == EMPTY_ASCII || c == MARK_ASCII,
                "unexpected ascii char {c:?} in {s:?}"
            );
        }
        // 7 filled + marks + dashes = 14
        assert_eq!(s.chars().filter(|c| *c == FILLED_ASCII).count(), 7);
        // Marks at cell 7 and 12 → 2 colons
        assert_eq!(s.chars().filter(|c| *c == MARK_ASCII).count(), 2);
    }

    #[test]
    fn ascii_mode_emits_no_unicode_block_chars() {
        // Catch-net: the new gauge must not emit U+2588 family even at
        // various pct values under Ascii. (Old gauge had a bug that
        // emitted blocks under Ascii — this is the regression guard.)
        const BLOCKS: &[char] = &[
            '\u{2588}', '\u{2589}', '\u{258A}', '\u{258B}', '\u{258C}', '\u{258D}', '\u{258E}',
            '\u{258F}', '\u{2591}',
        ];
        for pct in [0, 25, 50, 75, 100] {
            let s = render(pct, 14, &[50, 85], "FILL", &p(), GlyphMode::Ascii, false);
            assert!(
                !s.chars().any(|c| BLOCKS.contains(&c)),
                "pct={pct} produced a block char: {s:?}"
            );
        }
    }

    #[test]
    fn render_uses_caller_supplied_fill_color() {
        let s = render(60, 8, &[50], "AMBER", &p(), GlyphMode::Icon, true);
        assert!(s.contains("AMBER"));
    }

    #[test]
    fn empty_track_does_not_carry_fill_color() {
        // 0% → caller-supplied fill never enters the rendered string.
        let s = render(0, 8, &[50], "FILL", &p(), GlyphMode::Icon, true);
        assert!(!s.contains("FILL"));
    }

    #[test]
    fn disabled_color_returns_no_ansi_escapes() {
        let s = render(50, 14, &[50, 85], "FILL", &p(), GlyphMode::Icon, false);
        assert!(!s.contains("\x1b["), "found ANSI escape: {s:?}");
    }

    #[test]
    fn ctx_threshold_marks_at_55_70_in_14_cell_bar() {
        // Verify that the standard CTX threshold marks land at the
        // correct cells.  55%/14 → 8 (cell 8); 70%/14 → 10 (cell 10).
        let s = render(0, 14, &[55, 70], "FILL", &p(), GlyphMode::Icon, false);
        let marks: Vec<usize> = s
            .chars()
            .enumerate()
            .filter_map(|(i, c)| if c == MARK_ICON { Some(i) } else { None })
            .collect();
        assert_eq!(marks, vec![8, 10]);
    }
}
