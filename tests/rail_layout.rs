//! `rail` v2 — three grouped rows (identity · context · quota).
//!
//! Covers the Must/Should of `designs/rail-anchor-grouped-rows.md`:
//! - default = 3 rows; quota-less = 2; `max_total_lines = 1` = the v1 fused bar
//! - anti-rainbow: ZERO left-cell ink flags in a calm fixture; a high fixture
//!   lights exactly the effort/ctx/git ink flags
//! - headline column shares an axis across rows
//! - ink survives the ASCII floor; blocks tier uses half-blocks (no PUA seam)
//!
//! Ink detection is precise. An ink flag is a ramp segment (bg = RampLevel::Base
//! = 256 code 235) whose TEXT fg is a role colour — emitted as `48;5;235m`
//! immediately followed by the role `38;5` escape. A Powerline seam emits the
//! opposite order (`38;5` fg then `48;5` bg then a glyph), so a seam can never
//! match this marker even when it leaks a band colour as a foreground. That is
//! the soundness fix for the naive `contains(role_colour)` approach.

use cc_pulseline::config::{GlyphMode, LayoutSeams, RenderConfig};
use cc_pulseline::render::color::{strip_ansi, visible_width};
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{QuotaMetrics, RenderFrame, StdinPayload};
use serde_json::json;

const SEAM_R: char = '\u{e0b0}';
const HALF_R: char = '\u{2590}';
const HALF_L: char = '\u{258c}';
const TINT_FG_ESC: &str = "\x1b[38;5;16m"; // every reverse-video fill (head / headline)
const RAMP_BASE_BG: &str = "\x1b[48;5;235m"; // RampLevel::Base background

/// The precise byte signature of an ink-flagged ramp cell: a Base ramp bg
/// immediately followed by the role-colour fg. Seams can't produce this.
fn ink_marker(role_escape: &str) -> String {
    format!("{RAMP_BASE_BG}{role_escape}")
}

fn payload() -> StdinPayload {
    serde_json::from_str(&json!({"session_id": "rail-v2"}).to_string()).unwrap()
}

fn frame(effort: &str, ctx: u64, q5: f64, q7: f64, dirty: bool, quota: bool) -> RenderFrame {
    let mut f = RenderFrame::from_payload(&payload());
    f.line1.model = "Opus 4.6".into();
    f.line1.claude_code_version = "2.1.153".into();
    f.line1.project_path = "/home/me/cc-pulseline".into();
    f.line1.git_branch = "main".into();
    f.line1.effort_level = Some(effort.into());
    f.line1.git_dirty = dirty;
    if dirty {
        f.line1.git_modified = 2;
    }
    f.line3.context_used_percentage = Some(ctx);
    f.line3.context_window_size = Some(200_000);
    f.line3.total_cost_usd = Some(3.47);
    f.line3.total_duration_ms = Some(2_700_000);
    f.line3.input_tokens = Some(12_800);
    f.line3.output_tokens = Some(24_600);
    f.line3.cache_read_tokens = Some(68_200);
    if quota {
        f.quota = QuotaMetrics {
            five_hour_pct: Some(q5),
            five_hour_reset_minutes: Some(119),
            seven_day_pct: Some(q7),
            seven_day_reset_minutes: Some(5_760),
        };
    }
    f
}

fn rail_config() -> RenderConfig {
    RenderConfig {
        pane_style: LayoutStyle::Rail,
        glyph_mode: GlyphMode::Icon,
        color_enabled: true,
        pane_seams: LayoutSeams::Powerline,
        show_model: true,
        show_effort: true,
        show_project: true,
        show_git: true,
        show_context: true,
        show_cost: true,
        show_version: true,
        terminal_width: None,
        ..Default::default()
    }
}

fn render(f: &RenderFrame, c: &RenderConfig) -> Vec<String> {
    cc_pulseline::render::layout::render_frame(f, c)
}

fn has_pua(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0xE000..=0xF8FF).contains(&u)
            || (0xF0000..=0xFFFFD).contains(&u)
            || (0x10_0000..=0x10_FFFD).contains(&u)
    })
}

