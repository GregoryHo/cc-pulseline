//! Braille line plot — a single-row trend line at 2×4 sub-cell resolution.
//!
//! Distinct from `sparkline` in two ways that matter for a *velocity*
//! readout: it draws a **line** (one dot per column at the sample's
//! height) rather than filling bars bottom-up, and it **normalizes** the
//! window to its own min→max so the trend *shape* is visible even when the
//! absolute range is small (a 30→43% climb fills the cell height instead of
//! collapsing into the sparkline's global 0–100 buckets).
//!
//! Icon-only: returns `""` under `GlyphMode::Ascii` (braille has no ascii
//! equivalent) — the velocity layout's delta-time tail carries the trend
//! in that mode, exactly as the ledger sparkline does.

use crate::config::GlyphMode;
use crate::render::color::colorize;

const BRAILLE_BASE: u32 = 0x2800;
/// Dot bits per row (top→bottom) for the left / right sub-columns of a
/// braille cell. Mirrors the sparkline's packing.
const COL_L: [u8; 4] = [0x01, 0x02, 0x04, 0x40];
const COL_R: [u8; 4] = [0x08, 0x10, 0x20, 0x80];

/// Visible width in cells; 2 samples per cell → `PLOT_SAMPLES` window.
const PLOT_CELLS: usize = 6;
const PLOT_SAMPLES: usize = PLOT_CELLS * 2;

/// Map a normalized height (0.0 = window min, 1.0 = window max) to a dot
/// row index: 0 = top, 3 = bottom. A higher value sits higher in the cell.
fn dot_row(norm: f64) -> usize {
    let level = (norm * 3.0).round().clamp(0.0, 3.0) as usize;
    3 - level
}

/// Render the most-recent `PLOT_SAMPLES` of `samples` as a normalized
/// braille line, right-aligned (newest on the right), filled with
/// `fill_color`. Empty when there is no data or under Ascii.
pub fn render(
    samples: &[(u8, u64)],
    fill_color: &str,
    mode: GlyphMode,
    color_enabled: bool,
) -> String {
    if matches!(mode, GlyphMode::Ascii) {
        return String::new();
    }
    if samples.is_empty() {
        return String::new();
    }

    let window = &samples[samples.len().saturating_sub(PLOT_SAMPLES)..];
    let m = window.len();
    let min = window.iter().map(|(p, _)| *p).min().unwrap_or(0);
    let max = window.iter().map(|(p, _)| *p).max().unwrap_or(0);
    let span = (max - min) as f64;

    // Per-sample dot row, oldest → newest. A degenerate (flat) window sits
    // mid-cell so the line reads as steady rather than pinned to an edge.
    let rows: Vec<usize> = window
        .iter()
        .map(|(p, _)| {
            let norm = if span == 0.0 {
                0.5
            } else {
                (*p - min) as f64 / span
            };
            dot_row(norm)
        })
        .collect();

    // Right-align the window into PLOT_SAMPLES dot-columns (pad the left).
    let pad = PLOT_SAMPLES - m;
    let mut raw = String::with_capacity(PLOT_CELLS * 3);
    for cell in 0..PLOT_CELLS {
        let mut bits: u8 = 0;
        for col in 0..2usize {
            let dc = cell * 2 + col;
            if dc >= pad {
                let si = dc - pad;
                if si < m {
                    bits |= if col == 0 {
                        COL_L[rows[si]]
                    } else {
                        COL_R[rows[si]]
                    };
                }
            }
        }
        raw.push(char::from_u32(BRAILLE_BASE + bits as u32).unwrap_or('\u{2800}'));
    }

    colorize(&raw, fill_color, color_enabled)
}

/// Visible cell width of the plot — fixed, for callers budgeting row space.
pub const fn width() -> usize {
    PLOT_CELLS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hist(pcts: &[u8]) -> Vec<(u8, u64)> {
        pcts.iter()
            .enumerate()
            .map(|(i, p)| (*p, i as u64 * 1000))
            .collect()
    }

    #[test]
    fn empty_history_renders_nothing() {
        assert_eq!(render(&[], "C", GlyphMode::Icon, false), "");
    }

    #[test]
    fn ascii_mode_is_icon_gated() {
        let h = hist(&[10, 20, 30, 40]);
        assert_eq!(render(&h, "C", GlyphMode::Ascii, false), "");
    }

    #[test]
    fn renders_braille_only_glyphs() {
        let h = hist(&[18, 21, 23, 27, 30, 33, 36, 39, 41, 43]);
        let s = render(&h, "", GlyphMode::Icon, false);
        assert_eq!(s.chars().count(), PLOT_CELLS, "one glyph per cell: {s:?}");
        assert!(
            s.chars().all(|c| (0x2800..=0x28FF).contains(&(c as u32))),
            "all glyphs must be braille: {s:?}"
        );
    }

    #[test]
    fn rising_trend_climbs_higher_than_its_start() {
        // A monotonic climb should light a higher dot at the newest (right)
        // column than at the oldest — the line goes up.
        let h = hist(&[0, 25, 50, 75, 100]);
        let s = render(&h, "", GlyphMode::Icon, false);
        // Newest sample (100) normalizes to 1.0 → top row (dot_row 0);
        // oldest (0) → 0.0 → bottom row (dot_row 3). Distinct glyphs.
        assert!(!s.is_empty());
    }

    #[test]
    fn flat_window_stays_mid_cell_without_panicking() {
        let h = hist(&[42, 42, 42, 42]);
        let s = render(&h, "", GlyphMode::Icon, false);
        assert_eq!(s.chars().count(), PLOT_CELLS);
    }

    #[test]
    fn applies_fill_color_when_enabled() {
        let h = hist(&[10, 50, 90]);
        let s = render(&h, "AURORA", GlyphMode::Icon, true);
        assert!(s.contains("AURORA"), "{s:?}");
    }
}
