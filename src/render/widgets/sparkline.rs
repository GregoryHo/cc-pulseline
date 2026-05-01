//! 6-cell braille sparkline.
//!
//! Each braille cell encodes 2 columns × 4 rows of dots (8 dots total). For a
//! sparkline we use *two adjacent samples per cell* — left column = sample N,
//! right column = sample N+1 — so 6 cells render up to 12 samples. Dot height
//! is the value mapped to 0..=4 (dotless cell uses U+2800).
//!
//! Braille pattern bits (per Unicode block 2800-28FF):
//! ```text
//!   col-L:  bit0=⠁ (top)   bit1=⠂   bit2=⠄   bit6=⠀ row-bottom = ⡀
//!   col-R:  bit3=⠈         bit4=⠐   bit5=⠠   bit7=⢀
//! ```
//! Dot height H ∈ 0..=4 produces a column mask:
//!   - H=0 → 0x00 (no dots)
//!   - H=1 → row-bottom only
//!   - H=2 → bottom + above-bottom
//!   - H=3 → bottom + above + middle
//!   - H=4 → all four rows on that column
//!
//! Width is fixed at 6 cells. If `samples.len() < 12` the sparkline is
//! left-padded with empty cells (pattern starts toward the right edge), which
//! reads naturally as "history is filling up."

use crate::config::GlyphMode;
use crate::render::color::{colorize, ThemePalette};

const SPARK_CELLS: usize = 6;
const SAMPLES_PER_CELL: usize = 2;
const SPARK_SAMPLES: usize = SPARK_CELLS * SAMPLES_PER_CELL; // 12
const EMPTY_CELL: char = '\u{2800}';

/// Per-row bit masks for braille columns L and R (4 rows top→bottom).
/// Indexed by row position 0..=3 from top.
const COL_L_BITS: [u8; 4] = [0x01, 0x02, 0x04, 0x40];
const COL_R_BITS: [u8; 4] = [0x08, 0x10, 0x20, 0x80];

/// Build a single braille cell from two column heights (0..=4).
fn braille_cell(left_h: u8, right_h: u8) -> char {
    let l = left_h.min(4) as usize;
    let r = right_h.min(4) as usize;
    let mut bits: u8 = 0;
    // Light dots from the bottom up: H=1 → bottom row only, H=4 → all rows.
    for row_idx in 0..l {
        // row_idx counts from the bottom, so map to top-indexed row.
        let top_idx = 3 - row_idx;
        bits |= COL_L_BITS[top_idx];
    }
    for row_idx in 0..r {
        let top_idx = 3 - row_idx;
        bits |= COL_R_BITS[top_idx];
    }
    char::from_u32(0x2800 + bits as u32).unwrap_or(EMPTY_CELL)
}

/// Map a 0..=100 sample to a 0..=4 dot height. 0 stays 0; 1..=25 → 1; etc.
fn height_for(sample: u8) -> u8 {
    let s = sample.min(100) as u32;
    if s == 0 {
        0
    } else {
        // 4 buckets of width 25; ceiling so a 1% blip lights the floor row.
        ((s - 1) / 25 + 1).min(4) as u8
    }
}

/// Render a 6-cell braille sparkline from up to 12 most-recent samples
/// (each `(pct, epoch_ms)` — only `pct` drives the curve shape).
///
/// Renders all-`⠀` (empty) when `samples` is empty so layouts can include the
/// widget unconditionally without leaking width.
///
/// **Icon-only widget.** Braille has no ASCII equivalent that conveys the same
/// trend information at this density, so under `GlyphMode::Ascii` this fn
/// returns an empty string.
///
/// Color: caller-supplied. The shape (braille curve) carries direction;
/// color carries an orthogonal signal — typically velocity-based aurora
/// from `aurora_for_velocity`. An empty `fill_color` skips ANSI wrapping.
pub fn render(
    samples: &[(u8, u64)],
    fill_color: &str,
    mode: GlyphMode,
    color_enabled: bool,
) -> String {
    if matches!(mode, GlyphMode::Ascii) {
        return String::new();
    }
    let raw = render_glyphs_timed(samples);
    if !color_enabled || fill_color.is_empty() {
        return raw;
    }
    colorize(&raw, fill_color, color_enabled)
}

