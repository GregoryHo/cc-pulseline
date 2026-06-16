//! `rail` layout — one connected Powerline bar (height 1, stdin-only).
//!
//! Covers the design's Must/Should checks from
//! `designs/powerline-rail-anchor.md`:
//! - height 1 from stdin fields only
//! - exactly one tinted segment in the default fixture (no rainbow)
//! - seam glyph present in powerline mode, half-block in blocks mode
//! - ASCII floor has no PUA
//! - cell-drop order under width pressure (model + ctx survive longest)
//! - a dirty tree tints only the `~N` modified count

use cc_pulseline::config::{GlyphMode, LayoutSeams, RenderConfig};
use cc_pulseline::render::color::{strip_ansi, visible_width};
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{RenderFrame, StdinPayload};
use serde_json::json;

const SEAM_R: char = '\u{e0b0}';
const SEAM_L: char = '\u{e0b2}';
const HALF_R: char = '\u{2590}';
const HALF_L: char = '\u{258c}';
/// Reverse-video text colour emitted once per tinted segment body.
const TINT_FG_ESC: &str = "\x1b[38;5;16m";

fn payload() -> StdinPayload {
    let input = json!({
        "session_id": "rail-test",
        "model": {"display_name": "Opus 4.6"},
        "version": "2.1.153",
        "workspace": {"current_dir": "/home/me/cc-pulseline"},
        "context_window": {"context_window_size": 200000, "used_percentage": 43},
        "cost": {"total_cost_usd": 3.47}
    })
    .to_string();
    serde_json::from_str(&input).unwrap()
}

/// Default fixture: effort = high (the one tinted segment), ctx calm (43%),
/// clean tree.
fn default_frame() -> RenderFrame {
    let mut f = RenderFrame::from_payload(&payload());
    f.line1.model = "Opus 4.6".into();
    f.line1.claude_code_version = "2.1.153".into();
    f.line1.project_path = "/home/me/cc-pulseline".into();
    f.line1.git_branch = "main".into();
    f.line1.effort_level = Some("high".into());
    f.line3.context_used_percentage = Some(43);
    f.line3.context_window_size = Some(200000);
    f.line3.total_cost_usd = Some(3.47);
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

fn render(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    cc_pulseline::render::layout::render_frame(frame, config)
}

fn has_pua(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0xE000..=0xF8FF).contains(&u)
            || (0xF0000..=0xFFFFD).contains(&u)
            || (0x10_0000..=0x10_FFFD).contains(&u)
    })
}

#[test]
fn renders_single_row_from_stdin_fields() {
    let lines = render(&default_frame(), &rail_config());
    assert_eq!(lines.len(), 1, "rail is height 1");
    let bar = strip_ansi(&lines[0]);
    assert!(bar.contains("Opus 4.6"), "model present: {bar}");
    assert!(bar.contains("43%"), "ctx present: {bar}");
    assert!(bar.contains("v2.1.153"), "version present: {bar}");
}

#[test]
fn exactly_one_tinted_segment_in_default_fixture() {
    let lines = render(&default_frame(), &rail_config());
    // Each reverse-video (tinted) segment body emits exactly one TINT_FG. In
    // the default fixture only effort=high crosses threshold → exactly one.
    let tinted = lines[0].matches(TINT_FG_ESC).count();
    assert_eq!(
        tinted, 1,
        "exactly one tinted segment, no rainbow: {}",
        lines[0]
    );
}

#[test]
fn calm_session_has_zero_tints() {
    // effort=low, ctx low, clean tree → the whole bar rides the ramp.
    let mut f = default_frame();
    f.line1.effort_level = Some("low".into());
    f.line3.context_used_percentage = Some(20);
    let lines = render(&f, &rail_config());
    assert_eq!(
        lines[0].matches(TINT_FG_ESC).count(),
        0,
        "no tints when calm"
    );
}

#[test]
fn ctx_tints_when_over_threshold() {
    let mut f = default_frame();
    f.line1.effort_level = Some("low".into()); // not a signal
    f.line3.context_used_percentage = Some(72); // crit → tints
    let lines = render(&f, &rail_config());
    assert_eq!(
        lines[0].matches(TINT_FG_ESC).count(),
        1,
        "ctx is the lone tint"
    );
}

#[test]
fn seam_glyph_present_in_powerline_mode() {
    let lines = render(&default_frame(), &rail_config());
    let s = &lines[0];
    assert!(
        s.contains(SEAM_R) || s.contains(SEAM_L),
        "powerline mode emits a PUA seam glyph"
    );
    assert!(
        !s.contains(HALF_R) && !s.contains(HALF_L),
        "no half-block in powerline mode"
    );
}

