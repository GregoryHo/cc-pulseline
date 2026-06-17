//! `anchor` v2 — three grouped capsule+trail rows.
//!
//! Covers the Must/Should of `designs/rail-anchor-grouped-rows.md`:
//! - default = 3 rows; quota-less = 2; `max_total_lines = 1` = the v1 single bar
//! - rounded capsule heroes (always filled); row 2 carries the shipped gauge
//! - trail flags (effort / git) light only past threshold (anti-rainbow)
//! - ` ❯ ` (PL_TICK) trail separator in Powerline, ` · ` in the ASCII floor
//! - capsule degrades to `[model]`, no PUA; blocks tier uses half-blocks
//!
//! Flag detection is precise: a lit trail flag is `38;5;<role>` immediately
//! followed by the cell's icon glyph. A capsule cap also emits a role colour as
//! fg, but followed by a CAP glyph — never the cell icon — so the marker can't
//! false-match a cap leak.

use cc_pulseline::config::{GlyphMode, LayoutSeams, RenderConfig};
use cc_pulseline::render::color::strip_ansi;
use cc_pulseline::render::icons::{ICON_EFFORT, ICON_GIT};
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{QuotaMetrics, RenderFrame, StdinPayload};
use serde_json::json;

const SEAM_R: char = '\u{e0b0}';
const CAP_ROUND_L: char = '\u{e0b6}';
const PL_TICK: char = '\u{e0b1}';
const HALF_R: char = '\u{2590}';
const HALF_L: char = '\u{258c}';
const TINT_FG_ESC: &str = "\x1b[38;5;16m"; // reverse-video capsule body

