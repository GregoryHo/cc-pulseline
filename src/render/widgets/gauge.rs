//! Bracket-framed gauge — `[████▎      ]` style, like a car/electronic
//! device battery indicator.
//!
//! Icon mode uses `█` (U+2588) for filled cells, `▏▎▍▌▋▊▉` for the partial
//! cell at the fill boundary, and a literal space (U+0020) for empty cells.
//! The whole inside is wrapped in `[` `]` brackets in `palette.structural`
//! tone so the eye reads "the area inside the brackets is the gauge; the
//! coloured portion shows usage; the rest is unused capacity".
//!
//! Ascii mode uses `'#'` for filled and `'-'` for empty (more visible than
//! a bare space on monochrome terminals where the brackets alone may not
//! pop), still wrapped in `[ ]`.
//!
//! Why literal whitespace for empty (Icon mode):
//! - Truly composable across themes — empty visually disappears regardless
//!   of which fill color is active, instead of competing with it.
//! - Brackets carry the "this is a gauge bounding box" semantics so the
//!   empty portion doesn't get confused with row whitespace.
//! - Mirrors a physical battery indicator: filled segments + visually
//!   absent empty segments inside a frame.
//!
//! Color is threshold-based:
//!   - pct < 55           → `aurora_mid` (calm, default)
//!   - 55 <= pct < 70     → `active_amber` (warning)
//!   - pct >= 70          → `alert_red`   (critical)

use crate::config::GlyphMode;
use crate::render::color::{colorize, ThemePalette};

/// Eighths blocks from 1/8 (▏) to 8/8 (█). Indexed 1..=8. Index 0 is space
/// (used implicitly when no partial cell is rendered).
const EIGHTHS: [char; 9] = [
    ' ', '\u{258F}', '\u{258E}', '\u{258D}', '\u{258C}', '\u{258B}', '\u{258A}', '\u{2589}',
    '\u{2588}',
];
/// Empty cell glyph in Icon mode — literal space. Inside the `[ ]` frame
/// this reads as "unused capacity"; the bracket bounds the gauge so the
/// space doesn't visually merge with surrounding row whitespace.
const EMPTY_CELL_ICON: char = ' ';
const FILLED_CELL_ASCII: char = '#';
const EMPTY_CELL_ASCII: char = '-';
/// Frame characters that bracket the gauge interior. Identical in both
/// glyph modes — square brackets render cleanly in any terminal font.
const FRAME_LEFT: &str = "[";
const FRAME_RIGHT: &str = "]";

/// Render a gauge of `width` interior cells at `pct` (0..=100). The visible
/// width is `width + 2` (the bracket frame adds one cell on each side).
/// Width 0 returns an empty string (no brackets either).
///
/// Glyph set:
/// - `GlyphMode::Icon`  → `[████▎      ]` — 1/8 sub-cell block precision
///   for filled, literal space for empty, brackets around the interior.
/// - `GlyphMode::Ascii` → `[####------]` — `#` filled, `-` empty, same
///   brackets.
pub fn render(
    pct: u64,
    width: usize,
    mode: GlyphMode,
    fill_color: &str,
    palette: &ThemePalette,
    color_enabled: bool,
) -> String {
    if width == 0 {
        return String::new();
    }
    let raw = render_glyphs(pct, width, mode);
    if !color_enabled {
        return format!("{FRAME_LEFT}{raw}{FRAME_RIGHT}");
    }
    // Split the interior into filled (full + optional partial) and empty
    // halves so each gets its own colour. The caller supplies the fill
    // tone (so CTX uses `color_for_ctx_pct` and quota uses
    // `color_for_quota_pct`); the empty half is literal whitespace
    // (Icon) or dashes (Ascii) and reuses the structural frame tone.
    let pct_clamped = pct.min(100);
    let total_eighths = (width as u64 * 8 * pct_clamped) / 100;
    let full_cells = (total_eighths / 8) as usize;
    let has_partial = (total_eighths % 8) > 0;
    let filled_count = (full_cells + if has_partial { 1 } else { 0 }).min(width);

    let filled: String = raw.chars().take(filled_count).collect();
    let empty: String = raw.chars().skip(filled_count).collect();
    let frame_color = &palette.structural;

    let mut out = String::with_capacity(filled.len() + empty.len() + 32);
    out.push_str(&colorize(FRAME_LEFT, frame_color, true));
    if !filled.is_empty() {
        out.push_str(&colorize(&filled, fill_color, true));
    }
    if !empty.is_empty() {
        // Empty interior is whitespace / dashes — colourise with the frame
        // tone so any rendered character (Ascii `-`) reads as "frame
        // continuation" rather than competing with the fill.
        out.push_str(&colorize(&empty, frame_color, true));
    }
    out.push_str(&colorize(FRAME_RIGHT, frame_color, true));
    out
}

