//! Badge layout contract — the experimental background-fill layout.
//!
//! Pins: the three render tiers (DRIFT pills / STENCIL slabs / ASCII
//! brackets) keyed off `(color_enabled, glyph_mode)`, the deep-teal accent
//! landing only on budget/quota, the warm alert swap, dash placeholders for
//! absent data, and the width-degradation ladder's fit contract.

use cc_pulseline::config::{GlyphMode, RenderConfig};
use cc_pulseline::render::color::{resolve_palette, visible_width};
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{QuotaMetrics, RenderFrame};

// ANSI fragments the tiers emit.
const TEAL_BG: &str = "\x1b[48;5;30m"; // accent
const NEUTRAL_BG: &str = "\x1b[48;5;238m"; // value half
const LABEL_BG: &str = "\x1b[48;5;236m"; // label half
const WARN_BG: &str = "\x1b[48;5;173m"; // quota warn
const CRIT_BG: &str = "\x1b[48;5;130m"; // quota critical
const CAP_L: &str = "\u{e0b6}"; // rounded left cap

fn full_frame() -> RenderFrame {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus 4.8".to_string();
    f.line1.effort_level = Some("high".to_string());
    f.line1.claude_code_version = "2.1.119".to_string();
    f.line1.project_path = "~/AI/cc-pulseline".to_string();
    f.line1.git_branch = "main".to_string();
    f.line1.git_dirty = true;
    f.line3.context_window_size = Some(200_000);
    f.line3.context_used_percentage = Some(43);
    f.line3.input_tokens = Some(12_400);
    f.line3.output_tokens = Some(3_100);
    f.line3.cache_creation_tokens = Some(1_800);
    f.line3.cache_read_tokens = Some(98_000);
    f.quota = QuotaMetrics {
        five_hour_pct: Some(62.0),
        five_hour_reset_minutes: Some(128),
        seven_day_pct: Some(19.0),
        seven_day_reset_minutes: Some(60 * 24 * 5),
    };
    f
}

fn cfg(color: bool, glyph: GlyphMode, width: usize) -> RenderConfig {
    RenderConfig {
        glyph_mode: glyph,
        color_enabled: color,
        terminal_width: Some(width + 4), // +cc_margin so the layout sees `width`
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        pane_style: LayoutStyle::Badge,
        pane_cc_margin: 4,
        show_quota: true,
        show_quota_five_hour: true,
        show_quota_seven_day: true,
        ..RenderConfig::default()
    }
}

fn blob(frame: &RenderFrame, c: &RenderConfig) -> String {
    render_frame(frame, c).join("\n")
}

#[test]
fn drift_tier_paints_teal_pills_with_caps() {
    let out = blob(&full_frame(), &cfg(true, GlyphMode::Icon, 160));
    assert!(out.contains(CAP_L), "DRIFT must draw rounded caps:\n{out}");
    assert!(
        out.contains(TEAL_BG),
        "accent teal must appear on budget/quota"
    );
    assert!(out.contains(NEUTRAL_BG), "neutral value halves must appear");
    assert!(out.contains(LABEL_BG), "label halves must appear");
}

#[test]
fn stencil_tier_fills_without_caps() {
    let out = blob(&full_frame(), &cfg(true, GlyphMode::Ascii, 160));
    assert!(
        out.contains(NEUTRAL_BG),
        "STENCIL must still paint bg fills"
    );
    assert!(!out.contains(CAP_L), "STENCIL must not draw powerline caps");
    assert!(
        out.contains("in "),
        "ascii glyph mode spells tokens (in/out)"
    );
    assert!(!out.contains('↓'), "ascii glyph mode drops arrow glyphs");
}

#[test]
fn ascii_tier_is_plain_brackets() {
    let out = blob(&full_frame(), &cfg(false, GlyphMode::Ascii, 160));
    assert!(
        out.contains("[MDL Opus 4.8]"),
        "ASCII tier uses brackets:\n{out}"
    );
    assert!(
        !out.contains('\x1b'),
        "NO_COLOR tier emits zero ANSI:\n{out}"
    );
    assert!(!out.contains("48;5"), "NO_COLOR tier emits no bg fills");
}

#[test]
fn quota_value_redlines_when_near_limit() {
    let mut f = full_frame();
    f.quota.five_hour_pct = Some(88.0); // ≥ 85 → critical
    f.quota.seven_day_pct = Some(19.0); // healthy → teal
    let out = blob(&f, &cfg(true, GlyphMode::Icon, 200));
    assert!(
        out.contains(CRIT_BG),
        "a critical quota window fills warm:\n{out}"
    );
    assert!(out.contains(TEAL_BG), "a healthy quota window stays teal");
}

#[test]
fn absent_data_shows_dashes_without_accent() {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus 4.8".to_string();
    f.line1.git_branch = "main".to_string();
    // line3 + quota all default None.
    let out = blob(&f, &cfg(true, GlyphMode::Icon, 160));
    assert!(
        out.contains("--"),
        "missing budget/quota renders dashes:\n{out}"
    );
    assert!(!out.contains(TEAL_BG), "no accent without live data");
    assert!(
        !out.contains(WARN_BG) && !out.contains(CRIT_BG),
        "no warm without data"
    );
}

#[test]
fn drift_renders_three_rows() {
    let lines = render_frame(&full_frame(), &cfg(true, GlyphMode::Icon, 160));
    assert_eq!(lines.len(), 3, "identity / budget / quota:\n{lines:#?}");
}

#[test]
fn every_line_fits_the_terminal_width() {
    let f = full_frame();
    for w in [200usize, 160, 130, 110, 95] {
        let lines = render_frame(&f, &cfg(true, GlyphMode::Icon, w));
        for line in &lines {
            assert!(
                visible_width(line) <= w,
                "width {w}: line overflows ({} cols): {line:?}",
                visible_width(line)
            );
        }
    }
}

#[test]
fn width_ladder_sheds_ctx_total_then_reset() {
    let f = full_frame();
    let wide = blob(&f, &cfg(true, GlyphMode::Icon, 200));
    assert!(wide.contains("86.0k/200.0k"), "wide keeps CTX used/total");
    assert!(wide.contains("resets"), "wide keeps quota reset countdown");

    // Shrinking the width must eventually shed CTX used/total while the
    // render is still a badge (not yet the Compact fallback). The exact
    // pixel width depends on content, so scan rather than hardcode.
    let shed = (60..=200)
        .rev()
        .map(|w| blob(&f, &cfg(true, GlyphMode::Icon, w)))
        .find(|out| out.contains(LABEL_BG) && !out.contains("86.0k/200.0k"));
    let shed = shed.expect("some badge width should shed CTX used/total");
    assert!(
        shed.contains("43%"),
        "CTX percentage survives the shed:\n{shed}"
    );
}

#[test]
fn very_narrow_falls_back_to_compact() {
    // No badge ladder rung fits 30 cols → drop to Compact (no bg pills).
    let out = blob(&full_frame(), &cfg(true, GlyphMode::Icon, 30));
    assert!(
        !out.contains(LABEL_BG),
        "fallback must not emit badge pills:\n{out}"
    );
    assert!(!out.contains(CAP_L), "fallback must not emit badge caps");
    assert!(!out.is_empty(), "fallback still renders something");
}
