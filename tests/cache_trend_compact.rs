//! Compact layout: opt-in cache-trend sparkline on the budget row.
//!
//! The spark is a knob-gated (`show_cache_trend`, default OFF) Optional
//! cell pushed after the TOK cell, so `pack_with_separator` drops it
//! FIRST under width pressure — before TOK, and never instead of the
//! Required CTX / cost / quota cells.
//!
//! Frames are hand-built and fed to `render_frame` directly (display_axes
//! style) so every assertion is deterministic. The budget row is located
//! by its `/200.0k` CTX total — the `CTX` label is an NF icon under
//! `GlyphMode::Icon`, so the text label can't be matched there.

use cc_pulseline::config::{GlyphMode, RenderConfig};
use cc_pulseline::render::color::{resolve_palette, visible_width};
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{QuotaMetrics, RenderFrame};

fn has_braille(s: &str) -> bool {
    s.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32)))
}

/// Frame with cache signal (input 1000 / creation 500 / read 8500 → 85%
/// hit rate) and a 6-sample hit-rate history.
fn cache_frame() -> RenderFrame {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus".to_string();
    f.line1.output_style = "concise".to_string();
    f.line1.claude_code_version = "2.2.0".to_string();
    f.line1.project_path = "~/proj".to_string();
    f.line1.git_branch = "main".to_string();
    f.line3.context_window_size = Some(200_000);
    f.line3.context_used_percentage = Some(43);
    f.line3.input_tokens = Some(1_000);
    f.line3.output_tokens = Some(20);
    f.line3.cache_creation_tokens = Some(500);
    f.line3.cache_read_tokens = Some(8_500);
    f.line3.total_cost_usd = Some(3.50);
    f.line3.total_duration_ms = Some(60_000 * 30);
    f.cache_history = vec![
        (40, 0),
        (55, 1_000),
        (70, 2_000),
        (80, 3_000),
        (85, 4_000),
        (85, 5_000),
    ];
    f
}

fn cfg(width: usize, knob_on: bool) -> RenderConfig {
    RenderConfig {
        pane_style: LayoutStyle::Compact,
        glyph_mode: GlyphMode::Icon,
        color_enabled: false,
        terminal_width: Some(width),
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        show_cache_trend: knob_on,
        show_quota: false,
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    }
}

fn budget_row(lines: &[String]) -> &String {
    lines
        .iter()
        .find(|l| l.contains("/200.0k"))
        .expect("budget row with CTX totals")
}

#[test]
fn compact_renders_cache_sparkline_when_knob_on() {
    let lines = render_frame(&cache_frame(), &cfg(200, true));
    let budget = budget_row(&lines);
    assert!(
        has_braille(budget),
        "budget row should carry the braille cache spark: {budget:?}"
    );
}

#[test]
fn compact_omits_cache_sparkline_by_default() {
    // Identical setup, knob off — A6 evidence at the layout level.
    let lines = render_frame(&cache_frame(), &cfg(200, false));
    for line in &lines {
        assert!(
            !has_braille(line),
            "no braille anywhere with the knob off: {line:?}"
        );
    }
}

#[test]
fn cache_sparkline_drops_first_under_width_pressure() {
    // Wide render: spark present. Re-render 2 cells narrower than the
    // measured budget row — the spark (rightmost Optional) must drop
    // BEFORE the TOK cell.
    let frame = cache_frame();
    let wide = render_frame(&frame, &cfg(200, true));
    let wide_budget = budget_row(&wide);
    assert!(has_braille(wide_budget), "precondition: spark visible");
    let w = visible_width(wide_budget); // color off — plain visible cells

    let narrow = render_frame(&frame, &cfg(w - 2, true));
    let narrow_budget = budget_row(&narrow);
    assert!(
        !has_braille(narrow_budget),
        "spark must drop first under width pressure: {narrow_budget:?}"
    );
    assert!(
        narrow_budget.contains("TOK"),
        "TOK cell must survive after the spark drops: {narrow_budget:?}"
    );
}

