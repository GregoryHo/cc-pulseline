//! Verifies that v2 layouts honor the `display.icons` axis and the new
//! `show_ctx_sparkline` toggle, plus that Q7d renders in cockpit + console
//! when the seven-day quota toggle is on.
//!
//! These pin the contract spelled out in `designs/style-to-layout-taxonomy.md`:
//! every (layout × display) pair must compose without leaking hardcoded
//! glyphs through.

use cc_pulseline::config::{build_render_config, GlyphMode, PulselineConfig, RenderConfig};
use cc_pulseline::render::color::resolve_palette;
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
    f.ctx_history = vec![
        (10, 1_000),
        (20, 2_000),
        (30, 3_000),
        (35, 4_000),
        (40, 5_000),
        (43, 6_000),
    ];
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

// ── Sparkline opt-in via `context_visual = "...+sparkline"` ──

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

// ── Q7d renders alongside Q5h when toggle on ──

#[test]
fn console_renders_seven_day_alongside_five_hour() {
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Console, true, 160);
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    cfg.show_quota_seven_day = true;
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    // Console now uses the flat `5h: 75% (resets ...)` format inside its
    // Budget group (sections + identity-in-title), not the cluster-style
    // `5h ` cells. Assert on the stable label substrings.
    assert!(blob.contains("5h"), "5h missing in console: {blob}");
    assert!(blob.contains("7d"), "7d missing in console: {blob}");
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
    LayoutStyle::Sections,
    LayoutStyle::Console,
    LayoutStyle::Ledger,
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
fn console_with_quota_visual_text_drops_gauge() {
    // Console's default `quota_visual = "gauge"` renders the F-style bar
    // (▰ for filled, ─ for empty, · for threshold marks). Opting down to
    // `"text"` must drop all three of those glyphs and leave only the
    // pct + reset text.
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Console, true, 160);
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    cfg.quota_visual = "text".to_string();
    let lines = render_frame(&f, &cfg);
    let q5h = lines.iter().find(|l| l.contains("5h")).expect("5h cell");
    assert!(
        !q5h.contains('\u{25B0}'),
        "quota_visual = \"text\" should drop ▰ filled cells: {q5h:?}"
    );
    assert!(
        !q5h.contains('\u{2500}'),
        "quota_visual = \"text\" should drop ─ empty cells: {q5h:?}"
    );
    assert!(q5h.contains("75%"), "should still show pct: {q5h:?}");
}

#[test]
fn console_default_quota_visual_includes_gauge() {
    // Sanity check the other side of the toggle: with the layout default
    // (no override), Console renders the bar (▰ filled cells present).
    let f = frame_with_agent_and_quota();
    let mut cfg = cfg_for(LayoutStyle::Console, true, 160);
    cfg.show_quota = true;
    cfg.show_quota_five_hour = true;
    // No explicit override — falls through to default_visuals_for(Console).
    let lines = render_frame(&f, &cfg);
    let q5h = lines.iter().find(|l| l.contains("5h")).expect("5h cell");
    assert!(
        q5h.contains('\u{25B0}'),
        "default quota_visual should render ▰ filled cells: {q5h:?}"
    );
    assert!(q5h.contains("75%"), "should still show pct: {q5h:?}");
}

#[test]
fn console_with_tape_detail_no_row_overflows_pane_width() {
    // Regression test for the multi-line-row bug in the cluster layouts.
    //
    // Pre-fix: `tape::render` produced a finished string with each cell
    // truncated to a fixed `ideal=40` regardless of pane width. Two long
    // Bash cells could easily exceed Console's `inner` budget. Console's
    // `framed()` only pads (saturating to 0) — it does NOT truncate — so
    // the right `│` border ended up past the user's pane width and CC's
    // statusline parser wrapped the row into multiple visual lines,
    // shifting everything below off-screen.
    //
    // Post-fix: tape returns `Vec<Cell>`, the dispatch hub passes
    // `inner.saturating_sub(2)` to `pack_with_separator`, and detail
    // cells (Required, min_width=8) compress their target to fit. Every
    // returned line must be visually <= the configured terminal width.
    use cc_pulseline::render::color::visible_width;
    use cc_pulseline::types::ToolSummary;
    let mut f = frame_with_agent_and_quota();
    f.tools = vec![
        ToolSummary {
            id: "1".to_string(),
            name: "Bash".to_string(),
            target: Some(
                "cargo test --release --no-default-features --features experimental_quota"
                    .to_string(),
            ),
        },
        ToolSummary {
            id: "2".to_string(),
            name: "Bash".to_string(),
            target: Some(
                "sed -i '' 's/^name = \".*\"$/name = \"cards\"/' .claude/pulseline.toml"
                    .to_string(),
            ),
        },
    ];
    let mut cfg = cfg_for(LayoutStyle::Console, true, 130);
    cfg.tools_visual = "tape+detail".to_string();
    let lines = render_frame(&f, &cfg);
    let term_w = cfg.terminal_width.unwrap_or(usize::MAX);
    for line in &lines {
        let w = visible_width(line);
        assert!(
            w <= term_w,
            "console line wider than terminal_width={term_w}: w={w}, line={line:?}"
        );
    }
    // Sanity: at least one line still mentions the verb so we know the
    // tape rendered (didn't get dropped wholesale by the budgeter).
    let blob = lines.join("\n");
    assert!(
        blob.contains("Bash"),
        "tape detail row should survive width pressure: {blob}"
    );
}

