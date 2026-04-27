//! Verifies that v2 layouts honor the `display.icons` axis and the new
//! `show_ctx_sparkline` toggle, plus that Q7d renders in cockpit + console
//! when the seven-day quota toggle is on.
//!
//! These pin the contract spelled out in `designs/style-to-layout-taxonomy.md`:
//! every (layout × display) pair must compose without leaking hardcoded
//! glyphs through.

use cc_pulseline::config::{build_render_config, GlyphMode, PulselineConfig, RenderConfig};
use cc_pulseline::render::color::resolve_palette;
use cc_pulseline::render::icons::ICON_AGENT;
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::render::pane::PaneStyle;
use cc_pulseline::types::{AgentSummary, QuotaMetrics, RenderFrame};

fn frame_with_agent_and_quota() -> RenderFrame {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus 4.7".to_string();
    f.line1.git_branch = "feat/x".to_string();
    f.line1.project_path = "~/proj".to_string();
    f.line3.context_window_size = Some(200_000);
    f.line3.context_used_percentage = Some(43);
    f.line3.input_tokens = Some(10);
    f.line3.output_tokens = Some(20);
    f.line3.total_cost_usd = Some(3.50);
    f.line3.total_duration_ms = Some(60_000 * 30); // 30 min → $7/h
    f.ctx_history = vec![10, 20, 30, 35, 40, 43];
    f.agents.push(AgentSummary {
        id: "a1".to_string(),
        description: "Investigate logic".to_string(),
        agent_type: Some("Explore".to_string()),
        started_at: None,
        model: None,
        completed_at: None,
        message_id: None,
    });
    f.quota = QuotaMetrics {
        five_hour_pct: Some(75.0),
        five_hour_reset_minutes: Some(120),
        seven_day_pct: Some(40.0),
        seven_day_reset_minutes: Some(60 * 24 * 3),
    };
    f
}

fn cfg_for(layout: PaneStyle, icons: bool, width: usize) -> RenderConfig {
    RenderConfig {
        glyph_mode: if icons {
            GlyphMode::Icon
        } else {
            GlyphMode::Ascii
        },
        color_enabled: false,
        terminal_width: Some(width + 4), // +cc_margin so layout actually has `width`
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        pane_style: layout,
        pane_cc_margin: 4,
        // v2 default: opt-in config row hidden
        show_claude_md: false,
        show_rules: false,
        show_memory: false,
        show_hooks: false,
        show_mcp: false,
        show_skills: false,
        show_plugins: false,
        ..RenderConfig::default()
    }
}

// ── Agent prefix: NF glyph under icons=true, "A:" text under icons=false ──

#[test]
fn cockpit_agent_uses_nf_glyph_when_icons_on() {
    let f = frame_with_agent_and_quota();
    let cfg = cfg_for(PaneStyle::V2Cockpit, true, 140);
    let lines = render_frame(&f, &cfg);
    let activity = lines
        .iter()
        .find(|l| l.contains("Explore"))
        .expect("activity line");
    assert!(
        !activity.contains("A:"),
        "agent should not show literal 'A:' in icon mode: {activity:?}"
    );
    assert!(
        activity.contains(ICON_AGENT),
        "agent prefix should be the ICON_AGENT glyph: {activity:?}"
    );
}

#[test]
fn cockpit_agent_uses_text_prefix_when_icons_off() {
    let f = frame_with_agent_and_quota();
    let cfg = cfg_for(PaneStyle::V2Cockpit, false, 140);
    let lines = render_frame(&f, &cfg);
    let activity = lines
        .iter()
        .find(|l| l.contains("Explore"))
        .expect("activity line");
    assert!(
        activity.contains("A:"),
        "agent should show literal 'A:' in ASCII mode: {activity:?}"
    );
}

#[test]
fn console_agent_uses_text_prefix_when_icons_off() {
    let f = frame_with_agent_and_quota();
    let cfg = cfg_for(PaneStyle::V2Console, false, 140);
    let lines = render_frame(&f, &cfg);
    let agent_line = lines
        .iter()
        .find(|l| l.contains("Explore"))
        .expect("agent line");
    assert!(
        agent_line.contains("A:"),
        "console agent should show 'A:' in ASCII mode: {agent_line:?}"
    );
}

// ── Cost cell: arc under icons=true, rate text under icons=false ──

#[test]
fn cockpit_cost_uses_arc_when_icons_on() {
    let f = frame_with_agent_and_quota();
    let cfg = cfg_for(PaneStyle::V2Cockpit, true, 140);
    let lines = render_frame(&f, &cfg);
    let cluster = lines.iter().find(|l| l.contains("$3.50")).expect("cluster");
    let arc_glyphs = ['\u{25CB}', '\u{25D4}', '\u{25D1}', '\u{25D5}', '\u{25CF}'];
    assert!(
        cluster.chars().any(|c| arc_glyphs.contains(&c)),
        "cost cell should contain an arc glyph in icon mode: {cluster:?}"
    );
}

