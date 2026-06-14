//! `budgets` layout — three column-aligned equal-weight gauges.
//!
//! Builds `RenderFrame`s directly and calls `render_frame` so assertions
//! are deterministic. Color is disabled; glyphs are Ascii except where a
//! test specifically exercises Icon mode (the block-dialect guard).

use cc_pulseline::config::{GlyphMode, RenderConfig};
use cc_pulseline::render::color::resolve_palette;
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{QuotaMetrics, RenderFrame};

fn cfg(icons: bool) -> RenderConfig {
    RenderConfig {
        color_enabled: false,
        glyph_mode: if icons {
            GlyphMode::Icon
        } else {
            GlyphMode::Ascii
        },
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        pane_style: LayoutStyle::Budgets,
        show_quota: true,
        show_quota_five_hour: true,
        show_quota_seven_day: true,
        // Wide enough that the gauge width clamps to the full BUDGET_GAUGE_W.
        terminal_width: Some(200),
        ..RenderConfig::default()
    }
}

fn budgets_frame() -> RenderFrame {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus".to_string();
    f.line1.claude_code_version = "2.2.0".to_string();
    f.line1.project_path = "~/proj".to_string();
    f.line1.git_branch = "main".to_string();
    f.line1.effort_level = Some("high".to_string());
    f.line3.context_window_size = Some(200_000);
    f.line3.context_used_percentage = Some(43);
    f.line3.total_cost_usd = Some(4.56);
    f.quota = QuotaMetrics {
        five_hour_pct: Some(62.0),
        five_hour_reset_minutes: Some(119),
        seven_day_pct: Some(28.0),
        seven_day_reset_minutes: Some(8_640),
    };
    f
}

/// Count of gauge cells (`=` fill, `-` empty, `:` mark) on a row — under
/// Ascii these glyphs appear nowhere else on a budget row, so the count is
/// the gauge's visible width.
fn ascii_gauge_len(line: &str) -> usize {
    line.chars()
        .filter(|c| matches!(c, '=' | '-' | ':'))
        .count()
}

fn row_with<'a>(lines: &'a [String], needle: &str) -> &'a str {
    lines
        .iter()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no row containing {needle:?} in:\n{}", lines.join("\n")))
        .as_str()
}

#[test]
fn renders_three_equal_width_aligned_gauges() {
    let lines = render_frame(&budgets_frame(), &cfg(false));
    let ctx = row_with(&lines, "CONTEXT");
    let q5 = row_with(&lines, "5H QUOTA");
    let q7 = row_with(&lines, "7D QUOTA");

    // Each budget window keeps its own percentage + trailing text.
    assert!(ctx.contains("43%") && ctx.contains("86.0k/200.0k"), "{ctx}");
    assert!(q5.contains("62%") && q5.contains("resets 1h 59m"), "{q5}");
    assert!(q7.contains("28%") && q7.contains("resets 6d"), "{q7}");

    // The three gauges share one width so they align on a common axis.
    let (lc, l5, l7) = (
        ascii_gauge_len(ctx),
        ascii_gauge_len(q5),
        ascii_gauge_len(q7),
    );
    assert_eq!(lc, l5, "CONTEXT vs 5H gauge widths differ: {lc} vs {l5}");
    assert_eq!(l5, l7, "5H vs 7D gauge widths differ: {l5} vs {l7}");
    assert_eq!(lc, 24, "expected the full BUDGET_GAUGE_W (24) at width 200");
}

#[test]
fn identity_row_leads_with_effort_ramp() {
    // budgets defaults effort_visual to "word+ramp"; ascii ramp is `===--`.
    let lines = render_frame(&budgets_frame(), &cfg(false));
    let identity = &lines[0];
    assert!(
        identity.contains("E:high ===--"),
        "identity should carry the word+ramp effort cell; got:\n{identity}"
    );
}

#[test]
fn context_row_carries_compaction_marker() {
    let mut frame = budgets_frame();
    frame.compact_count = 2;
    let lines = render_frame(&frame, &cfg(false));
    let ctx = row_with(&lines, "CONTEXT");
    assert!(
        ctx.contains("~2"),
        "CONTEXT row should show the ascii compaction marker ~2; got:\n{ctx}"
    );
}

#[test]
fn omits_quota_rows_when_quota_disabled() {
    let mut config = cfg(false);
    config.show_quota = false;
    let lines = render_frame(&budgets_frame(), &config);
    let blob = lines.join("\n");
    assert!(blob.contains("CONTEXT"), "CONTEXT always renders:\n{blob}");
    assert!(blob.contains("TOK"), "TOKENS row always renders:\n{blob}");
    assert!(
        !blob.contains("5H QUOTA") && !blob.contains("7D QUOTA"),
        "quota rows must vanish when quota is disabled:\n{blob}"
    );
}

#[test]
fn reuses_marks_gauge_never_the_block_dialect() {
    // The brief rejects the mockup's `█▌░` block bar in favour of the
    // shipped `▰─·` marks gauge. Under Icon mode the budget rows must use
    // `▰`/`─`/`·` and never leak a block-bar glyph.
    let lines = render_frame(&budgets_frame(), &cfg(true));
    let blob = lines.join("\n");
    assert!(blob.contains('\u{25B0}'), "expected ▰ marks-gauge fill"); // ▰
    for block in ['\u{2588}', '\u{258C}', '\u{2591}'] {
        // █ ▌ ░
        assert!(
            !blob.contains(block),
            "budgets leaked the rejected block-bar glyph U+{:04X}:\n{blob}",
            block as u32
        );
    }
}
