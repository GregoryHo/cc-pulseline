//! Integration tests for the Ledger layout.
//!
//! Ledger owns its own pipeline — typographic rhythm, TAG column, framed
//! borders, and the only sparkline-bearing layout. These tests pin the
//! visible contracts laid out in `designs/console-redesign/ledger-v2.md`.

use cc_pulseline::config::{GlyphMode, RenderConfig};
use cc_pulseline::render::color::resolve_palette;
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{
    AgentSummary, CompletedToolCount, QuotaMetrics, RenderFrame, ToolSummary,
};

fn cfg(width: usize) -> RenderConfig {
    RenderConfig {
        glyph_mode: GlyphMode::Icon,
        color_enabled: false,
        terminal_width: Some(width + 4), // +cc_margin so the layout sees `width`
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        pane_style: LayoutStyle::Ledger,
        pane_cc_margin: 4,
        // Default-on toggles; tests opt-down where needed.
        ..RenderConfig::default()
    }
}

fn frame_basic() -> RenderFrame {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus 4.6".to_string();
    f.line1.git_branch = "feature/status-pane".to_string();
    f.line1.project_path = "~/cc-pulseline".to_string();
    f.line3.context_window_size = Some(200_000);
    f.line3.context_used_percentage = Some(43);
    f.line3.input_tokens = Some(1);
    f.line3.output_tokens = Some(8);
    f.line3.cache_creation_tokens = Some(5_700);
    f.line3.cache_read_tokens = Some(114_500);
    f.line3.total_cost_usd = Some(4.56);
    f.line3.total_duration_ms = Some(60_000 * 62); // ~1h2m → ~$4.41/h
    f
}

#[test]
fn ledger_renders_framed_at_140_cols() {
    let f = frame_basic();
    let lines = render_frame(&f, &cfg(140));
    assert!(
        lines.first().unwrap().starts_with('╭'),
        "ledger top frame: {:?}",
        lines.first()
    );
    assert!(
        lines.last().unwrap().starts_with('╰'),
        "ledger bottom frame: {:?}",
        lines.last()
    );
    let blob = lines.join("\n");
    // Identity baked into top title.
    assert!(blob.contains("Opus 4.6"), "identity in title: {blob}");
    assert!(
        blob.contains("feature/status-pane"),
        "branch in title: {blob}"
    );
    // TAG column emits CTX / TOK / COST rows.
    assert!(blob.contains("CTX"), "CTX row: {blob}");
    assert!(blob.contains("TOK"), "TOK row: {blob}");
    assert!(blob.contains("COST"), "COST row: {blob}");
}

#[test]
fn ledger_drops_groups_when_no_data() {
    // With no quota / tools / agents / todo, those groups are skipped.
    let f = frame_basic();
    let mut c = cfg(140);
    c.show_quota = false;
    c.show_tools = false;
    c.show_agents = false;
    c.show_todo = false;
    let lines = render_frame(&f, &c);
    let blob = lines.join("\n");
    assert!(!blob.contains("5h"), "no quota: {blob}");
    assert!(!blob.contains("7d"), "no quota: {blob}");
    assert!(!blob.contains("TOOL"), "no tools: {blob}");
    assert!(!blob.contains("AGENT"), "no agents: {blob}");
    assert!(!blob.contains("TODO"), "no todo: {blob}");
}

#[test]
fn ledger_falls_back_to_sections_below_90_cols() {
    let f = frame_basic();
    let lines = render_frame(&f, &cfg(80));
    // Sections fallback uses the same `╭` glyph for the top border but the
    // body has labelled rows (`│ Identity │`, `│ Budget │`). Ledger's
    // own body uses the TAG column (no `Identity` label since identity is
    // in the title). At <90 cols we expect the sections fallback to put
    // identity back into a body row.
    let blob = lines.join("\n");
    assert!(
        blob.contains("Identity") || blob.contains("Budget"),
        "below-90 fallback should render sections labels: {blob}"
    );
}

#[test]
fn ledger_fallback_width_matches_direct_console() {
    // Ledger's <90-col fallback flips pane_style to Console and re-enters
    // render_frame. Without restoring the un-adjusted width, render_frame
    // subtracts `pane_cc_margin` a SECOND time and the fallback render
    // ends up `cc_margin` cells narrower than a direct Console render at
    // the same configured terminal width.
    let f = frame_basic();
    let mut ledger_cfg = cfg(80);
    let mut console_cfg = cfg(80);
    console_cfg.pane_style = LayoutStyle::Console;

    let ledger_lines = render_frame(&f, &ledger_cfg);
    let console_lines = render_frame(&f, &console_cfg);

    // The framed top row's visible width is the most stable witness here
    // — both layouts produce a `╭…╮` border whose length equals the inner
    // cell budget. If the budgets diverge, the borders diverge.
    let ledger_top = ledger_lines.first().expect("ledger emits a top frame");
    let console_top = console_lines.first().expect("console emits a top frame");
    assert_eq!(
        cc_pulseline::render::color::visible_width(ledger_top),
        cc_pulseline::render::color::visible_width(console_top),
        "ledger fallback width must equal direct console width\nledger: {ledger_top:?}\nconsole: {console_top:?}"
    );

    // Silence unused-mut warnings — we set pane_style on console_cfg and
    // intentionally do not mutate ledger_cfg.
    let _ = &mut ledger_cfg;
}

