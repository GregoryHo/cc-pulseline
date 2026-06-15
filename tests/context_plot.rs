//! Braille CTX **plot** widget via `context_visual = "plot+text"` on a
//! generic (non-velocity) layout.
//!
//! The velocity layout was removed — it was a config preset (`none` +
//! `plot+text` + `gauge`) with no bespoke builder. The plot *widget* lives
//! on: any layout opts into it through the dispatch hub via the `plot`
//! context_visual atom (the documented "trend-forward" recipe). These
//! assertions keep that widget covered and double as proof of the recipe.

use cc_pulseline::config::{GlyphMode, RenderConfig};
use cc_pulseline::render::color::resolve_palette;
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::RenderFrame;

/// `none` layout + an explicit `plot+text` CTX spec — reproduces what the
/// velocity layout used to default to, on a layout that is NOT velocity.
fn cfg(icons: bool) -> RenderConfig {
    RenderConfig {
        color_enabled: false,
        glyph_mode: if icons {
            GlyphMode::Icon
        } else {
            GlyphMode::Ascii
        },
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        pane_style: LayoutStyle::None,
        context_visual: "plot+text".to_string(),
        terminal_width: Some(160),
        ..RenderConfig::default()
    }
}

fn plot_frame() -> RenderFrame {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus".to_string();
    f.line1.claude_code_version = "2.2".to_string();
    f.line1.git_branch = "main".to_string();
    f.line3.context_window_size = Some(200_000);
    f.line3.context_used_percentage = Some(43);
    // 6 samples spanning 5 minutes → delta tail "18→43% in 5m".
    f.ctx_history = vec![
        (18, 0),
        (24, 60_000),
        (30, 120_000),
        (33, 180_000),
        (39, 240_000),
        (43, 300_000),
    ];
    f
}

fn is_braille(c: char) -> bool {
    (0x2800..=0x28FF).contains(&(c as u32))
}

fn ctx_row(lines: &[String]) -> &str {
    lines
        .iter()
        .find(|l| l.contains("86.0k/200.0k"))
        .unwrap_or_else(|| panic!("no CTX row in:\n{}", lines.join("\n")))
        .as_str()
}

#[test]
fn plot_spec_leads_with_braille_then_tail_then_number() {
    let lines = render_frame(&plot_frame(), &cfg(true));
    let ctx = ctx_row(&lines);

    // The plot (braille) leads, ahead of the delta tail and the CTX number.
    let arrow = ctx.find('→').expect("delta-time tail with → arrow");
    let number = ctx.find("86.0k/200.0k").expect("CTX used/total number");
    let braille_byte = ctx
        .char_indices()
        .find(|(_, c)| is_braille(*c))
        .map(|(i, _)| i)
        .expect("expected a braille plot glyph");
    assert!(braille_byte < arrow, "plot must lead the delta tail: {ctx}");
    assert!(arrow < number, "delta tail precedes the CTX number: {ctx}");
    assert!(ctx.contains("18→43% in 5m"), "delta tail text: {ctx}");
    assert!(ctx.contains("43%"), "CTX percentage present: {ctx}");
}

#[test]
fn plot_drops_under_ascii_but_the_tail_keeps_the_trend() {
    let lines = render_frame(&plot_frame(), &cfg(false));
    let ctx = ctx_row(&lines);
    assert!(
        !ctx.chars().any(is_braille),
        "braille plot must vanish under ascii: {ctx}"
    );
    // The text tail still carries the trend, plus the number.
    assert!(
        ctx.contains("18→43% in 5m"),
        "tail carries the trend: {ctx}"
    );
    assert!(ctx.contains("86.0k/200.0k"), "CTX number remains: {ctx}");
}