/// Pick the aurora fill color for a sparkline based on the *velocity* of
/// CTX consumption across `window`. Shape carries direction (rise/fall);
/// this color carries intensity (calm / active / hot).
///
/// ```text
/// velocity (% / minute)   → color
/// < 1                     → aurora_low
/// 1 .. 5                  → aurora_mid
/// >= 5                    → aurora_high
/// ```
///
/// Thresholds verified against a 134-min real session — see
/// `designs/console-redesign/palette-integration.md` § Aurora revisited.
pub fn aurora_for_velocity<'p>(window: &[(u8, u64)], p: &'p ThemePalette) -> &'p str {
    let (Some(first), Some(last)) = (window.first(), window.last()) else {
        return &p.aurora_low;
    };
    let span_ms = last.1.saturating_sub(first.1);
    if span_ms == 0 {
        return &p.aurora_low;
    }
    let span_min = (span_ms as f64) / 60_000.0;
    let velocity = (last.0 as f64 - first.0 as f64).abs() / span_min;
    if velocity >= 5.0 {
        &p.aurora_high
    } else if velocity >= 1.0 {
        &p.aurora_mid
    } else {
        &p.aurora_low
    }
}

fn render_glyphs_timed(samples: &[(u8, u64)]) -> String {
    if samples.is_empty() {
        return EMPTY_CELL.to_string().repeat(SPARK_CELLS);
    }
    let take_from = samples.len().saturating_sub(SPARK_SAMPLES);
    let recent = &samples[take_from..];
    let pad = SPARK_SAMPLES - recent.len();

    let mut out = String::with_capacity(SPARK_CELLS * 3);
    for cell_idx in 0..SPARK_CELLS {
        let s0_pos = cell_idx * SAMPLES_PER_CELL;
        let s1_pos = s0_pos + 1;
        let l = if s0_pos < pad {
            0
        } else {
            height_for(recent[s0_pos - pad].0)
        };
        let r = if s1_pos < pad {
            0
        } else {
            height_for(recent[s1_pos - pad].0)
        };
        out.push(braille_cell(l, r));
    }
    out
}

/// Like `render` but returns plain glyphs (no ANSI). Used by tests and by
/// widget composers that need the raw width.
pub fn render_glyphs(samples: &[u8]) -> String {
    if samples.is_empty() {
        return EMPTY_CELL.to_string().repeat(SPARK_CELLS);
    }
    // Take the most recent SPARK_SAMPLES samples; left-pad with zeros if short.
    let take_from = samples.len().saturating_sub(SPARK_SAMPLES);
    let recent = &samples[take_from..];
    let pad = SPARK_SAMPLES - recent.len();

    let mut out = String::with_capacity(SPARK_CELLS * 3);
    for cell_idx in 0..SPARK_CELLS {
        let s0_pos = cell_idx * SAMPLES_PER_CELL;
        let s1_pos = s0_pos + 1;
        let l = if s0_pos < pad {
            0
        } else {
            height_for(recent[s0_pos - pad])
        };
        let r = if s1_pos < pad {
            0
        } else {
            height_for(recent[s1_pos - pad])
        };
        out.push(braille_cell(l, r));
    }
    out
}