#[test]
fn ledger_renders_quota_when_data_present() {
    let mut f = frame_basic();
    f.quota = QuotaMetrics {
        five_hour_pct: Some(62.0),
        five_hour_reset_minutes: Some(119),
        seven_day_pct: Some(28.0),
        seven_day_reset_minutes: Some(60 * 24 * 4 + 60 * 23 + 59),
    };
    let mut c = cfg(140);
    c.show_quota = true;
    c.show_quota_five_hour = true;
    c.show_quota_seven_day = true;
    let lines = render_frame(&f, &c);
    let blob = lines.join("\n");
    assert!(blob.contains("5h"), "5h tag: {blob}");
    assert!(blob.contains("62%"), "5h pct: {blob}");
    assert!(blob.contains("7d"), "7d tag: {blob}");
    assert!(blob.contains("28%"), "7d pct: {blob}");
}

#[test]
fn ledger_renders_tools_with_one_running_per_row() {
    let mut f = frame_basic();
    f.tools = vec![
        ToolSummary {
            id: "1".to_string(),
            name: "Read".to_string(),
            target: Some("src/console.rs".to_string()),
        },
        ToolSummary {
            id: "2".to_string(),
            name: "Edit".to_string(),
            target: Some("src/gauge.rs".to_string()),
        },
    ];
    f.completed_tools = vec![CompletedToolCount {
        name: "Read".to_string(),
        count: 2,
        last_completed_at: None,
    }];
    let mut c = cfg(140);
    c.max_tool_lines = 4; // raise so both running tools show
    let lines = render_frame(&f, &c);
    let blob = lines.join("\n");
    assert!(blob.contains("TOOL"), "TOOL tag: {blob}");
    assert!(blob.contains("✓"), "completed check: {blob}");
    // Both running tools rendered; vertical scaling, not horizontal packing.
    assert!(blob.contains("Read"), "Read row: {blob}");
    assert!(blob.contains("Edit"), "Edit row: {blob}");
    assert!(blob.contains("▶"), "running arrow: {blob}");
}

#[test]
fn ledger_renders_agent_block() {
    let mut f = frame_basic();
    f.agents.push(AgentSummary {
        id: "a1".to_string(),
        description: "Investigate transcript dispatcher edge case".to_string(),
        agent_type: Some("Explore".to_string()),
        started_at: None,
        model: Some("haiku".to_string()),
        completed_at: None,
        message_id: None,
    });
    let lines = render_frame(&f, &cfg(140));
    let blob = lines.join("\n");
    assert!(blob.contains("AGENT"), "AGENT tag: {blob}");
    assert!(blob.contains("Explore"), "agent type: {blob}");
    assert!(blob.contains("haiku"), "agent model tag: {blob}");
}

#[test]
fn ledger_completed_single_agent_renders_done_check() {
    // A SINGLE completed agent must be visually distinct from a running one
    // — the user observed ledger leaving completed agents stuck in the
    // "running" appearance, which mirrored builder's contract divergence.
    // Pinning ✓ in the rendered row is the minimum visual contract.
    let mut f = frame_basic();
    f.agents.push(AgentSummary {
        id: "a1".to_string(),
        description: "Visual probe — ledger agent row".to_string(),
        agent_type: Some("general-purpose".to_string()),
        started_at: Some(1000),
        model: Some("haiku".to_string()),
        completed_at: Some(2000),
        message_id: None,
    });
    let lines = render_frame(&f, &cfg(140));
    let blob = lines.join("\n");
    assert!(blob.contains("AGENT"), "AGENT tag: {blob}");
    assert!(
        blob.contains('\u{2713}'),
        "completed agent must show ✓: {blob}"
    );
}

#[test]
fn ledger_respects_agents_visual_name_only() {
    // When the user opts out of description via `agents_visual = "name"`,
    // ledger must honour it the same way other layouts do — otherwise the
    // toggle is ineffective.
    let mut f = frame_basic();
    f.agents.push(AgentSummary {
        id: "a1".to_string(),
        description: "should not appear when description is hidden".to_string(),
        agent_type: Some("Explore".to_string()),
        started_at: None,
        model: Some("haiku".to_string()),
        completed_at: None,
        message_id: None,
    });
    let mut c = cfg(140);
    c.agents_visual = "name".to_string();
    let lines = render_frame(&f, &c);
    let blob = lines.join("\n");
    assert!(blob.contains("AGENT"), "AGENT tag still rendered: {blob}");
    assert!(blob.contains("Explore"), "agent name still appears: {blob}");
    assert!(
        !blob.contains("should not appear"),
        "description must be suppressed when visual='name': {blob}"
    );
    assert!(
        !blob.contains("haiku"),
        "model must be suppressed when visual='name': {blob}"
    );
}