#[test]
fn cache_sparkline_and_tok_drop_before_required_cells() {
    // Mirrors height_degrade's compact width-60 squeeze: the Optional
    // cells (spark, then TOK) drop, the Required CTX / quota / cost
    // cells survive.
    let mut frame = cache_frame();
    frame.quota = QuotaMetrics {
        five_hour_pct: Some(62.0),
        five_hour_reset_minutes: Some(120),
        seven_day_pct: None,
        seven_day_reset_minutes: None,
    };
    let config = RenderConfig {
        show_quota: true,
        show_quota_five_hour: true,
        ..cfg(60, true)
    };
    let lines = render_frame(&frame, &config);
    for line in &lines {
        assert!(visible_width(line) <= 60, "row must fit width 60: {line:?}");
        assert!(
            !has_braille(line),
            "spark must be gone at width 60: {line:?}"
        );
    }
    let budget = budget_row(&lines);
    assert!(
        budget.contains("/200.0k"),
        "CTX must survive width pressure: {budget:?}"
    );
    assert!(
        budget.contains("5h:62%"),
        "quota must survive width pressure: {budget:?}"
    );
    assert!(
        budget.contains('$'),
        "cost must survive width pressure: {budget:?}"
    );
}

#[test]
fn compact_ascii_mode_emits_no_braille_with_knob_on() {
    let config = RenderConfig {
        glyph_mode: GlyphMode::Ascii,
        ..cfg(200, true)
    };
    let lines = render_frame(&cache_frame(), &config);
    for line in &lines {
        assert!(
            !has_braille(line),
            "sparkline is icon-only — no braille under Ascii: {line:?}"
        );
    }
    let budget = budget_row(&lines);
    assert!(budget.contains("TOK"), "TOK cell still present: {budget:?}");
    assert!(budget.contains("C:"), "C cell still present: {budget:?}");
}

// ── End-to-end: runner populates frame.cache_history only when knob on ──

mod runner {
    use super::has_braille;
    use cc_pulseline::config::{GlyphMode, RenderConfig};
    use cc_pulseline::render::color::resolve_palette;
    use cc_pulseline::render::pane::LayoutStyle;
    use cc_pulseline::types::StdinPayload;
    use cc_pulseline::PulseLineRunner;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn payload(session_id: &str, transcript: &str, cache_read: u64) -> StdinPayload {
        let input = json!({
            "session_id": session_id,
            "model": {"display_name": "Opus"},
            "version": "1.0",
            "transcript_path": transcript,
            "context_window": {
                "context_window_size": 200000,
                "used_percentage": 43,
                "current_usage": {
                    "input_tokens": 1000,
                    "output_tokens": 20,
                    "cache_creation_input_tokens": 500,
                    "cache_read_input_tokens": cache_read
                }
            },
            "cost": {"total_cost_usd": 3.5, "total_duration_ms": 1800000}
        })
        .to_string();
        serde_json::from_str(&input).expect("valid payload json")
    }

    fn runner_cfg(knob_on: bool) -> RenderConfig {
        RenderConfig {
            pane_style: LayoutStyle::Compact,
            glyph_mode: GlyphMode::Icon,
            color_enabled: false,
            terminal_width: Some(200),
            palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
            show_cache_trend: knob_on,
            transcript_poll_throttle_ms: 0,
            ..RenderConfig::default()
        }
    }

    /// Two ticks with differing hit rates (85% then 93%) so ≥2 samples
    /// accumulate in `SessionState.cache_history`.
    fn run_twice(session_id: &str, knob_on: bool) -> Vec<String> {
        let dir = TempDir::new().expect("tempdir");
        let transcript = dir.path().join("transcript.jsonl");
        fs::write(&transcript, "").expect("create transcript");
        let transcript = transcript.to_string_lossy().to_string();

        let mut runner = PulseLineRunner::default();
        let _ = runner
            .run_from_payload(
                &payload(session_id, &transcript, 8_500),
                runner_cfg(knob_on),
            )
            .expect("first tick renders");
        runner
            .run_from_payload(
                &payload(session_id, &transcript, 18_500),
                runner_cfg(knob_on),
            )
            .expect("second tick renders")
    }

    #[test]
    fn runner_populates_cache_history_only_when_knob_on() {
        let on = run_twice("cache-trend-e2e-on", true);
        assert!(
            on.iter().any(|l| has_braille(l)),
            "knob on: cache spark must reach the output:\n{}",
            on.join("\n")
        );

        let off = run_twice("cache-trend-e2e-off", false);
        for line in &off {
            assert!(
                !has_braille(line),
                "knob off: no braille may leak into output: {line:?}"
            );
        }
    }
}
