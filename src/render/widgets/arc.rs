//! Cost-burn arc — single-glyph fill indicator.
//!
//! Maps a $/h burn rate to one of `○ ◔ ◑ ◕ ●`, giving a 5-step pictographic
//! gauge that pairs with the dollar text. Calibrated against the existing
//! cost-rate color thresholds (cost.rs::color_for_burn_rate):
//!
//! | rate ($/h)      | glyph | color           |
//! |-----------------|-------|-----------------|
//! | 0.0             | ○     | structural      |
//! | < 5             | ◔     | aurora_low      |
//! | < 15            | ◑     | aurora_mid      |
//! | < 50            | ◕     | active_amber    |
//! | >= 50           | ●     | aurora_high     |
//!
//! The thresholds are tuned for the typical Claude Code session shape: 0–5/h
//! is "doing fine," 5–15/h is "actively coding," 15–50/h is "watch out," and
//! ≥50/h is "long parallel run." Sized so a single glyph carries information.

use crate::render::color::{colorize, ThemePalette};

/// Render the cost arc glyph with its color, given hourly burn rate (USD/h).
pub fn render(burn_per_hour: f64, palette: &ThemePalette, color_enabled: bool) -> String {
    let (glyph, color) = pick(burn_per_hour, palette);
    colorize(&glyph.to_string(), color, color_enabled)
}

/// Bare glyph (no color), used by tests and width-conscious callers.
pub fn glyph_for(burn_per_hour: f64) -> char {
    pick_glyph(burn_per_hour)
}

/// Visible width — always 1 cell.
pub const fn width() -> usize {
    1
}

fn pick(burn_per_hour: f64, palette: &ThemePalette) -> (char, &str) {
    let glyph = pick_glyph(burn_per_hour);
    let color = if burn_per_hour <= 0.0 {
        &palette.structural
    } else if burn_per_hour < 5.0 {
        &palette.aurora_low
    } else if burn_per_hour < 15.0 {
        &palette.aurora_mid
    } else if burn_per_hour < 50.0 {
        &palette.active_amber
    } else {
        &palette.aurora_high
    };
    (glyph, color)
}

fn pick_glyph(burn_per_hour: f64) -> char {
    if burn_per_hour <= 0.0 {
        '\u{25CB}' // ○
    } else if burn_per_hour < 5.0 {
        '\u{25D4}' // ◔
    } else if burn_per_hour < 15.0 {
        '\u{25D1}' // ◑
    } else if burn_per_hour < 50.0 {
        '\u{25D5}' // ◕
    } else {
        '\u{25CF}' // ●
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::widgets::test_support::aurora_marker_palette;

    #[test]
    fn zero_burn_renders_empty_circle() {
        assert_eq!(glyph_for(0.0), '\u{25CB}');
    }

    #[test]
    fn fractional_burn_renders_quarter() {
        assert_eq!(glyph_for(0.5), '\u{25D4}');
        assert_eq!(glyph_for(4.99), '\u{25D4}');
    }

    #[test]
    fn mid_burn_renders_half() {
        assert_eq!(glyph_for(5.0), '\u{25D1}');
        assert_eq!(glyph_for(14.99), '\u{25D1}');
    }

    #[test]
    fn high_burn_renders_three_quarters() {
        assert_eq!(glyph_for(15.0), '\u{25D5}');
        assert_eq!(glyph_for(49.99), '\u{25D5}');
    }

    #[test]
    fn over_50_renders_full_circle() {
        assert_eq!(glyph_for(50.0), '\u{25CF}');
        assert_eq!(glyph_for(500.0), '\u{25CF}');
    }

    #[test]
    fn render_with_color_picks_aurora_stop() {
        let mut p = aurora_marker_palette();
        p.structural = "STRUCT".to_string();
        p.active_amber = "AMBER".to_string();

        assert!(render(0.0, &p, true).contains("STRUCT"));
        assert!(render(2.0, &p, true).contains("LOW"));
        assert!(render(10.0, &p, true).contains("MID"));
        assert!(render(20.0, &p, true).contains("AMBER"));
        assert!(render(80.0, &p, true).contains("HIGH"));
    }

    #[test]
    fn render_no_color_returns_just_glyph() {
        let p = aurora_marker_palette();
        let s = render(10.0, &p, false);
        assert_eq!(s.chars().count(), 1);
        assert_eq!(s.chars().next().unwrap(), '\u{25D1}');
    }
}