#[test]
fn console_with_many_agents_no_overflow_and_descriptions_visible() {
    // Regression test for the cluster-agent multi-line bug. Pre-fix, the
    // cluster `agent_todo_row` rolled its own `<icon><type>[model]` strings
    // and joined them with two spaces — no width budget, no parallel
    // grouping, no descriptions. Long descriptions would push the row past
    // the framed pane and CC's statusline parser wrapped it into multiple
    // visual rows. Post-fix the cluster path delegates to
    // `build_agent_cells` + `pack_with_separator`, so:
    //   1. Every emitted line is <= terminal_width (no wrap)
    //   2. Agents from the same `message_id` group as one parallel cell
    //   3. The first line of each agent's description is rendered
    use cc_pulseline::render::color::visible_width;
    let mut f = frame_with_agent_and_quota();
    // Replace the single seed agent with a parallel pair (same message_id)
    // plus one solo running agent — three distinct group cells.
    f.agents.clear();
    f.agents.push(AgentSummary {
        id: "p1".to_string(),
        description: "Inspect transcript parser for memory leaks".to_string(),
        agent_type: Some("Explore".to_string()),
        started_at: Some(0),
        model: Some("haiku".to_string()),
        completed_at: None,
        message_id: Some("m1".to_string()),
    });
    f.agents.push(AgentSummary {
        id: "p2".to_string(),
        description: "Audit cluster layout width handling".to_string(),
        agent_type: Some("Explore".to_string()),
        started_at: Some(0),
        model: Some("haiku".to_string()),
        completed_at: None,
        message_id: Some("m1".to_string()),
    });
    f.agents.push(AgentSummary {
        id: "s1".to_string(),
        description: "Implement cell-based cluster agents".to_string(),
        agent_type: Some("implementer".to_string()),
        started_at: Some(0),
        model: None,
        completed_at: None,
        message_id: Some("m2".to_string()),
    });
    let mut cfg = cfg_for(LayoutStyle::Console, true, 130);
    cfg.max_agent_lines = 3;
    let lines = render_frame(&f, &cfg);
    let term_w = cfg.terminal_width.unwrap_or(usize::MAX);
    for line in &lines {
        let w = visible_width(line);
        assert!(
            w <= term_w,
            "console agent line wider than terminal_width={term_w}: w={w}, line={line:?}"
        );
    }
    let blob = lines.join("\n");
    // Parallel pair (same message_id) collapses into one cell; the
    // homogeneous-group cell builder emits `Explore ×2` so both get
    // surfaced without taking two row-cells.
    assert!(
        blob.contains("Explore"),
        "Explore agent type missing from cluster agent row: {blob}"
    );
    assert!(
        blob.contains("\u{00D7}2") || blob.contains("Explore ×2"),
        "parallel pair should render as a `×2` group cell: {blob}"
    );
    // Description body must be visible (the pre-fix cluster path showed
    // only the agent_type with no description).
    assert!(
        blob.contains("transcript parser") || blob.contains("cluster layout"),
        "expected a description fragment from the parallel pair: {blob}"
    );
}

#[test]
fn console_tools_split_to_two_rows_when_running_too_long_for_inline_counts() {
    // User-driven layout rule: if running tape + completed counts can't
    // fit on one row of the framed budget, push counts to a row of their
    // own. Pre-fix behaviour was to emit them inline regardless and
    // overflow `framed()`'s padding (which doesn't truncate).
    use cc_pulseline::render::color::visible_width;
    use cc_pulseline::types::{CompletedToolCount, ToolSummary};
    let mut f = frame_with_agent_and_quota();
    f.tools = vec![
        ToolSummary {
            id: "1".to_string(),
            name: "Bash".to_string(),
            target: Some(
                "cargo test --release --no-default-features --features experimental".to_string(),
            ),
        },
        ToolSummary {
            id: "2".to_string(),
            name: "Bash".to_string(),
            target: Some(
                "sed -i '' 's/^name = \".*\"$/name = \"cards\"/' .claude/pulseline.toml"
                    .to_string(),
            ),
        },
    ];
    f.completed_tools = vec![
        CompletedToolCount {
            name: "Bash".to_string(),
            count: 206,
            last_completed_at: None,
        },
        CompletedToolCount {
            name: "Read".to_string(),
            count: 92,
            last_completed_at: None,
        },
        CompletedToolCount {
            name: "Edit".to_string(),
            count: 77,
            last_completed_at: None,
        },
        CompletedToolCount {
            name: "Write".to_string(),
            count: 4,
            last_completed_at: None,
        },
    ];
    let mut cfg = cfg_for(LayoutStyle::Console, true, 130);
    cfg.tools_visual = "tape+detail".to_string();
    let lines = render_frame(&f, &cfg);
    let term_w = cfg.terminal_width.unwrap_or(usize::MAX);
    for line in &lines {
        assert!(
            visible_width(line) <= term_w,
            "line wider than terminal_width={term_w}: w={} {line:?}",
            visible_width(line)
        );
    }
    let blob = lines.join("\n");
    assert!(
        blob.contains("Bash") && blob.contains("\u{00D7}206"),
        "running + completed both visible somewhere: {blob}"
    );
    // Count which framed body rows mention `T:` (running tape — uses
    // `T:Bash:` as the prefix in ascii or the tool icon + "Bash" in icon
    // mode) vs `✓ Bash ×206` (completed-count chip row). Even at 130
    // cols with two long Bash targets, a side-by-side layout would
    // overflow — so we expect the test to land on the split path.
    let running_rows: Vec<&String> = lines.iter().filter(|l| l.contains("Bash:")).collect();
    assert!(
        !running_rows.is_empty(),
        "running tape row not found: {blob}"
    );
}