/// Plain glyph rendering — used by tests, by the `Console` quota gauge that
/// composes its own colors, and by sub-bar variants.
pub fn render_glyphs(pct: u64, width: usize, mode: GlyphMode) -> String {
    if width == 0 {
        return String::new();
    }
    match mode {
        GlyphMode::Icon => render_block_glyphs(pct, width),
        GlyphMode::Ascii => render_hash_glyphs(pct, width),
    }
}

fn render_block_glyphs(pct: u64, width: usize) -> String {
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
            out.push(EMPTY_CELL_ICON);
        }
    }
    out
}

fn render_hash_glyphs(pct: u64, width: usize) -> String {
    let pct_clamped = pct.min(100);
    // Floor to whole cells — no fractional precision in Ascii mode. Round
    // half-up so a 50%/8 reads as 4 filled cells (matches Icon's perception
    // at the same threshold).
    let filled = ((width as u64 * pct_clamped + 50) / 100) as usize;
    let mut out = String::with_capacity(width);
    for cell_idx in 0..width {
        if cell_idx < filled {
            out.push(FILLED_CELL_ASCII);
        } else {
            out.push(EMPTY_CELL_ASCII);
        }
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::widgets::test_support::aurora_marker_palette;

    #[test]
    fn zero_pct_renders_all_spaces_inside() {
        // Interior at 0% is literally whitespace — the bracket frame on
        // either side is what makes the gauge visually exist.
        let s = render_glyphs(0, 8, GlyphMode::Icon);
        assert_eq!(s.chars().count(), 8);
        assert!(s.chars().all(|c| c == EMPTY_CELL_ICON));
        assert_eq!(EMPTY_CELL_ICON, ' ');
    }

    #[test]
    fn full_pct_renders_all_full_blocks() {
        let s = render_glyphs(100, 8, GlyphMode::Icon);
        assert_eq!(s.chars().count(), 8);
        assert!(s.chars().all(|c| c == '\u{2588}'));
    }

    #[test]
    fn half_pct_renders_half_full_half_empty() {
        let s = render_glyphs(50, 8, GlyphMode::Icon);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 8);
        // 50% of 8 cells = 4 full cells, no partial, 4 empty (= space).
        assert_eq!(chars[..4], ['\u{2588}'; 4]);
        for c in &chars[4..] {
            assert_eq!(*c, EMPTY_CELL_ICON);
        }
    }

    #[test]
    fn partial_eighth_picks_correct_block() {
        // 1/16 of total = ~6.25%. With width=2 → 16 eighths; 1 eighth filled.
        let s = render_glyphs(7, 2, GlyphMode::Icon);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 2);
        // 7% of 16 eighths = 1.12 → floor 1 → ▏ (partial sliver in fill)
        assert_eq!(chars[0], '\u{258F}');
        // chars[1] is the empty interior cell — a literal space.
        assert_eq!(chars[1], EMPTY_CELL_ICON);
    }

    #[test]
    fn over_100_clamps_to_full() {
        let s = render_glyphs(255, 4, GlyphMode::Icon);
        assert_eq!(s.chars().count(), 4);
        assert!(s.chars().all(|c| c == '\u{2588}'));
    }

    #[test]
    fn width_zero_renders_empty_string() {
        // No interior → no brackets either.
        assert_eq!(render_glyphs(50, 0, GlyphMode::Icon), "");
        assert_eq!(
            render(50, 0, GlyphMode::Icon, "FILL", &aurora_marker_palette(), true),
            ""
        );
    }

    #[test]
    fn render_uses_caller_supplied_fill_color() {
        // Threshold logic now lives at the call site (palette's
        // `color_for_ctx_pct` / `color_for_quota_pct`); the gauge just
        // applies whatever colour the caller passes to the filled cells.
        let p = aurora_marker_palette();
        assert!(render(40, 4, GlyphMode::Icon, "GREEN", &p, true).contains("GREEN"));
        assert!(render(60, 4, GlyphMode::Icon, "AMBER", &p, true).contains("AMBER"));
        assert!(render(80, 4, GlyphMode::Icon, "RED", &p, true).contains("RED"));
    }

    #[test]
    fn render_wraps_interior_in_brackets() {
        let p = aurora_marker_palette();
        let s = render(50, 4, GlyphMode::Icon, "X", &p, false);
        assert!(s.starts_with('['), "missing opening bracket: {s:?}");
        assert!(s.ends_with(']'), "missing closing bracket: {s:?}");
        // 4 interior cells + 2 brackets = 6 visible chars.
        assert_eq!(s.chars().count(), 6);
    }

    #[test]
    fn ascii_mode_uses_hash_and_dash_only() {
        let s = render_glyphs(50, 8, GlyphMode::Ascii);
        assert_eq!(s.chars().count(), 8);
        assert!(
            s.chars()
                .all(|c| c == FILLED_CELL_ASCII || c == EMPTY_CELL_ASCII),
            "expected only `#` and `-` in {s:?}"
        );
        // 50% of 8 cells = 4 filled, 4 empty.
        let filled = s.chars().filter(|c| *c == FILLED_CELL_ASCII).count();
        let empty = s.chars().filter(|c| *c == EMPTY_CELL_ASCII).count();
        assert_eq!(filled, 4);
        assert_eq!(empty, 4);
    }

    #[test]
    fn ascii_mode_zero_pct_is_all_dashes() {
        let s = render_glyphs(0, 6, GlyphMode::Ascii);
        assert_eq!(s, "------");
    }

    #[test]
    fn ascii_mode_full_pct_is_all_hashes() {
        let s = render_glyphs(100, 6, GlyphMode::Ascii);
        assert_eq!(s, "######");
    }

    #[test]
    fn empty_interior_does_not_carry_fill_color() {
        // 0% → no filled portion → caller-supplied fill colour never
        // makes it into the rendered string.
        let p = aurora_marker_palette();
        let zero = render(0, 8, GlyphMode::Icon, "FILL", &p, true);
        assert!(!zero.contains("FILL"));
    }

    #[test]
    fn ascii_mode_emits_no_unicode_block_chars() {
        // Catch the original Phase 2 bug: gauge emitted U+2588 etc. even in
        // ascii mode. Now the contract: zero block chars under Ascii.
        // Brackets `[` `]` are ASCII (U+005B/005D) so they don't trigger.
        const BLOCKS: &[char] = &[
            '\u{2588}', '\u{2589}', '\u{258A}', '\u{258B}', '\u{258C}', '\u{258D}', '\u{258E}',
            '\u{258F}', '\u{2591}',
        ];
        for pct in [0, 25, 50, 75, 100] {
            let s = render(pct, 10, GlyphMode::Ascii, "X", &aurora_marker_palette(), false);
            assert!(
                !s.chars().any(|c| BLOCKS.contains(&c)),
                "pct={pct} produced a block char: {s:?}"
            );
        }
    }
}