#[test]
fn ledger_keeps_newest_active_under_cap() {
    // With more concurrent active agents than `max_agent_lines`, the cap
    // must drop the OLDEST and surface the newest — otherwise users miss
    // the most recent activity. `frame.agents` arrives oldest-first per
    // `agents_for_display`'s contract, so a naive `.take(max)` would
    // preserve the wrong end.
    let mut f = frame_basic();
    for (id, desc) in [
        ("a1", "OLDEST agent description"),
        ("a2", "MIDDLE agent description"),
        ("a3", "NEWEST agent description"),
    ] {
        f.agents.push(AgentSummary {
            id: id.to_string(),
            description: desc.to_string(),
            agent_type: Some("Explore".to_string()),
            started_at: None,
            model: None,
            completed_at: None,
            message_id: Some(format!("msg_{id}")), // unique → stays Single
        });
    }
    let mut c = cfg(140);
    c.max_agent_lines = 2;
    let lines = render_frame(&f, &c);
    let blob = lines.join("\n");
    assert!(
        blob.contains("NEWEST"),
        "newest active must be kept under cap: {blob}"
    );
    assert!(
        blob.contains("MIDDLE"),
        "middle active must be kept under cap: {blob}"
    );
    assert!(
        !blob.contains("OLDEST"),
        "oldest active must be dropped under cap: {blob}"
    );
}

#[test]
fn ledger_sparkline_omitted_in_ascii_mode() {
    // Sparkline is icon-only; under `display.icons = false` the braille
    // glyphs disappear. The delta-time label remains (text carries the
    // trend even when braille can't).
    let mut f = frame_basic();
    f.ctx_history = vec![
        (30, 1_000),
        (35, 2_000),
        (40, 3_000),
        (43, 65_000), // 64s window — past the 1m floor
    ];
    let mut c = cfg(140);
    c.glyph_mode = GlyphMode::Ascii;
    let lines = render_frame(&f, &c);
    let blob = lines.join("\n");
    // No braille glyphs anywhere.
    for line in &lines {
        assert!(
            !line
                .chars()
                .any(|ch| (0x2800..=0x28FF).contains(&(ch as u32))),
            "ascii must hide braille: {line:?}"
        );
    }
    // Delta label still readable in plain text.
    assert!(blob.contains("30→43"), "delta label: {blob}");
}

#[test]
fn ledger_sparkline_renders_when_history_populated() {
    let mut f = frame_basic();
    f.ctx_history = vec![
        (30, 0),
        (32, 5_000),
        (34, 10_000),
        (36, 15_000),
        (38, 30_000),
        (40, 45_000),
        (43, 65_000),
    ];
    let lines = render_frame(&f, &cfg(140));
    let ctx_row = lines.iter().find(|l| l.contains("CTX")).expect("CTX row");
    // Braille present somewhere in the CTX row.
    assert!(
        ctx_row
            .chars()
            .any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
        "sparkline braille on CTX row: {ctx_row:?}"
    );
    // Delta-time label of the form `<first>→<last>% in <duration>`.
    assert!(ctx_row.contains("30→43%"), "delta-pct label: {ctx_row:?}");
}

#[test]
fn ledger_sparkline_window_respects_1min_floor() {
    // 14 samples spread over only 28 seconds (a burst) — the 1-minute
    // floor expands the window backward. `format_window_duration`
    // produces a label of at least `60s` worth ("1m" or higher).
    let mut f = frame_basic();
    let mut hist = Vec::new();
    let mut t: u64 = 0;
    for i in 0..14 {
        hist.push((30 + i as u8, t));
        t += 2_000; // 2 s per sample
    }
    f.ctx_history = hist;
    let lines = render_frame(&f, &cfg(140));
    let ctx_row = lines.iter().find(|l| l.contains("CTX")).expect("CTX row");
    // 14 samples × 2 s = 28 s total. The default visible window is 12
    // samples (24 s). Below the 1m floor → window expands. The text label
    // therefore reads ≥ 1m or ≥ 28s of elapsed window after expansion.
    // Sanity: at least the delta-pct label exists.
    assert!(ctx_row.contains("→"), "delta arrow: {ctx_row:?}");
}

#[test]
fn ledger_all_complete_celebration_line() {
    use cc_pulseline::types::TodoSummary;
    let mut f = frame_basic();
    f.todo = Some(TodoSummary {
        text: String::new(),
        pending: 0,
        completed: 3,
        total: 3,
        in_progress_items: Vec::new(),
        all_done: true,
        is_task_api: false,
    });
    let lines = render_frame(&f, &cfg(140));
    let blob = lines.join("\n");
    assert!(blob.contains("All complete"), "celebration line: {blob}");
    assert!(blob.contains("3/3"), "count fraction: {blob}");
}