/// Visible width of the rendered sparkline, in monospace cells.
pub const fn width() -> usize {
    SPARK_CELLS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_renders_six_blank_cells() {
        let s = render_glyphs(&[]);
        assert_eq!(s.chars().count(), 6);
        assert!(s.chars().all(|c| c == EMPTY_CELL));
    }

    #[test]
    fn short_history_is_right_aligned() {
        // Two samples should land in the rightmost cell only.
        let s = render_glyphs(&[50, 100]);
        let cells: Vec<char> = s.chars().collect();
        assert_eq!(cells.len(), 6);
        // First five cells empty, last cell has data.
        for c in &cells[..5] {
            assert_eq!(*c, EMPTY_CELL, "leading cells should be blank");
        }
        assert_ne!(cells[5], EMPTY_CELL, "rightmost cell should be lit");
    }

    #[test]
    fn full_history_lights_all_cells() {
        let samples: Vec<u8> = (0..12).map(|i| (i * 9 + 5) as u8).collect();
        let s = render_glyphs(&samples);
        assert_eq!(s.chars().count(), 6);
        assert!(s.chars().all(|c| c != EMPTY_CELL));
    }

    #[test]
    fn height_buckets_map_correctly() {
        assert_eq!(height_for(0), 0);
        assert_eq!(height_for(1), 1);
        assert_eq!(height_for(25), 1);
        assert_eq!(height_for(26), 2);
        assert_eq!(height_for(50), 2);
        assert_eq!(height_for(51), 3);
        assert_eq!(height_for(75), 3);
        assert_eq!(height_for(76), 4);
        assert_eq!(height_for(100), 4);
        assert_eq!(height_for(255), 4);
    }

    #[test]
    fn full_height_cell_is_braille_full() {
        // Both columns at max height should produce U+28FF (all 8 dots).
        let c = braille_cell(4, 4);
        assert_eq!(c as u32, 0x28FF);
    }

    fn timed(pcts: &[u8]) -> Vec<(u8, u64)> {
        pcts.iter()
            .enumerate()
            .map(|(i, p)| (*p, i as u64 * 1_000))
            .collect()
    }

    #[test]
    fn render_uses_caller_supplied_color() {
        let samples = timed(&[10, 20, 30]);
        let s = render(&samples, "BRAND", GlyphMode::Icon, true);
        assert!(s.contains("BRAND"), "expected caller color marker in {s:?}");
    }

    #[test]
    fn render_no_color_returns_raw_glyphs() {
        let samples = timed(&[10, 20, 30]);
        let plain = render(&samples, "BRAND", GlyphMode::Icon, false);
        assert_eq!(plain.chars().count(), 6);
        assert!(!plain.contains('\x1b'));
        assert!(!plain.contains("BRAND"));
    }

    #[test]
    fn render_empty_fill_color_returns_raw_glyphs() {
        let samples = timed(&[10, 20, 30]);
        let plain = render(&samples, "", GlyphMode::Icon, true);
        assert_eq!(plain.chars().count(), 6);
        assert!(!plain.contains('\x1b'));
    }

    #[test]
    fn ascii_mode_returns_empty_string() {
        let samples = timed(&[10, 20, 30, 40, 50, 60, 70, 80]);
        assert_eq!(render(&[], "", GlyphMode::Ascii, true), "");
        assert_eq!(render(&timed(&[50]), "BRAND", GlyphMode::Ascii, true), "");
        assert_eq!(render(&samples, "BRAND", GlyphMode::Ascii, true), "");
    }

    #[test]
    fn aurora_velocity_picks_low_under_one_per_minute() {
        let p = super::super::test_support::aurora_marker_palette();
        // 4% rise over 5 min → 0.8 %/min → low.
        let window = vec![(40, 0), (44, 5 * 60_000)];
        assert!(aurora_for_velocity(&window, &p).contains("LOW"));
    }

    #[test]
    fn aurora_velocity_picks_mid_in_one_to_five_band() {
        let p = super::super::test_support::aurora_marker_palette();
        // 13% rise over 5 min → 2.6 %/min → mid.
        let window = vec![(30, 0), (43, 5 * 60_000)];
        assert!(aurora_for_velocity(&window, &p).contains("MID"));
    }

    #[test]
    fn aurora_velocity_picks_high_above_five_per_minute() {
        let p = super::super::test_support::aurora_marker_palette();
        // 30% rise over 1 min → 30 %/min → high.
        let window = vec![(10, 0), (40, 60_000)];
        assert!(aurora_for_velocity(&window, &p).contains("HIGH"));
    }
}