fn payload() -> StdinPayload {
    serde_json::from_str(&json!({"session_id": "anchor-v2"}).to_string()).unwrap()
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

fn anchor_config() -> RenderConfig {
    RenderConfig {
        pane_style: LayoutStyle::Anchor,
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

/// A lit trail flag: the role fg escape immediately followed by the cell icon.
fn flag_marker(role_escape: &str, icon: &str) -> String {
    format!("{role_escape}{icon}")
}

#[test]
fn default_renders_three_capsule_rows() {
    let lines = render(
        &frame("high", 43, 62.0, 41.0, false, true),
        &anchor_config(),
    );
    assert_eq!(lines.len(), 3);
    let plain: Vec<String> = lines.iter().map(|l| strip_ansi(l)).collect();
    assert!(plain[0].contains("Opus 4.6"), "row1 hero: {}", plain[0]);
    assert!(
        plain[1].contains("CTX 43%") && plain[1].contains("in 12.8k"),
        "row2: {}",
        plain[1]
    );
    assert!(
        plain[2].contains("5H 62%") && plain[2].contains("7D 41%"),
        "row3: {}",
        plain[2]
    );
}

#[test]
fn quota_less_fixture_renders_two_rows() {
    let lines = render(&frame("high", 43, 0.0, 0.0, false, false), &anchor_config());
    assert_eq!(lines.len(), 2);
    let joined: String = lines
        .iter()
        .map(|l| strip_ansi(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !joined.contains("5H") && !joined.contains("7D"),
        "the dropped row is quota"
    );
}

#[test]
fn context_row_drops_cleanly_when_no_api_data() {
    let mut f = frame("high", 43, 62.0, 41.0, false, true);
    f.line3.context_used_percentage = None;
    let lines = render(&f, &anchor_config());
    assert!(
        lines.iter().all(|l| !l.is_empty()),
        "no blank rows: {lines:?}"
    );
    assert!(lines.len() < 3, "the empty context row dropped: {lines:?}");
}

#[test]
fn max_total_lines_one_is_single_capsule_bar() {
    let mut config = anchor_config();
    config.max_total_lines = Some(1);
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    assert_eq!(lines.len(), 1);
    let bar = strip_ansi(&lines[0]);
    assert!(
        bar.contains("Opus 4.6") && bar.contains("43%"),
        "fused capsule bar: {bar}"
    );
}

#[test]
fn max_total_lines_two_keeps_identity_and_context() {
    let mut config = anchor_config();
    config.max_total_lines = Some(2);
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    assert_eq!(lines.len(), 2);
    assert!(strip_ansi(&lines[1]).contains("CTX 43%"), "row1 is context");
    let joined: String = lines
        .iter()
        .map(|l| strip_ansi(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!joined.contains("5H 62%"), "quota is the dropped row");
}

#[test]
fn capsules_are_rounded_and_filled() {
    let s = render(
        &frame("high", 43, 62.0, 41.0, false, true),
        &anchor_config(),
    )
    .join("\n");
    assert!(
        s.contains(CAP_ROUND_L),
        "v2 anchors with rounded caps (e0b6)"
    );
    assert!(
        !s.contains(SEAM_R),
        "rounded caps, not the angled rail seam"
    );
    assert!(
        s.contains(TINT_FG_ESC),
        "capsule body is reverse-video (always filled)"
    );
}

#[test]
fn row_two_carries_the_shipped_gauge() {
    let plain = strip_ansi(
        &render(
            &frame("high", 43, 62.0, 41.0, false, true),
            &anchor_config(),
        )[1],
    );
    assert!(
        plain.contains('▰') || plain.contains('─') || plain.contains('·'),
        "context row reuses widgets::gauge: {plain}"
    );
}

#[test]
fn trail_separator_is_pl_tick_in_powerline() {
    let s = render(
        &frame("high", 43, 62.0, 41.0, false, true),
        &anchor_config(),
    )
    .join("\n");
    assert!(s.contains(PL_TICK), "powerline trail uses the thin tick");
}

#[test]
fn calm_fixture_has_no_lit_trail_flags() {
    let config = anchor_config();
    let p = &config.palette;
    let s = render(&frame("low", 40, 20.0, 20.0, false, true), &config).join("\n");
    assert!(
        !s.contains(&flag_marker(p.color_for_effort_level("high"), ICON_EFFORT)),
        "no effort flag when calm"
    );
    assert!(
        !s.contains(&flag_marker(&p.alert_orange, ICON_GIT)),
        "no git flag when clean"
    );
    assert!(s.contains(TINT_FG_ESC), "capsule heroes are still filled");
}

#[test]
fn high_fixture_lights_the_trail_flags() {
    let config = anchor_config();
    let p = &config.palette;
    let s = render(&frame("high", 72, 30.0, 90.0, true, true), &config).join("\n");
    assert!(
        s.contains(&flag_marker(p.color_for_effort_level("high"), ICON_EFFORT)),
        "effort flag lights at high"
    );
    assert!(
        s.contains(&flag_marker(&p.alert_orange, ICON_GIT)),
        "git flag lights when dirty"
    );
}

#[test]
fn ascii_floor_brackets_capsule_no_pua() {
    let mut config = anchor_config();
    config.glyph_mode = GlyphMode::Ascii;
    let lines = render(&frame("high", 43, 62.0, 41.0, false, true), &config);
    let s = lines.join("\n");
    assert!(!has_pua(&s), "ASCII floor has zero PUA: {s:?}");
    assert!(
        strip_ansi(&lines[0]).contains("[Opus 4.6]"),
        "capsule degrades to [model]"
    );
    assert!(s.contains(" · "), "trail separator degrades to a middot");
}

#[test]
fn blocks_tier_uses_half_block_caps() {
    let mut config = anchor_config();
    config.pane_seams = LayoutSeams::Blocks;
    let s = render(&frame("high", 43, 62.0, 41.0, false, true), &config).join("\n");
    assert!(
        s.contains(HALF_R) || s.contains(HALF_L),
        "blocks caps are half-blocks"
    );
    assert!(
        !s.contains(SEAM_R) && !s.contains(CAP_ROUND_L),
        "no PUA cap glyphs in blocks"
    );
}