#[test]
fn blocks_mode_uses_half_block_not_seam_glyph() {
    let mut config = rail_config();
    config.pane_seams = LayoutSeams::Blocks;
    let lines = render(&default_frame(), &config);
    let s = &lines[0];
    assert!(
        s.contains(HALF_R) || s.contains(HALF_L),
        "blocks mode emits a unicode half-block: {s}"
    );
    assert!(
        !s.contains(SEAM_R) && !s.contains(SEAM_L),
        "blocks mode emits NO PUA seam glyph"
    );
}

#[test]
fn ascii_floor_has_no_pua() {
    let mut config = rail_config();
    config.glyph_mode = GlyphMode::Ascii; // display.icons = false
    let lines = render(&default_frame(), &config);
    let s = &lines[0];
    assert!(
        !has_pua(s),
        "ASCII floor must contain zero PUA glyphs: {s:?}"
    );
    assert!(s.contains(" | "), "ASCII floor joins cells with ` | `");
    assert!(s.contains("M:"), "ASCII floor uses ascii label prefixes");
}

#[test]
fn no_color_floor_drops_fills_and_seams() {
    let mut config = rail_config();
    config.color_enabled = false;
    let lines = render(&default_frame(), &config);
    let s = &lines[0];
    assert!(
        !s.contains("\x1b[48;5;"),
        "no background fills without colour"
    );
    assert!(
        !s.contains(SEAM_R) && !s.contains(SEAM_L),
        "no seam glyphs in the floor"
    );
    assert!(s.contains(" | "), "collapses to the ` | ` separator floor");
}

#[test]
fn seam_string_visible_width_fits_terminal() {
    // The new escape shapes (48;5 bg, mid-row resets, fg+bg seam pairs) must
    // still measure correctly via strip_ansi so width math holds.
    let mut config = rail_config();
    config.terminal_width = Some(120);
    let lines = render(&default_frame(), &config);
    let w = visible_width(&lines[0]);
    assert!(w <= 116, "bar fits the cc-margin-adjusted width (got {w})");
}

#[test]
fn narrow_width_drops_version_first_keeps_model_and_ctx() {
    let mut config = rail_config();
    config.terminal_width = Some(70); // forces a few drops, still > min_width
    let lines = render(&default_frame(), &config);
    assert_eq!(lines.len(), 1, "still one row");
    let bar = strip_ansi(&lines[0]);
    assert!(bar.contains("Opus 4.6"), "model never drops: {bar}");
    assert!(bar.contains("43%"), "ctx never drops: {bar}");
    assert!(
        !bar.contains("v2.1.153"),
        "version drops first under pressure: {bar}"
    );
}

#[test]
fn below_min_width_falls_back_to_flat() {
    let mut config = rail_config();
    config.terminal_width = Some(40); // < pane_min_width (60) after margin
    let lines = render(&default_frame(), &config);
    // Flat `none` renders multiple rows and never emits the rail's bg fills.
    let joined = lines.join("\n");
    assert!(
        !joined.contains("\x1b[48;5;241m"),
        "no rail ramp fill in fallback"
    );
    assert!(lines.len() >= 2, "flat fallback renders >1 row: {lines:?}");
}

#[test]
fn dirty_tree_tints_only_the_modified_count() {
    let mut f = default_frame();
    f.line1.git_modified = 2;
    f.line1.git_added = 1;
    let config = rail_config();
    let lines = render(&f, &config);
    let s = &lines[0];
    // git stays a ramp segment (not a reverse-video tint): the tint count is
    // unchanged — still just effort.
    assert_eq!(
        s.matches(TINT_FG_ESC).count(),
        1,
        "git dirtiness adds no tinted segment"
    );
    // The `~2` modified count is painted alert_orange; the branch is not.
    let orange = &config.palette.alert_orange;
    assert!(
        s.contains(&format!("{orange}~2")),
        "modified count tints alert_orange"
    );
    let bar = strip_ansi(s);
    assert!(bar.contains("main"), "branch text present (on the ramp)");
    assert!(bar.contains("+1"), "staged count present");
}

#[test]
fn all_absent_fields_renders_one_row() {
    // Only a session id — every optional field is None. The bar stays height 1
    // with no panic and no stray seam from a missing cell.
    let input = json!({ "session_id": "x" }).to_string();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();
    let frame = RenderFrame::from_payload(&payload);
    let lines = render(&frame, &rail_config());
    assert_eq!(
        lines.len(),
        1,
        "rail returns one row even with all fields absent"
    );
}
