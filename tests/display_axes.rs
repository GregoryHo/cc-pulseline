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
use cc_pulseline::render::pane::LayoutStyle;
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

fn cfg_for(layout: LayoutStyle, icons: bool, width: usize) -> RenderConfig {
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
    let cfg = cfg_for(LayoutStyle::Cockpit, true, 140);
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
    let cfg = cfg_for(LayoutStyle::Cockpit, false, 140);
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
    let cfg = cfg_for(LayoutStyle::Console, false, 140);
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
    let cfg = cfg_for(LayoutStyle::Cockpit, true, 140);
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
    let cfg = cfg_for(LayoutStyle::Cockpit, false, 140);
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

// ── Sparkline: composed via `context_visual = "...+sparkline"`, gated by ICON ──

#[test]
fn cockpit_no_sparkline_when_context_visual_drops_it() {
    // Cockpit's layout default is "gauge+sparkline"; explicitly drop sparkline
    // by setting context_visual = "gauge".
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Cockpit, true, 140);
    cfg.context_visual = "gauge".to_string();
    let lines = render_frame(&f, &cfg);
    for l in &lines {
        assert!(
            !l.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
            "no braille expected when context_visual = \"gauge\": {l:?}"
        );
    }
}

#[test]
fn cockpit_sparkline_appears_with_layout_default() {
    // cfg_for leaves context_visual = "" → layout default kicks in. Cockpit's
    // default is "gauge+sparkline", so the braille trend should show up.
    let f = frame_with_agent_and_quota();
    let cfg = cfg_for(LayoutStyle::Cockpit, true, 140);
    assert!(
        cfg.context_visual.is_empty(),
        "test premise: empty user value"
    );
    let lines = render_frame(&f, &cfg);
    let cluster = lines.iter().find(|l| l.contains("CTX")).expect("cluster");
    assert!(
        cluster
            .chars()
            .any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
        "sparkline should appear from cockpit's layout default: {cluster:?}"
    );
}

#[test]
fn cockpit_sparkline_hidden_in_ascii_mode_even_when_spec_includes_it() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Cockpit, false, 140);
    cfg.context_visual = "gauge+sparkline".to_string();
    let lines = render_frame(&f, &cfg);
    for l in &lines {
        assert!(
            !l.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
            "ASCII mode must hide braille even when spec opts in: {l:?}"
        );
    }
}

#[test]
fn v1_layouts_render_sparkline_when_context_visual_includes_it() {
    // The sparkline is layout-agnostic — any layout that emits the CTX
    // segment should pick it up when the user opts in. Flat layouts default
    // to "text" so we set it explicitly here. Use "text+sparkline" so the
    // legacy `(used/total)` annotation still appears alongside the trend.
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::None, true, 140);
    cfg.context_visual = "text+sparkline".to_string();
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
fn cockpit_renders_seven_day_alongside_five_hour() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Cockpit, true, 140);
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    cfg.show_quota_seven_day = true;
    let lines = render_frame(&f, &cfg);
    let cluster = lines
        .iter()
        .find(|l| l.contains("5h "))
        .expect("cluster with quota");
    // Bare `5h ` / `7d ` labels (no `Q` prefix — the cluster-row position
    // and reset annotation provide enough context).
    assert!(cluster.contains("5h "), "5h missing: {cluster:?}");
    assert!(cluster.contains("7d "), "7d missing: {cluster:?}");
}

#[test]
fn console_renders_seven_day_alongside_five_hour() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Console, true, 160);
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    cfg.show_quota_seven_day = true;
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    assert!(blob.contains("5h "), "5h missing in console: {blob}");
    assert!(blob.contains("7d "), "7d missing in console: {blob}");
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
    assert_eq!(cfg.pane_style, LayoutStyle::Console);
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
    assert_eq!(cfg.pane_style, LayoutStyle::None);
}

// ── ASCII contract: `display.icons = false` produces zero Unicode block
//    chars across every layout. This is the catch-net for any widget call
//    site that forgot to pass `glyph_mode`. ──

const UNICODE_BLOCKS: &[char] = &[
    '\u{2588}', // █
    '\u{2589}', // ▉
    '\u{258A}', // ▊
    '\u{258B}', // ▋
    '\u{258C}', // ▌
    '\u{258D}', // ▍
    '\u{258E}', // ▎
    '\u{258F}', // ▏
    '\u{2591}', // ░
    '\u{25B6}', // ▶ (tape arrow — must become '>' in Ascii mode)
];