/// Column where the headline (right cluster) begins = the index just after the
/// longest run of spaces (the alignment gap). Used to verify the shared axis.
fn headline_start(row: &str) -> usize {
    let plain: Vec<char> = strip_ansi(row).chars().collect();
    let (mut best_end, mut best_len, mut i) = (0usize, 0usize, 0usize);
    while i < plain.len() {
        if plain[i] == ' ' {
            let start = i;
            while i < plain.len() && plain[i] == ' ' {
                i += 1;
            }
            if i - start >= best_len {
                best_len = i - start;
                best_end = i;
            }
        } else {
            i += 1;
        }
    }
    best_end
}

#[test]
fn default_renders_three_grouped_rows() {
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &rail_config());
    assert_eq!(lines.len(), 3, "identity · context · quota");
    let plain: Vec<String> = lines.iter().map(|l| strip_ansi(l)).collect();
    assert!(
        plain[0].contains("Opus 4.6") && plain[0].contains("$3.47"),
        "row1: {}",
        plain[0]
    );
    assert!(
        plain[1].contains("43%") && plain[1].contains("↓12.8k"),
        "row2: {}",
        plain[1]
    );
    assert!(
        plain[2].contains("5H") && plain[2].contains("7D"),
        "row3: {}",
        plain[2]
    );
}

