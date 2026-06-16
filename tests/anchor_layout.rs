//! `anchor` layout — hero capsule + dim trail (height 1, stdin-only).
//!
//! Covers the design's Must/Should checks from
//! `designs/powerline-rail-anchor.md`:
//! - height 1 from stdin fields only
//! - a reverse-video capsule (model) + a dim trail
//! - exactly one trail item lights up (shape = identity, colour = state)
//! - ASCII floor degrades the capsule to `[model]`, no PUA
//! - blocks mode swaps the caps for half-blocks (no PUA seam)

use cc_pulseline::config::{GlyphMode, LayoutSeams, RenderConfig};
use cc_pulseline::render::color::strip_ansi;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{RenderFrame, StdinPayload};
use serde_json::json;

const SEAM_R: char = '\u{e0b0}';
const SEAM_L: char = '\u{e0b2}';
const HALF_R: char = '\u{2590}';
const HALF_L: char = '\u{258c}';
const TINT_FG_ESC: &str = "\x1b[38;5;16m"; // reverse-video capsule text

fn payload() -> StdinPayload {
    let input = json!({
        "session_id": "anchor-test",
        "model": {"display_name": "Opus 4.6"},
        "version": "2.1.153",
        "workspace": {"current_dir": "/home/me/cc-pulseline"},
        "context_window": {"context_window_size": 200000, "used_percentage": 43},
        "cost": {"total_cost_usd": 3.47}
    })
    .to_string();
    serde_json::from_str(&input).unwrap()
}

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
fn renders_single_row_with_capsule_and_trail() {
    let lines = render(&default_frame(), &anchor_config());
    assert_eq!(lines.len(), 1, "anchor is height 1");
    let plain = strip_ansi(&lines[0]);
    assert!(plain.contains("Opus 4.6"), "hero model present: {plain}");
    assert!(plain.contains("43%"), "trail ctx present: {plain}");
    assert!(plain.contains(" · "), "trail joined by ` · `");
}

#[test]
fn capsule_is_reverse_video_with_caps() {
    let lines = render(&default_frame(), &anchor_config());
    let s = &lines[0];
    assert!(
        s.contains(TINT_FG_ESC),
        "capsule body is reverse-video (term-bg text)"
    );
    assert!(
        s.contains(SEAM_L) || s.contains(SEAM_R),
        "angled caps use Powerline cap glyphs"
    );
}

#[test]
fn exactly_one_trail_item_lights_up() {
    // Default fixture: effort=high (lights), ctx=43% calm (dim), clean tree.
    // Isolation, not presence: the effort role colour appears exactly once and
    // NO other signal colour (ctx warn/crit, git orange) appears at all.
    let config = anchor_config();
    let p = &config.palette;
    let lines = render(&default_frame(), &config);
    let s = &lines[0];
    let effort_color = p.color_for_effort_level("high");
    assert_eq!(
        s.matches(effort_color).count(),
        1,
        "effort=high is the single lit trail item: {s}"
    );
    // ctx is calm and the tree is clean → neither lights up.
    assert!(
        !s.contains(p.color_for_ctx_pct(72)),
        "calm ctx does not use a ctx signal colour"
    );
    assert!(
        !s.contains(p.alert_orange.as_str()),
        "clean tree does not light the git count"
    );
}

#[test]
fn second_signal_lights_only_when_its_state_crosses() {
    // Pushing ctx over threshold adds a second lit item — proving the test
    // above isn't tautological (the rule is `tint ⟺ threshold crossed`).
    let mut f = default_frame();
    f.line3.context_used_percentage = Some(72); // crit → lights
    let config = anchor_config();
    let s = &render(&f, &config)[0];
    assert!(
        s.contains(config.palette.color_for_ctx_pct(72)),
        "ctx over threshold now lights in its crit colour: {s}"
    );
}

#[test]
fn ascii_floor_capsule_is_bracketed_no_pua() {
    let mut config = anchor_config();
    config.glyph_mode = GlyphMode::Ascii;
    let lines = render(&default_frame(), &config);
    let s = &lines[0];
    assert!(!has_pua(s), "ASCII floor has zero PUA: {s:?}");
    let plain = strip_ansi(s);
    assert!(
        plain.contains("[Opus 4.6]"),
        "capsule degrades to [model]: {plain}"
    );
}

#[test]
fn blocks_mode_caps_are_half_blocks_no_seam_glyph() {
    let mut config = anchor_config();
    config.pane_seams = LayoutSeams::Blocks;
    let lines = render(&default_frame(), &config);
    let s = &lines[0];
    assert!(
        s.contains(HALF_R) || s.contains(HALF_L),
        "blocks caps are half-blocks: {s}"
    );
    assert!(
        !s.contains(SEAM_L) && !s.contains(SEAM_R),
        "blocks mode emits no PUA cap glyph"
    );
}

#[test]
fn dirty_tree_lights_only_the_modified_count() {
    let mut f = default_frame();
    f.line1.effort_level = Some("low".into()); // remove the effort signal
    f.line1.git_modified = 3;
    let config = anchor_config();
    let p = &config.palette;
    let lines = render(&f, &config);
    let s = &lines[0];
    // Isolation: orange appears exactly once (the `~3`), not on the branch.
    assert_eq!(
        s.matches(p.alert_orange.as_str()).count(),
        1,
        "alert_orange lights only the modified count, not the branch: {s}"
    );
    assert!(
        s.contains(&format!("{} ~3", p.alert_orange)),
        "the lit fragment is the `~3` count"
    );
    // effort is low and ctx is calm → no other trail item lights.
    assert!(
        !s.contains(p.color_for_effort_level("high")),
        "low effort stays dim"
    );
    assert!(!s.contains(p.color_for_ctx_pct(72)), "calm ctx stays dim");
}

#[test]
fn all_absent_fields_renders_one_row() {
    // Payload with only a session id — every optional field is None. The row
    // must stay height 1 with no panic, no empty capsule, no double seam.
    let input = json!({ "session_id": "x" }).to_string();
    let payload: StdinPayload = serde_json::from_str(&input).unwrap();
    let frame = RenderFrame::from_payload(&payload);
    let lines = render(&frame, &anchor_config());
    assert_eq!(
        lines.len(),
        1,
        "anchor returns one row even with all fields absent"
    );
}

#[test]
fn below_min_width_falls_back_to_flat() {
    let mut config = anchor_config();
    config.terminal_width = Some(40); // < pane_min_width (60) after margin
    let lines = render(&default_frame(), &config);
    assert!(
        lines.len() >= 2,
        "narrow terminal bypasses to flat `none`: {lines:?}"
    );
}

#[test]
fn narrow_width_drops_trail_keeps_capsule_and_ctx() {
    let mut config = anchor_config();
    config.terminal_width = Some(72); // forces trail drops, still > min_width
    let lines = render(&default_frame(), &config);
    assert_eq!(lines.len(), 1, "still one row");
    let plain = strip_ansi(&lines[0]);
    assert!(
        plain.contains("Opus 4.6"),
        "capsule (hero) never drops: {plain}"
    );
    assert!(plain.contains("43%"), "ctx (signal) never drops: {plain}");
    assert!(
        !plain.contains("v2.1.153"),
        "version trail item drops first: {plain}"
    );
}