const ALL_LAYOUTS: &[LayoutStyle] = &[
    LayoutStyle::None,
    LayoutStyle::Zones,
    LayoutStyle::Grid,
    LayoutStyle::Cards,
    LayoutStyle::Sections,
    LayoutStyle::Cockpit,
    LayoutStyle::Console,
    LayoutStyle::Flightstrip,
    LayoutStyle::Auto,
];

fn frame_with_tools_for_ascii_contract() -> RenderFrame {
    let mut f = frame_with_agent_and_quota();
    // Force the tape widget into the render: tools push the recent-tools row
    // through tape::render. Without this, the catch-net wouldn't catch a
    // missed glyph_mode threading on the tape path.
    use cc_pulseline::types::{CompletedToolCount, ToolSummary};
    f.tools = vec![
        ToolSummary {
            id: "1".to_string(),
            name: "Read".to_string(),
            target: Some("main.rs".to_string()),
        },
        ToolSummary {
            id: "2".to_string(),
            name: "Bash".to_string(),
            target: Some("cargo test".to_string()),
        },
    ];
    f.completed_tools = vec![CompletedToolCount {
        name: "Edit".to_string(),
        count: 5,
        last_completed_at: None,
    }];
    f
}

#[test]
fn ascii_mode_emits_no_unicode_block_chars_across_every_layout() {
    let frame = frame_with_tools_for_ascii_contract();
    for layout in ALL_LAYOUTS {
        let cfg = cfg_for(*layout, false, 160);
        let lines = render_frame(&frame, &cfg);
        let blob = lines.join("\n");
        for c in UNICODE_BLOCKS {
            assert!(
                !blob.contains(*c),
                "{:?} layout under display.icons=false leaked U+{:04X} (`{c}`):\n{blob}",
                layout,
                *c as u32
            );
        }
    }
}

// ── Composability: per-segment `*_visual` config can override layout default ──

#[test]
fn cockpit_with_context_visual_text_emits_no_gauge() {
    // Cockpit's layout default is "gauge+sparkline"; users who don't want
    // graphic instruments can opt down to plain text.
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Cockpit, true, 140);
    cfg.context_visual = "text".to_string();
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    // No gauge block chars in any line.
    for c in &['\u{2588}', '\u{2589}', '\u{258F}', '\u{2591}'] {
        assert!(
            !blob.contains(*c),
            "context_visual = \"text\" should suppress the gauge ({:?}): {blob}",
            *c
        );
    }
    // No braille either (sparkline excluded from "text").
    for l in &lines {
        assert!(
            !l.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
            "context_visual = \"text\" should suppress sparkline: {l:?}"
        );
    }
}

#[test]
fn cockpit_with_context_visual_gauge_only_emits_no_sparkline() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Cockpit, true, 140);
    cfg.context_visual = "gauge".to_string();
    let lines = render_frame(&f, &cfg);
    let cluster = lines.iter().find(|l| l.contains("CTX")).expect("cluster");
    // Gauge present.
    assert!(
        cluster.contains('\u{2588}') || cluster.contains('\u{258F}'),
        "expected gauge block chars: {cluster:?}"
    );
    // No braille.
    for l in &lines {
        assert!(
            !l.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
            "context_visual = \"gauge\" should not emit sparkline: {l:?}"
        );
    }
}

#[test]
fn cockpit_with_cost_visual_text_only_has_no_arc() {
    // Cockpit's default cost_visual is "text+arc"; opting down to "text"
    // should drop the arc glyph completely (and the rate annotation
    // re-appears since arc isn't there to convey burn rate).
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Cockpit, true, 140);
    cfg.cost_visual = "text".to_string();
    let lines = render_frame(&f, &cfg);
    let cluster = lines.iter().find(|l| l.contains("$3.50")).expect("cluster");
    let arc_glyphs = ['\u{25CB}', '\u{25D4}', '\u{25D1}', '\u{25D5}', '\u{25CF}'];
    assert!(
        !cluster.chars().any(|c| arc_glyphs.contains(&c)),
        "cost_visual = \"text\" should drop arc glyph: {cluster:?}"
    );
    assert!(
        cluster.contains("/h)"),
        "lone text widget should include rate annotation: {cluster:?}"
    );
}