#[test]
fn quota_less_fixture_renders_two_rows() {
    let lines = render(&frame("high", 43, 0.0, 0.0, false, false), &rail_config());
    assert_eq!(
        lines.len(),
        2,
        "no rate-limit data → quota row drops (not blank)"
    );
    let joined: String = lines
        .iter()
        .map(|l| strip_ansi(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !joined.contains("5H") && !joined.contains("7D"),
        "the dropped row is quota: {joined}"
    );
}

#[test]
fn context_row_drops_cleanly_when_no_api_data() {
    // Before the first API call: no ctx, no tokens → the context row would be
    // empty. It must drop, not render a blank line.
    let mut f = frame("high", 43, 62.0, 41.0, false, true);
    f.line3.context_used_percentage = None;
    f.line3.input_tokens = None;
    f.line3.output_tokens = None;
    f.line3.cache_read_tokens = None;
    let lines = render(&f, &rail_config());
    assert!(
        lines.iter().all(|l| visible_width(l) > 0),
        "no blank rows: {lines:?}"
    );
    assert!(lines.len() < 3, "the empty context row dropped: {lines:?}");
}

#[test]
fn max_total_lines_one_is_the_v1_fused_bar() {
    let mut config = rail_config();
    config.max_total_lines = Some(1);
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    assert_eq!(lines.len(), 1, "bottom rung is a single fused bar");
    let bar = strip_ansi(&lines[0]);
    for needle in ["Opus 4.6", "43%", "$3.47", "v2.1.153"] {
        assert!(bar.contains(needle), "fused bar missing {needle}: {bar}");
    }
}

#[test]
fn max_total_lines_two_keeps_identity_and_context_drops_quota() {
    let mut config = rail_config();
    config.max_total_lines = Some(2);
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    assert_eq!(lines.len(), 2);
    assert!(
        strip_ansi(&lines[0]).contains("Opus 4.6"),
        "row0 is identity"
    );
    assert!(strip_ansi(&lines[1]).contains("43%"), "row1 is context");
    let joined: String = lines
        .iter()
        .map(|l| strip_ansi(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !joined.contains("5H") && !joined.contains("7D"),
        "quota is the dropped row"
    );
}

#[test]
fn calm_fixture_has_zero_left_ink_flags() {
    // effort=low, ctx=40, quota 20/20, clean tree. Headlines (cost, 7d) and the
    // model head are still filled Tints — by design, NOT a rainbow. The test:
    // no LEFT cell carries a role-colour ink flag (precise ramp-base marker).
    let config = rail_config();
    let p = &config.palette;
    let s = render(&frame("low", 40, 20.0, 20.0, false, true), &config).join("\n");
    assert!(
        !s.contains(&ink_marker(p.color_for_effort_level("high"))),
        "no effort ink when calm"
    );
    assert!(
        !s.contains(&ink_marker(p.color_for_ctx_pct(72))),
        "no ctx ink when calm"
    );
    assert!(
        !s.contains(&ink_marker(&p.alert_orange)),
        "no git ink when clean"
    );
    assert!(
        s.contains(TINT_FG_ESC),
        "headline/head fills are always present"
    );
}

#[test]
fn high_fixture_lights_exactly_the_threshold_flags() {
    // effort=high, ctx=72 (crit), dirty → effort/ctx/git ink flags all light.
    let config = rail_config();
    let p = &config.palette;
    let s = render(&frame("high", 72, 30.0, 90.0, true, true), &config).join("\n");
    assert!(
        s.contains(&ink_marker(p.color_for_effort_level("high"))),
        "effort ink lights at high"
    );
    assert!(
        s.contains(&ink_marker(p.color_for_ctx_pct(72))),
        "ctx ink lights past threshold"
    );
    assert!(
        s.contains(&ink_marker(&p.alert_orange)),
        "git ink lights when dirty"
    );
}

#[test]
fn tokens_headline_is_not_tinted() {
    // Row 2's headline (tokens) has no band → stays ramp; the values render as
    // plain text, never a reverse-video fill.
    let config = rail_config();
    let plain = strip_ansi(&render(&frame("low", 40, 20.0, 20.0, false, true), &config)[1]);
    assert!(
        plain.contains("↓12.8k ↑24.6k"),
        "tokens render as text: {plain}"
    );
}

#[test]
fn headline_column_shares_an_axis_across_rows() {
    let mut config = rail_config();
    config.terminal_width = Some(120);
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    assert_eq!(lines.len(), 3);
    let starts: Vec<usize> = lines.iter().map(|l| headline_start(l)).collect();
    assert!(
        starts.iter().all(|&c| c == starts[0]),
        "headline left-edges align on a shared axis: {starts:?}"
    );
}

#[test]
fn ink_survives_the_ascii_floor() {
    let mut config = rail_config();
    config.glyph_mode = GlyphMode::Ascii;
    let p = config.palette.clone();
    let s = render(&frame("high", 72, 30.0, 90.0, true, true), &config).join("\n");
    assert!(!has_pua(&s), "ASCII floor has zero PUA: {s:?}");
    assert!(
        !s.contains("\x1b[48;5;"),
        "no background fills in the floor"
    );
    // No seams in the floor, so a role fg can only come from an ink flag.
    assert!(
        s.contains(p.color_for_ctx_pct(72)),
        "ctx ink survives as fg"
    );
    assert!(
        s.contains(p.alert_orange.as_str()),
        "git ink survives as fg"
    );
}

#[test]
fn blocks_tier_uses_half_block_not_seam_glyph() {
    let mut config = rail_config();
    config.pane_seams = LayoutSeams::Blocks;
    let s = render(&frame("high", 43, 62.0, 41.0, false, true), &config).join("\n");
    assert!(
        s.contains(HALF_R) || s.contains(HALF_L),
        "blocks emits half-blocks"
    );
    assert!(!s.contains(SEAM_R), "blocks emits no PUA seam glyph");
}

#[test]
fn below_min_width_falls_back_to_flat() {
    let mut config = rail_config();
    config.terminal_width = Some(40); // < pane_min_width (60) after margin
    let s = render(&frame("high", 43, 62.0, 41.0, false, true), &config).join("\n");
    assert!(
        !s.contains("\x1b[48;5;"),
        "flat fallback has no powerline bg fills"
    );
}

#[test]
fn narrow_width_drops_identity_cells_to_fit() {
    // A width above min_width but below the natural row width forces per-row
    // cell-drop (version first); the row must end up within budget.
    let mut config = rail_config();
    config.terminal_width = Some(80);
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    for l in &lines {
        assert!(
            visible_width(l) <= 76,
            "row fits the cc-margin-adjusted width: {}",
            visible_width(l)
        );
    }
    assert!(
        strip_ansi(&lines[0]).contains("Opus 4.6"),
        "model never drops"
    );
}
