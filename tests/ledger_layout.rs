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
fn ledger_tok_row_appends_hit_rate() {
    // frame_basic: read=114.5k, creation=5.7k, input=1
    // → hit = 114_500 / (1 + 5_700 + 114_500) ≈ 95%.
    let f = frame_basic();
    let lines = render_frame(&f, &cfg(140));
    let tok_row = lines.iter().find(|l| l.contains("TOK")).expect("TOK row");
    assert!(
        tok_row.contains("% hit"),
        "TOK row should append the cache hit rate: {tok_row}"
    );
    assert!(
        tok_row.contains("cache"),
        "TOK row should keep the raw create/read pair: {tok_row}"
    );
}

#[test]
fn ledger_tok_row_omits_hit_when_no_cache() {
    let mut f = frame_basic();
    f.line3.cache_creation_tokens = Some(0);
    f.line3.cache_read_tokens = Some(0);
    let lines = render_frame(&f, &cfg(140));
    let tok_row = lines.iter().find(|l| l.contains("TOK")).expect("TOK row");
    assert!(
        !tok_row.contains("hit"),
        "TOK row should omit the hit rate without cache signal: {tok_row}"
    );
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
fn ledger_falls_back_to_console_below_90_cols() {
    let f = frame_basic();
    let lines = render_frame(&f, &cfg(80));
    // Console fallback uses the same `╭` glyph for the top border but the
    // body has labelled rows (`│ Budget │`, `│ Activity │`). Ledger's
    // own body uses the TAG column (no group labels — metrics anchor on
    // TAGs like CTX / TOK / COST instead). At <90 cols we expect the
    // console fallback to render group-labelled body rows.
    let blob = lines.join("\n");
    assert!(
        blob.contains("Identity") || blob.contains("Budget"),
        "below-90 fallback should render console group labels: {blob}"
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
        failed: 0,
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
        agent_id: None,
        total_duration_ms: None,
        total_tokens: None,
        total_tool_use_count: None,
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
        agent_id: None,
        total_duration_ms: None,
        total_tokens: None,
        total_tool_use_count: None,
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
        agent_id: None,
        total_duration_ms: None,
        total_tokens: None,
        total_tool_use_count: None,
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
            message_id: Some(format!("msg_{id}")), // unique → stays Single,
            agent_id: None,
            total_duration_ms: None,
            total_tokens: None,
            total_tool_use_count: None,
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

// ─────────────────────────────────────────────────────────────────────
// Frame width invariants — top border must always equal bottom border
// across realistic long-path / long-branch combos. Catches the bug where
// `top_frame` left `head` un-truncated when it exceeded `ctx.inner`,
// pushing the right `╮` past the terminal width.
// ─────────────────────────────────────────────────────────────────────

fn make_frame(path: &str, branch: &str) -> RenderFrame {
    let mut f = frame_basic();
    f.line1.project_path = path.to_string();
    f.line1.git_branch = branch.to_string();
    f
}

#[test]
fn ledger_top_border_width_matches_bottom_across_widths() {
    use cc_pulseline::render::color::visible_width;

    let short_path = "~/cc-pulseline";
    let short_branch = "main";
    let long_path = "~/Workspace/Paradise/Frontend/platform-1.0/platform-web";
    let long_branch = "feature/e2e-traceability-rollout-integration";

    let cases: &[(&str, &str, &str)] = &[
        ("short_path_short_branch", short_path, short_branch),
        ("long_path_short_branch", long_path, short_branch),
        ("short_path_long_branch", short_path, long_branch),
        ("long_path_long_branch", long_path, long_branch),
    ];
    let widths = [100usize, 140, 200, 240];

    for w in widths {
        for (label, path, branch) in cases {
            let f = make_frame(path, branch);
            let lines = render_frame(&f, &cfg(w));
            let top = lines.first().expect("top frame line");
            let bottom = lines.last().expect("bottom frame line");
            assert_eq!(
                visible_width(top),
                visible_width(bottom),
                "width {w}, case {label}\n  top:    {top}\n  bottom: {bottom}",
            );
        }
    }
}

#[test]
fn ledger_top_aligns_under_long_path_and_branch_at_100_cols() {
    use cc_pulseline::render::color::visible_width;

    // Direct repro of the user-reported screenshot conditions.
    let f = make_frame(
        "~/Workspace/Paradise/Frontend/platform-1.0/platform-web",
        "feature/e2e-traceability-rollout-integration",
    );
    let lines = render_frame(&f, &cfg(100));
    let top = lines.first().expect("top frame line");
    let bottom = lines.last().expect("bottom frame line");
    assert_eq!(
        visible_width(top),
        visible_width(bottom),
        "headline must self-bound\n  top:    {top}\n  bottom: {bottom}",
    );
    // Right edge `╮` must appear — its absence is the original bug signal.
    assert!(top.contains('╮'), "top frame has right corner: {top}");
}

#[test]
fn ledger_preserves_show_version_when_path_is_long() {
    // Regression test for v1.1.3 → v1.1.4 cascade reorder: a user who
    // explicitly enables `show_version = true` should not lose the CC:
    // pill just because their project path is long. The cascade must
    // compress path/branch (visibly recoverable as `~/…/leaf`) before
    // dropping any pill (toggle off — entire segment vanishes).
    //
    // Repro case: platform-web — path 57 chars, branch 40 chars,
    // `show_version` + `show_git_stats` both on. At pane_max_width=140
    // the full headline overflows by ~20 chars. Pre-fix DROP_ORDER
    // killed the CC: pill first; post-fix the path compresses to
    // `~/…/platform-web` and the pill survives.
    let mut f = make_frame(
        "~/Workspace/Paradise/Frontend/platform-1.0/platform-web",
        "feature/e2e-traceability-graph-viz",
    );
    f.line1.claude_code_version = "2.1.138".to_string();

    for w in [120usize, 140, 160, 200, 240] {
        let lines = render_frame(&f, &cfg(w));
        let top = lines.first().expect("top frame line");
        assert!(
            top.contains("2.1.138"),
            "CC: pill must survive path overflow at width={w}\n  top: {top}"
        );
    }
}

#[test]
fn ledger_renders_at_pane_max_width_when_terminal_width_unknown() {
    // Reproduces the statusline-hook context where `/dev/tty` is
    // unreachable and `COLUMNS` is unset or `"0"` → `terminal_width: None`.
    // Pre-fix: ledger fell back to Console (sections), surfacing internal
    // `├─┼─┤` row separators and a `│ TAG │ body │` cell border, which
    // doesn't match the user's chosen ledger layout.
    let mut c = cfg(140);
    c.terminal_width = None;
    let f = frame_basic();
    let lines = render_frame(&f, &c);
    let blob = lines.join("\n");

    assert!(
        !blob.contains('├') && !blob.contains('┼') && !blob.contains('┤'),
        "ledger should not render Console sections separators (├ ┼ ┤) when width is unknown\n{blob}"
    );
    let top = lines.first().expect("top frame line");
    assert!(
        top.starts_with('╭'),
        "top frame line must start with ╭: {top}"
    );
    assert!(top.contains('╮'), "top frame line must end with ╮: {top}");
}

#[test]
fn ledger_unknown_width_honors_pane_max_width_and_aligns_borders() {
    use cc_pulseline::render::color::visible_width;

    // Pin two invariants for the `terminal_width: None` code path:
    //   (1) top width == bottom width (frame geometry)
    //   (2) frame width tracks `pane_max_width` (width sourcing — proves
    //       we didn't accidentally regress to a hardcoded default or to
    //       ignoring the user's max_width override).
    // The exact delta between `pane_max_width` and the rendered frame
    // width is governed by FRAME_INNER_PAD (private to ledger.rs); we
    // assert a loose bound rather than the exact number so the test
    // survives constant tuning.
    let f = frame_basic();
    for max_w in [100usize, 120, 140, 180] {
        let mut c = cfg(140);
        c.terminal_width = None;
        c.pane_max_width = max_w;
        let lines = render_frame(&f, &c);
        let top = lines.first().expect("top frame line");
        let bottom = lines.last().expect("bottom frame line");
        let top_w = visible_width(top);
        let bot_w = visible_width(bottom);

        assert_eq!(
            top_w, bot_w,
            "borders must align at pane_max_width={max_w}\n  top: {top}\n  bot: {bottom}"
        );
        // Loose: frame width is within 10 cells of pane_max_width (private
        // FRAME_INNER_PAD = 5 currently yields top_w == pane_max_width - 3).
        let lower = max_w.saturating_sub(10);
        assert!(
            (lower..=max_w).contains(&top_w),
            "frame width {top_w} should track pane_max_width={max_w} (within 10)\n  top: {top}"
        );
    }
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
        sub_agent_count: None,
    });
    let lines = render_frame(&f, &cfg(140));
    let blob = lines.join("\n");
    assert!(blob.contains("All complete"), "celebration line: {blob}");
    assert!(blob.contains("3/3"), "count fraction: {blob}");
}

/// A row consisting only of the frame bars and inner spaces — the visual
/// signature of `blank_row()` when color is disabled.
fn is_blank_inner_row(line: &str) -> bool {
    // Strip the outer frame bars and check the interior is whitespace-only.
    let stripped = line.trim_start_matches('│').trim_end_matches('│');
    !stripped.is_empty() && stripped.chars().all(|c| c == ' ')
}

#[test]
fn ledger_no_trailing_blank_when_tools_last() {
    // Invariant: when TOOL is the last non-empty group, no blank row may sit
    // between it and `bottom_frame`.
    let mut f = frame_basic();
    f.tools = vec![ToolSummary {
        id: "1".to_string(),
        name: "Read".to_string(),
        target: Some("src/lib.rs".to_string()),
    }];
    f.completed_tools = vec![CompletedToolCount {
        name: "Read".to_string(),
        count: 3,
        last_completed_at: None,
        failed: 0,
    }];
    let mut c = cfg(140);
    c.show_agents = false;
    c.show_todo = false;
    let lines = render_frame(&f, &c);

    assert!(lines.last().unwrap().starts_with('╰'), "bottom frame last");
    let above_bottom = &lines[lines.len() - 2];
    assert!(
        !is_blank_inner_row(above_bottom),
        "row above bottom frame must NOT be a blank inner row: {above_bottom:?}"
    );
    // Sanity check: TOOL group did render.
    let blob = lines.join("\n");
    assert!(blob.contains("TOOL"), "TOOL group should render: {blob}");
}

#[test]
fn ledger_dense_drops_all_inter_group_blanks() {
    // dense=true → zero blank rows between groups. Every interior row must
    // carry a TAG label or content; only the top/bottom frame chrome lines
    // are exempt.
    let mut f = frame_basic();
    f.tools = vec![ToolSummary {
        id: "1".to_string(),
        name: "Read".to_string(),
        target: Some("src/lib.rs".to_string()),
    }];
    f.agents.push(AgentSummary {
        id: "a1".to_string(),
        description: "Dense rhythm check".to_string(),
        agent_type: Some("Explore".to_string()),
        started_at: None,
        model: None,
        completed_at: None,
        message_id: None,
        agent_id: None,
        total_duration_ms: None,
        total_tokens: None,
        total_tool_use_count: None,
    });
    let mut c = cfg(140);
    c.pane_ledger_dense = true;
    let lines = render_frame(&f, &c);

    // Skip top + bottom frame lines; the interior must contain no blank rows.
    for line in &lines[1..lines.len() - 1] {
        assert!(
            !is_blank_inner_row(line),
            "dense ledger must have no blank inner row: {line:?}"
        );
    }
    // Sanity: groups still rendered.
    let blob = lines.join("\n");
    assert!(
        blob.contains("ENV") || blob.contains("CTX"),
        "G1/G2: {blob}"
    );
    assert!(blob.contains("TOOL"), "tools group: {blob}");
    assert!(blob.contains("AGENT"), "actors group: {blob}");
}

#[test]
fn ledger_dense_false_preserves_legacy_spacing() {
    // dense=false (default) reproduces the legacy blank-row positions:
    // exactly one blank between each pair of present groups, and no
    // trailing blank above the bottom frame.
    let mut f = frame_basic();
    f.tools = vec![ToolSummary {
        id: "1".to_string(),
        name: "Read".to_string(),
        target: Some("src/lib.rs".to_string()),
    }];
    f.agents.push(AgentSummary {
        id: "a1".to_string(),
        description: "Legacy rhythm check".to_string(),
        agent_type: Some("Explore".to_string()),
        started_at: None,
        model: None,
        completed_at: None,
        message_id: None,
        agent_id: None,
        total_duration_ms: None,
        total_tokens: None,
        total_tool_use_count: None,
    });
    let lines = render_frame(&f, &cfg(140));

    // Groups present: G1 ENV (default-on), G2 Budget (CTX/TOK/COST),
    // G3 Tools, G4 Actors → 3 inter-group separators.
    let blank_count = lines.iter().filter(|l| is_blank_inner_row(l)).count();
    assert_eq!(
        blank_count, 3,
        "expected 3 inter-group blank rows in legacy mode, got {blank_count}: {lines:?}"
    );
    // No trailing blank above bottom frame.
    let above_bottom = &lines[lines.len() - 2];
    assert!(
        !is_blank_inner_row(above_bottom),
        "row above bottom frame must NOT be blank: {above_bottom:?}"
    );
}

// ── tools_visual / todo_visual spec compliance ───────────────────────

#[test]
fn ledger_tools_ticker_fuses_to_one_tool_row() {
    let mut f = frame_basic();
    f.completed_tools = vec![
        cc_pulseline::types::CompletedToolCount {
            name: "Bash".to_string(),
            count: 12,
            last_completed_at: None,
            failed: 0,
        },
        cc_pulseline::types::CompletedToolCount {
            name: "Read".to_string(),
            count: 8,
            last_completed_at: None,
            failed: 0,
        },
    ];
    f.tools = vec![cc_pulseline::types::ToolSummary {
        id: "t1".to_string(),
        name: "Bash".to_string(),
        target: Some("cargo test".to_string()),
    }];
    let config = RenderConfig {
        tools_visual: "ticker".to_string(),
        ..cfg(120)
    };
    let lines = cc_pulseline::render::layout::render_frame(&f, &config);
    let tool_rows: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("TOOL") || l.contains("20 tools"))
        .collect();
    let blob = lines.join("\n");
    assert!(
        blob.contains("20 tools"),
        "ticker grand total expected: {blob}"
    );
    assert!(
        blob.contains("cargo test"),
        "first running tool keeps target: {blob}"
    );
    // Grand total and running tool share ONE row.
    assert!(
        lines
            .iter()
            .any(|l| l.contains("20 tools") && l.contains("Bash")),
        "ticker must fuse onto one row: {blob}"
    );
    drop(tool_rows);
}

#[test]
fn ledger_todo_bar_renders_progress_gauge() {
    let mut f = frame_basic();
    f.todo = Some(cc_pulseline::types::TodoSummary {
        text: String::new(),
        pending: 3,
        completed: 2,
        total: 5,
        in_progress_items: vec![],
        all_done: false,
        is_task_api: true,
        sub_agent_count: None,
    });
    let config = RenderConfig {
        todo_visual: "bar+text".to_string(),
        ..cfg(120)
    };
    let lines = cc_pulseline::render::layout::render_frame(&f, &config);
    let blob = lines.join("\n");
    // 2/5 = 40% on a 5-cell icon gauge → ▰▰───.
    assert!(
        blob.contains("\u{25B0}\u{25B0}"),
        "todo gauge expected in ledger: {blob}"
    );
    assert!(blob.contains("2/5 done"), "text atom kept: {blob}");
}