#[test]
fn cockpit_with_cost_visual_arc_only_has_no_dollar_text() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Cockpit, true, 140);
    cfg.cost_visual = "arc".to_string();
    let lines = render_frame(&f, &cfg);
    // The cost cell is somewhere in the cluster row; identify it by looking
    // for a row that has the arc but no `$3.50`.
    let arc_glyphs = ['\u{25CB}', '\u{25D4}', '\u{25D1}', '\u{25D5}', '\u{25CF}'];
    let cluster = lines
        .iter()
        .find(|l| l.chars().any(|c| arc_glyphs.contains(&c)))
        .expect("cluster with arc");
    assert!(
        !cluster.contains("$3.50"),
        "cost_visual = \"arc\" should drop dollar text: {cluster:?}"
    );
}

#[test]
fn cards_with_context_visual_gauge_emits_gauge_inside_card() {
    // The headline composability proof from `designs/composable-redesign.md`:
    // "cards + gauge" was impossible before Phase 3 because flat layouts
    // hardcoded text rendering for L3. Now they dispatch through
    // render_context_visual just like instrument-cluster layouts do.
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Cards, true, 160);
    cfg.context_visual = "gauge".to_string();
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    // Card frame should be present (top + bottom borders).
    assert!(
        blob.contains('\u{256D}') && blob.contains('\u{2570}'),
        "expected Cards frame chrome (╭ and ╰): {blob}"
    );
    // Gauge block chars should be present somewhere — proof that the gauge
    // widget made it into the Cards frame.
    assert!(
        blob.contains('\u{2588}') || blob.contains('\u{258F}'),
        "cards + context_visual = \"gauge\" should embed a gauge in the \
         Budget card: {blob}"
    );
}

#[test]
fn cockpit_with_quota_visual_bar_emits_gauge_chars() {
    // Cockpit's default quota_visual is "text"; opting up to "bar" should
    // emit gauge block chars beside the percentage.
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Cockpit, true, 160);
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    cfg.quota_visual = "bar".to_string();
    let lines = render_frame(&f, &cfg);
    let q5h = lines.iter().find(|l| l.contains("5h ")).expect("5h cell");
    assert!(
        q5h.contains('\u{2588}') || q5h.contains('\u{258F}'),
        "quota_visual = \"bar\" should emit gauge block chars: {q5h:?}"
    );
}

#[test]
fn console_with_quota_visual_text_drops_gauge() {
    // Console's default quota_visual is "bar"; opting down to "text" should
    // drop the gauge. With battery-style gauges, the empty-cell glyph is
    // also `█` — so we detect the gauge by *the structural-color empty
    // cells* being present, not by any specific glyph. Easier proxy: the
    // bar version contains a long run of block chars right after the label,
    // the text version doesn't.
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Console, true, 160);
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    cfg.quota_visual = "text".to_string();
    let lines = render_frame(&f, &cfg);
    let q5h = lines.iter().find(|l| l.contains("5h ")).expect("5h cell");
    // No `█` block chars at all under text-only spec.
    assert!(
        !q5h.contains('\u{2588}'),
        "quota_visual = \"text\" should drop gauge: {q5h:?}"
    );
    assert!(q5h.contains("75%"), "should still show pct: {q5h:?}");
}

#[test]
fn cockpit_with_tools_visual_tape_brief_omits_target() {
    // Default `tape` spec — brief format, just the running-arrow icon and
    // the tool name. Targets are deliberately suppressed so the cluster
    // row stays narrow.
    use cc_pulseline::types::ToolSummary;
    let mut f = frame_with_agent_and_quota();
    f.tools = vec![ToolSummary {
        id: "1".to_string(),
        name: "Read".to_string(),
        target: Some("src/main.rs".to_string()),
    }];
    let cfg = cfg_for(LayoutStyle::Cockpit, true, 160);
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    assert!(
        blob.contains("\u{25B6}") && blob.contains("Read"),
        "brief tape should show ▶ and tool name: {blob}"
    );
    assert!(
        !blob.contains("src/main.rs"),
        "brief tape should NOT show target: {blob}"
    );
}

