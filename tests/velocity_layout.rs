//! `velocity` layout — CTX row leads with the braille line-plot + a
//! delta-time tail. Builds `RenderFrame`s directly and calls `render_frame`
//! so assertions are deterministic and isolated from any on-disk config.

use cc_pulseline::config::{GlyphMode, RenderConfig};
use cc_pulseline::render::color::resolve_palette;
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::RenderFrame;

fn cfg(icons: bool) -> RenderConfig {
    RenderConfig {
        color_enabled: false,
        glyph_mode: if icons {
            GlyphMode::Icon
        } else {
            GlyphMode::Ascii
        },
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        pane_style: LayoutStyle::Velocity,
        terminal_width: Some(160),
        ..RenderConfig::default()
    }
}

fn velocity_frame() -> RenderFrame {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus".to_string();
    f.line1.claude_code_version = "2.2".to_string();
    f.line1.git_branch = "main".to_string();
    f.line1.effort_level = Some("high".to_string());
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
fn ctx_row_leads_with_braille_plot_then_tail_then_number() {
    let lines = render_frame(&velocity_frame(), &cfg(true));
    let ctx = ctx_row(&lines);

    // The plot (braille) leads, ahead of the delta tail and the CTX number.
    let first_braille = ctx.chars().position(is_braille);
    assert!(
        first_braille.is_some(),
        "expected a braille plot glyph: {ctx}"
    );
    let arrow = ctx.find('→').expect("delta-time tail with → arrow");
    let number = ctx.find("86.0k/200.0k").expect("CTX used/total number");
    let braille_byte = ctx
        .char_indices()
        .find(|(_, c)| is_braille(*c))
        .map(|(i, _)| i)
        .unwrap();
    assert!(braille_byte < arrow, "plot must lead the delta tail: {ctx}");
    assert!(arrow < number, "delta tail precedes the CTX number: {ctx}");
    assert!(ctx.contains("18→43% in 5m"), "delta tail text: {ctx}");
    assert!(ctx.contains("43%"), "CTX percentage present: {ctx}");
}

#[test]
fn plot_drops_under_ascii_but_the_tail_keeps_the_trend() {
    let lines = render_frame(&velocity_frame(), &cfg(false));
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

#[test]
fn is_frameless_flat_layout() {
    let lines = render_frame(&velocity_frame(), &cfg(true));
    let blob = lines.join("\n");
    assert!(
        !blob.contains('╭') && !blob.contains('│'),
        "velocity is a flat layout — no frame chrome:\n{blob}"
    );
}

#[test]
fn identity_leads_with_effort_ramp() {
    let lines = render_frame(&velocity_frame(), &cfg(false));
    assert!(
        lines[0].contains("E:high ===--"),
        "velocity defaults effort_visual to word+ramp; got:\n{}",
        lines[0]
    );
}