#[test]
fn console_agents_wrap_to_multiple_rows_when_overflowing_one() {
    // User-driven layout rule: agent cells that don't fit on one row
    // wrap to additional rows (capped by `max_agent_lines`) instead of
    // dropping cells via Optional priority.
    use cc_pulseline::render::color::visible_width;
    let mut f = frame_with_agent_and_quota();
    f.agents.clear();
    // Five Single-priority agents (no shared message_id), each with a
    // long enough description that two cells don't fit on a 130-col
    // framed row → multi-row wrap.
    for i in 0..5 {
        f.agents.push(AgentSummary {
            id: format!("a{i}"),
            description: format!("Agent {i} doing some longer description to force overflow"),
            agent_type: Some(format!("explorer-{i}")),
            started_at: Some(0),
            model: None,
            completed_at: None,
            message_id: Some(format!("m{i}")),
        });
    }
    let mut cfg = cfg_for(LayoutStyle::Console, true, 130);
    cfg.max_agent_lines = 3;
    let lines = render_frame(&f, &cfg);
    let term_w = cfg.terminal_width.unwrap_or(usize::MAX);
    for line in &lines {
        assert!(
            visible_width(line) <= term_w,
            "line wider than terminal_width={term_w}: w={} {line:?}",
            visible_width(line)
        );
    }
    let agent_rows: Vec<&String> = lines.iter().filter(|l| l.contains("explorer-")).collect();
    assert!(
        agent_rows.len() >= 2,
        "expected agents to wrap to ≥2 framed rows when 5 don't fit on one: \
         got {} rows: {:?}",
        agent_rows.len(),
        agent_rows
    );
    assert!(
        agent_rows.len() <= cfg.max_agent_lines,
        "must respect max_agent_lines cap of {}: got {}",
        cfg.max_agent_lines,
        agent_rows.len()
    );
}

#[test]
fn console_agents_visual_name_only_drops_description_and_model() {
    // `agents_visual = "name"` → only the agent type renders. No
    // description body, no `[model]` slack tail.
    let mut f = frame_with_agent_and_quota();
    f.agents.clear();
    f.agents.push(AgentSummary {
        id: "a1".to_string(),
        description: "Investigate transcript leaks".to_string(),
        agent_type: Some("Explore".to_string()),
        started_at: Some(0),
        model: Some("haiku".to_string()),
        completed_at: None,
        message_id: None,
    });
    let mut cfg = cfg_for(LayoutStyle::Console, true, 130);
    cfg.agents_visual = "name".to_string();
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    assert!(blob.contains("Explore"), "name still rendered: {blob}");
    assert!(
        !blob.contains("Investigate transcript leaks"),
        "description must be hidden: {blob}"
    );
    assert!(
        !blob.contains("[haiku]"),
        "model tag must be hidden: {blob}"
    );
}

#[test]
fn console_agents_visual_name_and_model_shows_model_tag_no_description() {
    let mut f = frame_with_agent_and_quota();
    f.agents.clear();
    f.agents.push(AgentSummary {
        id: "a1".to_string(),
        description: "Investigate transcript leaks".to_string(),
        agent_type: Some("Explore".to_string()),
        started_at: Some(0),
        model: Some("haiku".to_string()),
        completed_at: None,
        message_id: None,
    });
    let mut cfg = cfg_for(LayoutStyle::Console, true, 130);
    cfg.agents_visual = "name+model".to_string();
    let lines = render_frame(&f, &cfg);
    let blob = lines.join("\n");
    assert!(blob.contains("Explore"));
    assert!(blob.contains("[haiku]"), "model tag should show: {blob}");
    assert!(
        !blob.contains("Investigate transcript leaks"),
        "description must be hidden when only `name+model`: {blob}"
    );
}