#[test]
fn cockpit_with_tools_visual_tape_detail_shows_target() {
    // `tape+detail` spec — opt-in detailed format. Per-tool layout
    // matches the flat-row `list` widget exactly (shared formatter via
    // `widgets::recent_tool::format_recent_tool_inline`).
    use cc_pulseline::types::ToolSummary;
    let mut f = frame_with_agent_and_quota();
    f.tools = vec![ToolSummary {
        id: "1".to_string(),
        name: "Read".to_string(),
        target: Some("src/main.rs".to_string()),
    }];
    let mut cfg = cfg_for(LayoutStyle::Cockpit, true, 160);
    cfg.tools_visual = "tape+detail".to_string();
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    // Format `<icon> Read: src/main.rs` — colon separator + target string.
    assert!(
        blob.contains("Read: src/main.rs"),
        "tape+detail should show name + ': ' + target: {blob}"
    );
}

#[test]
fn cockpit_with_tools_visual_list_drops_inline_tape() {
    // Inline tools_visual="list" is meaningless inside the cockpit ticker
    // (it's a single line); render_tools_visual_inline silently drops it.
    // The user can still see completed-tool counts on the same row, but the
    // running tools tape disappears.
    let f = frame_with_tools_for_ascii_contract();
    let mut cfg = cfg_for(LayoutStyle::Cockpit, true, 160);
    cfg.tools_visual = "list".to_string();
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    assert!(
        !blob.contains('\u{25B6}'),
        "tools_visual = \"list\" should drop the running-tools tape (no ▶): {blob}"
    );
    // Completed-tool checkmark still appears.
    assert!(
        blob.contains('\u{2713}'),
        "completed-tool ✓ should still render: {blob}"
    );
}

#[test]
fn layout_defaults_round_trip_via_default_visuals_for() {
    // Sanity: each instrument-cluster layout's default includes "gauge", and
    // flat layouts default to "text". Catches accidental edits to the
    // default_visuals_for table.
    use cc_pulseline::render::frames::default_visuals_for;
    assert_eq!(
        default_visuals_for(LayoutStyle::Cockpit).context_visual,
        "gauge+sparkline"
    );
    assert_eq!(
        default_visuals_for(LayoutStyle::Console).context_visual,
        "gauge+sparkline"
    );
    assert_eq!(
        default_visuals_for(LayoutStyle::Flightstrip).context_visual,
        "gauge"
    );
    // Console is the only layout that defaults `tools_visual` to detailed
    // — its wide framed dashboard has the room. Cockpit & Flightstrip
    // stay brief so their narrower row budgets don't overflow.
    assert_eq!(
        default_visuals_for(LayoutStyle::Console).tools_visual,
        "tape+detail"
    );
    assert_eq!(
        default_visuals_for(LayoutStyle::Cockpit).tools_visual,
        "tape"
    );
    assert_eq!(
        default_visuals_for(LayoutStyle::Flightstrip).tools_visual,
        "tape"
    );
    assert_eq!(
        default_visuals_for(LayoutStyle::None).context_visual,
        "text"
    );
    assert_eq!(
        default_visuals_for(LayoutStyle::Cards).context_visual,
        "text"
    );
}

#[test]
fn icon_mode_still_uses_block_chars_in_instrument_clusters() {
    // Sanity: the catch-net above must not pass trivially because no widget
    // ever emits a block char. Confirm Icon mode keeps emitting them in at
    // least one of cockpit/console/flightstrip (the ones that actually use
    // gauge widgets).
    let frame = frame_with_tools_for_ascii_contract();
    let mut saw_block = false;
    for layout in [
        LayoutStyle::Cockpit,
        LayoutStyle::Console,
        LayoutStyle::Flightstrip,
    ] {
        let cfg = cfg_for(layout, true, 160);
        let blob = render_frame(&frame, &cfg).join("\n");
        if UNICODE_BLOCKS.iter().any(|c| blob.contains(*c)) {
            saw_block = true;
            break;
        }
    }
    assert!(
        saw_block,
        "expected at least one block glyph from gauge/tape in icon mode \
         across cockpit/console/flightstrip — otherwise the ASCII catch-net \
         above is meaningless"
    );
}
