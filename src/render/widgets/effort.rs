//! Effort ordinal pip-ramp — `▰▰▰··` (icon) / `===--` (ascii).
//!
//! Renders the model effort level as a fixed-length pip ramp: the filled
//! count is the level's 1-based ordinal on a pinned scale, so the reader
//! gets a *spatial* "3-of-5" reading the bare word can't give. The ramp
//! is the gauge-alongside-text companion to the effort word — it never
//! decodes a level on its own (the word always names it; see
//! `render_effort_visual`).
//!
//! Effort is an open string in the stdin contract (`EffortInfo.level:
//! Option<String>`, no enum), so the ordinal scale is pinned HERE. Values
//! off the scale (e.g. `auto`, or a future level) degrade to a single
//! filled pip — honest ("some effort"), with no false N-of-M reading.
//!
//! Block glyphs survive Ascii (unlike the braille sparkline), so this
//! widget is NOT icon-gated: it returns `===--` under `GlyphMode::Ascii`.

use crate::config::GlyphMode;
use crate::render::color::{colorize, ThemePalette};

const FILLED_ICON: char = '\u{25B0}'; // ▰
const EMPTY_ICON: char = '\u{00B7}'; // ·

const FILLED_ASCII: char = '=';
const EMPTY_ASCII: char = '-';

/// Fixed ordinal scale for the ramp. The ramp has exactly `SCALE.len()`
/// cells; the filled count is the matched level's 1-based position. Keep
/// in lockstep with `ThemePalette::color_for_effort_level`'s known arms.
const SCALE: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// 1-based ordinal of `level` on the fixed scale, or `None` for unknown /
/// non-ordinal values (`auto`, future levels).
fn ordinal(level: &str) -> Option<usize> {
    SCALE.iter().position(|l| *l == level).map(|i| i + 1)
}

/// Render the ordinal pip ramp for `level`. Filled pips take the
/// escalating effort color (`color_for_effort_level`); empty pips recede
/// into `structural` (matching the gauge's empty-track treatment).
pub fn render(level: &str, mode: GlyphMode, palette: &ThemePalette, color_enabled: bool) -> String {
    let (filled_ch, empty_ch) = match mode {
        GlyphMode::Icon => (FILLED_ICON, EMPTY_ICON),
        GlyphMode::Ascii => (FILLED_ASCII, EMPTY_ASCII),
    };
    let fill_color = palette.color_for_effort_level(level);

    let filled = match ordinal(level) {
        // Off-scale value — degrade to a single filled pip (no N-of-M lie).
        None => return colorize(&filled_ch.to_string(), fill_color, color_enabled),
        Some(n) => n,
    };
    let width = SCALE.len();

    let mut filled_buf = String::with_capacity(filled);
    for _ in 0..filled {
        filled_buf.push(filled_ch);
    }
    let mut empty_buf = String::with_capacity(width - filled);
    for _ in 0..(width - filled) {
        empty_buf.push(empty_ch);
    }

    if !color_enabled {
        return format!("{filled_buf}{empty_buf}");
    }

    let mut out = String::with_capacity(filled_buf.len() + empty_buf.len() + 32);
    out.push_str(&colorize(&filled_buf, fill_color, true));
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
    fn known_level_fills_its_ordinal_position() {
        // "high" is index 2 → 3 filled of 5.
        let s = render("high", GlyphMode::Icon, &p(), false);
        assert_eq!(s.chars().count(), 5);
        assert_eq!(s.chars().filter(|c| *c == FILLED_ICON).count(), 3);
        assert_eq!(s.chars().filter(|c| *c == EMPTY_ICON).count(), 2);
    }

    #[test]
    fn lowest_and_highest_levels_anchor_the_scale() {
        let low = render("low", GlyphMode::Icon, &p(), false);
        assert_eq!(low.chars().filter(|c| *c == FILLED_ICON).count(), 1);
        let max = render("max", GlyphMode::Icon, &p(), false);
        assert_eq!(max.chars().filter(|c| *c == FILLED_ICON).count(), 5);
        assert_eq!(max.chars().filter(|c| *c == EMPTY_ICON).count(), 0);
    }

    #[test]
    fn unknown_value_degrades_to_single_pip() {
        // `auto` and future levels are off the ordinal scale: one pip, no
        // empty cells, so the reader never sees a false "1-of-5".
        for level in ["auto", "ludicrous", ""] {
            let s = render(level, GlyphMode::Icon, &p(), false);
            assert_eq!(s.chars().count(), 1, "level {level:?} → {s:?}");
            assert_eq!(s.chars().next(), Some(FILLED_ICON));
        }
    }

    #[test]
    fn ascii_mode_is_not_icon_gated() {
        // Unlike the sparkline, the ramp survives Ascii as `===--`.
        let s = render("high", GlyphMode::Ascii, &p(), false);
        assert_eq!(s, "===--");
    }

    #[test]
    fn ascii_mode_emits_no_unicode_block_chars() {
        for level in ["low", "medium", "high", "xhigh", "max", "auto"] {
            let s = render(level, GlyphMode::Ascii, &p(), false);
            for c in s.chars() {
                assert!(
                    c == FILLED_ASCII || c == EMPTY_ASCII,
                    "unexpected ascii char {c:?} for {level:?} in {s:?}"
                );
            }
        }
    }

    #[test]
    fn disabled_color_returns_no_ansi_escapes() {
        let s = render("high", GlyphMode::Icon, &p(), false);
        assert!(!s.contains("\x1b["), "found ANSI escape: {s:?}");
    }

    #[test]
    fn filled_pips_carry_the_effort_color() {
        // aurora_marker_palette leaves most fields empty; "max" maps to
        // alert_red. Use a palette whose alert_red is a marker.
        let mut palette = p();
        palette.alert_red = "REDMARK".to_string();
        let s = render("max", GlyphMode::Icon, &palette, true);
        assert!(s.contains("REDMARK"), "rendered: {s:?}");
    }
}