#[test]
fn cockpit_cost_uses_rate_text_when_icons_off() {
    let f = frame_with_agent_and_quota();
    let cfg = cfg_for(PaneStyle::V2Cockpit, false, 140);
    let lines = render_frame(&f, &cfg);
    let cluster = lines.iter().find(|l| l.contains("$3.50")).expect("cluster");
    let arc_glyphs = ['\u{25CB}', '\u{25D4}', '\u{25D1}', '\u{25D5}', '\u{25CF}'];
    assert!(
        !cluster.chars().any(|c| arc_glyphs.contains(&c)),
        "ascii mode must drop the arc glyph: {cluster:?}"
    );
    assert!(
        cluster.contains("/h)"),
        "ascii mode should append rate text: {cluster:?}"
    );
}

// ── Sparkline: hidden by default, present only when toggle on AND icons on ──

#[test]
fn cockpit_no_sparkline_when_toggle_off() {
    let f = frame_with_agent_and_quota();
    let cfg = cfg_for(PaneStyle::V2Cockpit, true, 140);
    assert!(!cfg.show_ctx_sparkline);
    let lines = render_frame(&f, &cfg);
    for l in &lines {
        assert!(
            !l.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
            "no braille expected when sparkline toggle off: {l:?}"
        );
    }
}

#[test]
fn cockpit_sparkline_appears_when_toggle_on_and_icons_on() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(PaneStyle::V2Cockpit, true, 140);
    cfg.show_ctx_sparkline = true;
    let lines = render_frame(&f, &cfg);
    let cluster = lines.iter().find(|l| l.contains("CTX")).expect("cluster");
    assert!(
        cluster
            .chars()
            .any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
        "sparkline should appear: {cluster:?}"
    );
}

#[test]
fn cockpit_sparkline_hidden_when_toggle_on_but_icons_off() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(PaneStyle::V2Cockpit, false, 140);
    cfg.show_ctx_sparkline = true;
    let lines = render_frame(&f, &cfg);
    for l in &lines {
        assert!(
            !l.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
            "ASCII mode must hide braille even when toggle on: {l:?}"
        );
    }
}

#[test]
fn v1_sections_layout_renders_sparkline_when_toggle_on() {
    // The sparkline is layout-agnostic — any layout that emits the CTX
    // segment should pick it up when the user opts in.
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(PaneStyle::V1None, true, 140);
    cfg.show_ctx_sparkline = true;
    let lines = render_frame(&f, &cfg);
    // v1 L3 is the third line; identify it by the "(86.0k/200.0k)" pattern.
    let ctx = lines.iter().find(|l| l.contains("/200.0k")).expect("L3");
    assert!(
        ctx.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
        "v1 layout should show sparkline when opt-in: {ctx:?}"
    );
}

// ── Q7d renders in cockpit cluster + console quota row when toggle on ──

#[test]
fn cockpit_renders_q7d_alongside_q5h() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(PaneStyle::V2Cockpit, true, 140);
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    cfg.show_quota_seven_day = true;
    let lines = render_frame(&f, &cfg);
    let cluster = lines
        .iter()
        .find(|l| l.contains("Q5h"))
        .expect("cluster with quota");
    assert!(cluster.contains("Q5h"), "Q5h missing: {cluster:?}");
    assert!(cluster.contains("Q7d"), "Q7d missing: {cluster:?}");
}

#[test]
fn console_renders_q7d_alongside_q5h() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(PaneStyle::V2Console, true, 160);
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    cfg.show_quota_seven_day = true;
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    assert!(blob.contains("Q5h"), "Q5h missing in console: {blob}");
    assert!(blob.contains("Q7d"), "Q7d missing in console: {blob}");
}

// ── TOML rename: `[layout]` parses, `[pane]` no longer recognized ──

#[test]
fn layout_section_parses() {
    let toml_input = r#"
[layout]
name = "console"
"#;
    let parsed: PulselineConfig = toml::from_str(toml_input).expect("parse");
    let cfg = build_render_config(&parsed);
    assert_eq!(cfg.pane_style, PaneStyle::V2Console);
}

#[test]
fn old_pane_section_is_silently_ignored_after_rename() {
    // Hard cut: `[pane]` is no longer a recognized section. Its content is
    // dropped (TOML allows unknown tables), and we fall back to defaults.
    let toml_input = r#"
[pane]
style = "console"
"#;
    let parsed: PulselineConfig = toml::from_str(toml_input).expect("parse");
    let cfg = build_render_config(&parsed);
    // Default layout is "none" → V1None.
    assert_eq!(cfg.pane_style, PaneStyle::V1None);
}
