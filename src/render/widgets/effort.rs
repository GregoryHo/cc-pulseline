//! Effort ordinal pip-ramp — `▮▮▮▮▮` (icon, colour-differentiated) /
//! `===--` (ascii / NO_COLOR).
//!
//! Renders the model effort level as a fixed-length pip ramp: the lit
//! count is the level's 1-based ordinal on a pinned scale, so the reader
//! gets a *spatial* "3-of-5" reading the bare word can't give. The ramp
//! is the gauge-alongside-text companion to the effort word — it never
//! decodes a level on its own (the word always names it; see
//! `render_effort_visual`).
//!
//! Following the design system, every cell is the *same* glyph `▮`
//! (U+25AE); the lit/dim split is carried purely by colour — lit pips in
//! the escalating effort colour (`color_for_effort_level`), dim pips in
//! `separator`. A single glyph only reads with colour on, so under Ascii
//! OR NO_COLOR the ramp falls back to distinct shapes (`=` lit / `-` dim,
//! the gauge's ascii dialect) — the N-of-5 count survives without colour.
//!
//! Effort is an open string in the stdin contract (`EffortInfo.level:
//! Option<String>`, no enum), so the ordinal scale is pinned HERE. Values
//! off the scale (e.g. `auto`, or a future level) degrade to a single lit
//! pip — honest ("some effort"), with no false N-of-M reading.

use crate::config::GlyphMode;
use crate::render::color::{colorize, ThemePalette};

/// Design-system pip glyph: one `▮` (U+25AE) for BOTH lit and dim cells —
/// the ramp differentiates purely by colour (see module docs).
const PIP_ICON: char = '\u{25AE}'; // ▮

/// Colourless fallback (Ascii mode or NO_COLOR): distinct shapes so the
/// N-of-5 reading survives without colour — `=` lit / `-` dim.
const FILLED_FALLBACK: char = '=';
const EMPTY_FALLBACK: char = '-';

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
    let fill_color = palette.color_for_effort_level(level);

    // The single-glyph `▮` form differentiates lit/dim by colour alone, so
    // it only reads with colour on AND icons available. Otherwise fall back
    // to distinct `=`/`-` shapes (Ascii mode, or NO_COLOR).
    let color_form = color_enabled && matches!(mode, GlyphMode::Icon);
    let (filled_ch, empty_ch) = if color_form {
        (PIP_ICON, PIP_ICON)
    } else {
        (FILLED_FALLBACK, EMPTY_FALLBACK)
    };

    let filled = match ordinal(level) {
        // Off-scale value — degrade to a single lit pip (no N-of-M lie).
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

    // Lit pips in the escalating effort colour; dim pips in `separator`
    // (dimmer than `structural`) so the single-glyph form still reads at
    // `low`, whose fill colour is itself structural.
    let mut out = String::with_capacity(filled_buf.len() + empty_buf.len() + 32);
    out.push_str(&colorize(&filled_buf, fill_color, true));
    if !empty_buf.is_empty() {
        out.push_str(&colorize(&empty_buf, &palette.separator, true));
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
    fn colourless_icon_uses_fallback_shapes_at_its_ordinal() {
        // Icon mode but colour OFF → distinct =/- shapes; "high" = 3 of 5.
        let s = render("high", GlyphMode::Icon, &p(), false);
        assert_eq!(s, "===--");
    }

    #[test]
    fn colour_icon_uses_one_glyph_split_by_colour() {
        // Colour ON + icon → every cell is the same ▮ (U+25AE); 5 total,
        // lit/dim carried by colour. "high" → active_amber lit, separator dim.
        let mut palette = p();
        palette.active_amber = "AMBER".to_string();
        palette.separator = "SEP".to_string();
        let s = render("high", GlyphMode::Icon, &palette, true);
        assert_eq!(
            s.chars().filter(|c| *c == PIP_ICON).count(),
            5,
            "all five cells are ▮: {s:?}"
        );
        assert!(s.contains("AMBER"), "lit pips carry the effort colour: {s:?}");
        assert!(s.contains("SEP"), "dim pips carry separator: {s:?}");
    }

    #[test]
    fn lowest_and_highest_levels_anchor_the_scale() {
        // Count via the colourless fallback for determinism: low = 1 of 5,
        // max = 5 of 5 (no dim cells).
        assert_eq!(render("low", GlyphMode::Icon, &p(), false), "=----");
        assert_eq!(render("max", GlyphMode::Icon, &p(), false), "=====");
    }

    #[test]
    fn low_level_stays_legible_in_colour_form() {
        // low's fill colour IS structural; the dim pips must differ
        // (separator) or the single-glyph ramp collapses to 5 identical cells.
        let mut palette = p();
        palette.structural = "STRUCT".to_string();
        palette.separator = "SEP".to_string();
        let s = render("low", GlyphMode::Icon, &palette, true);
        assert!(s.contains("STRUCT"), "lit low pip: {s:?}");
        assert!(s.contains("SEP"), "dim pips distinct from lit: {s:?}");
    }

    #[test]
    fn unknown_value_degrades_to_single_pip() {
        // `auto` and future levels are off the ordinal scale: one pip, no
        // empty cells, so the reader never sees a false "1-of-5".
        for level in ["auto", "ludicrous", ""] {
            let s = render(level, GlyphMode::Icon, &p(), false);
            assert_eq!(s, "=", "level {level:?} → {s:?}");
        }
    }

    #[test]
    fn ascii_mode_is_not_icon_gated() {
        // Unlike the sparkline, the ramp survives Ascii as `===--`.
        let s = render("high", GlyphMode::Ascii, &p(), false);
        assert_eq!(s, "===--");
    }

    #[test]
    fn ascii_mode_emits_no_unicode_glyphs() {
        for level in ["low", "medium", "high", "xhigh", "max", "auto"] {
            let s = render(level, GlyphMode::Ascii, &p(), false);
            for c in s.chars() {
                assert!(
                    c == FILLED_FALLBACK || c == EMPTY_FALLBACK,
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
}
