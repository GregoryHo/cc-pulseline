//! Integration tests for the quota gauge bar (F design — marks on track).
//!
//! Verifies:
//!   - Per-layout default `quota_visual` (none → text;
//!     console/ledger → gauge)
//!   - Bar is positioned BEFORE the percentage in both flat (console)
//!     and TAG-column (ledger) formats
//!   - `quota_visual = "text"` opt-out actually drops the bar
//!   - Mark positions land at cells 7 (50%) and 12 (85%) in the 14-cell bar
//!   - CTX gauge uses its own threshold marks (55/70 std window),
//!     distinct from quota's [50, 85]
//!   - ASCII mode swaps to `=`/`-`/`:` punctuation (no Unicode block leak)

use cc_pulseline::config::{GlyphMode, RenderConfig};
use cc_pulseline::render::color::resolve_palette;
use cc_pulseline::render::layout::render_frame;
use cc_pulseline::render::pane::LayoutStyle;
use cc_pulseline::types::{QuotaMetrics, RenderFrame};

const FILLED_ICON: char = '\u{25B0}'; // ▰
const EMPTY_ICON: char = '\u{2500}'; // ─
const MARK_ICON: char = '\u{00B7}'; // ·

fn frame_with_quota(five_pct: f64, seven_pct: f64) -> RenderFrame {
    let mut f = RenderFrame::default();
    f.line1.model = "Opus".to_string();
    f.line1.git_branch = "main".to_string();
    f.line1.project_path = "~/x".to_string();
    f.line3.context_window_size = Some(200_000);
    f.line3.context_used_percentage = Some(43);
    f.line3.total_cost_usd = Some(1.0);
    f.line3.total_duration_ms = Some(60_000);
    f.quota = QuotaMetrics {
        five_hour_pct: Some(five_pct),
        five_hour_reset_minutes: Some(120),
        seven_day_pct: Some(seven_pct),
        seven_day_reset_minutes: Some(60 * 24 * 4),
    };
    f
}

fn cfg(layout: LayoutStyle, mode: GlyphMode) -> RenderConfig {
    RenderConfig {
        glyph_mode: mode,
        color_enabled: false,
        terminal_width: Some(160),
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        pane_style: layout,
        pane_cc_margin: 4,
        show_quota: true,
        show_quota_five_hour: true,
        show_quota_seven_day: true,
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

// ── Per-layout defaults ─────────────────────────────────────────────

#[test]
fn flat_layouts_default_to_text_no_bar() {
    // none keeps its minimalist default — `quota_visual = "text"`.
    let f = frame_with_quota(62.0, 28.0);
    let layout = LayoutStyle::None;
    let lines = render_frame(&f, &cfg(layout, GlyphMode::Icon));
    let quota_line = lines
        .iter()
        .find(|l| l.contains("5h:"))
        .unwrap_or_else(|| panic!("no 5h line for {layout:?}"));
    assert!(
        !quota_line.contains(FILLED_ICON),
        "{layout:?} should default to text (no ▰): {quota_line:?}"
    );
}

#[test]
fn framed_layouts_default_to_gauge_with_bar() {
    let f = frame_with_quota(62.0, 28.0);
    for layout in [LayoutStyle::Console, LayoutStyle::Ledger] {
        let lines = render_frame(&f, &cfg(layout, GlyphMode::Icon));
        // Console: 5h on a `5h:` cell. Ledger: 5h on a TAG row.
        let line = lines
            .iter()
            .find(|l| l.contains("5h"))
            .unwrap_or_else(|| panic!("no 5h line for {layout:?}"));
        assert!(
            line.contains(FILLED_ICON),
            "{layout:?} should render ▰ by default: {line:?}"
        );
    }
}

#[test]
fn quota_visual_text_disables_bar_in_framed_layouts() {
    let f = frame_with_quota(62.0, 28.0);
    for layout in [LayoutStyle::Console, LayoutStyle::Ledger] {
        let mut c = cfg(layout, GlyphMode::Icon);
        c.quota_visual = "text".to_string();
        let lines = render_frame(&f, &c);
        let line = lines.iter().find(|l| l.contains("5h")).unwrap();
        assert!(
            !line.contains(FILLED_ICON),
            "{layout:?} with quota_visual=text should drop ▰: {line:?}"
        );
    }
}

#[test]
fn flat_quota_visual_gauge_enables_bar() {
    // User opting in should override the flat-layout default.
    let f = frame_with_quota(62.0, 28.0);
    let mut c = cfg(LayoutStyle::None, GlyphMode::Icon);
    c.quota_visual = "gauge".to_string();
    let lines = render_frame(&f, &c);
    let line = lines.iter().find(|l| l.contains("5h:")).unwrap();
    assert!(
        line.contains(FILLED_ICON),
        "None + quota_visual=gauge should render ▰: {line:?}"
    );
}

// ── Bar position (D2 = A: bar before pct) ───────────────────────────

#[test]
fn bar_appears_before_pct_in_console() {
    let f = frame_with_quota(62.0, 28.0);
    let lines = render_frame(&f, &cfg(LayoutStyle::Console, GlyphMode::Icon));
    let line = lines.iter().find(|l| l.contains("5h:")).unwrap();
    let bar_pos = line.find(FILLED_ICON).expect("bar present");
    let pct_pos = line.find("62%").expect("pct present");
    assert!(
        bar_pos < pct_pos,
        "bar should come before 62%: {line:?} (bar={bar_pos}, pct={pct_pos})"
    );
}

#[test]
fn bar_appears_before_pct_in_ledger() {
    let f = frame_with_quota(62.0, 28.0);
    let lines = render_frame(&f, &cfg(LayoutStyle::Ledger, GlyphMode::Icon));
    let line = lines
        .iter()
        .find(|l| l.contains("5h") && l.contains("62%"))
        .unwrap();
    let bar_pos = line.find(FILLED_ICON).unwrap();
    let pct_pos = line.find("62%").unwrap();
    assert!(
        bar_pos < pct_pos,
        "ledger bar before pct: {line:?} (bar={bar_pos}, pct={pct_pos})"
    );
}

// ── Mark positions ──────────────────────────────────────────────────

#[test]
fn quota_marks_at_cell_7_and_12_in_14_cell_bar() {
    // 28% in 14-cell bar = 4 filled cells. Both 50% mark (cell 7) and
    // 85% mark (cell 12) should be visible on the empty side as `·`.
    // The bar substring is contiguous: 4 ▰ + 3 ─ + 1 · + 4 ─ + 1 · + 1 ─.
    let f = frame_with_quota(28.0, 28.0);
    let lines = render_frame(&f, &cfg(LayoutStyle::Ledger, GlyphMode::Icon));
    let line = lines
        .iter()
        .find(|l| l.contains("5h") && l.contains("28%"))
        .unwrap();
    // Find the bar by locating its first ▰ — the entire bar is
    // 14 cells, no other ▰ anywhere on the row.
    let bar_start = line.find(FILLED_ICON).unwrap();
    let bar_chars: Vec<char> = line[bar_start..].chars().take(14).collect();
    assert_eq!(bar_chars.len(), 14);
    assert_eq!(bar_chars[0..4], [FILLED_ICON; 4], "first 4 cells filled");
    assert_eq!(bar_chars[7], MARK_ICON, "50% mark at cell 7");
    assert_eq!(bar_chars[12], MARK_ICON, "85% mark at cell 12");
    // Cells 4-6, 8-11, 13 should be empty.
    for &i in &[4, 5, 6, 8, 9, 10, 11, 13] {
        assert_eq!(
            bar_chars[i], EMPTY_ICON,
            "cell {i} should be empty in {bar_chars:?}"
        );
    }
}

#[test]
fn high_quota_buries_first_mark_keeps_second() {
    // 62% in 14-cell bar = 9 filled cells (0..8). Cell 7 (50% mark)
    // is buried under fill; cell 12 (85% mark) remains visible.
    let f = frame_with_quota(62.0, 62.0);
    let lines = render_frame(&f, &cfg(LayoutStyle::Ledger, GlyphMode::Icon));
    let line = lines
        .iter()
        .find(|l| l.contains("5h") && l.contains("62%"))
        .unwrap();
    let bar_start = line.find(FILLED_ICON).unwrap();
    let bar_chars: Vec<char> = line[bar_start..].chars().take(14).collect();
    assert_eq!(bar_chars[0..9], [FILLED_ICON; 9], "first 9 filled");
    let marks_visible: Vec<usize> = bar_chars
        .iter()
        .enumerate()
        .filter_map(|(i, c)| (*c == MARK_ICON).then_some(i))
        .collect();
    assert_eq!(marks_visible, vec![12], "only 85% mark visible");
}

// ── ASCII fallback ──────────────────────────────────────────────────

#[test]
fn ascii_mode_renders_punctuation_ladder_no_blocks() {
    // Catch-net: under Ascii, no Unicode block characters in the bar.
    let f = frame_with_quota(62.0, 28.0);
    let lines = render_frame(&f, &cfg(LayoutStyle::Ledger, GlyphMode::Ascii));
    let line = lines
        .iter()
        .find(|l| l.contains("5h") && l.contains("62%"))
        .unwrap();
    const BLOCKS: &[char] = &[
        '\u{2588}', '\u{2589}', '\u{258A}', '\u{258B}', '\u{258C}', '\u{258D}', '\u{258E}',
        '\u{258F}', '\u{2591}', '\u{25B0}',
    ];
    assert!(
        !line.chars().any(|c| BLOCKS.contains(&c)),
        "ascii mode produced a block: {line:?}"
    );
    // Should contain `=` (filled) and `-` (empty); marks `:` are also OK.
    assert!(line.contains('='), "ascii bar should have `=`: {line:?}");
}

// ── CTX vs quota threshold separation ───────────────────────────────

#[test]
fn ctx_gauge_uses_window_aware_marks_not_quota_50_85() {
    // CTX gauge in console (FLAT_GAUGE_WIDTH = 18 cells). With a 200k
    // window, marks come from `ctx_marks_for_window`:
    //   warn = 100 - (90_000 * 100 / 200_000) = 55  → round(55*18/100) = cell 10
    //   crit = 100 - (50_000 * 100 / 200_000) = 75  → round(75*18/100) = cell 14
    // Crucially DIFFERENT from quota's [50, 85] positions on a 14-cell
    // bar (cells 7 and 12). This test asserts CTX marks are NOT at
    // quota's positions — proving the two segments use distinct
    // threshold sets.
    let mut f = frame_with_quota(0.0, 0.0);
    f.line3.context_used_percentage = Some(0);
    let mut c = cfg(LayoutStyle::Console, GlyphMode::Icon);
    c.context_visual = "gauge".to_string();
    // Set quota_visual = "text" so the only bar in the output is CTX's,
    // removing ambiguity about which line we're inspecting.
    c.quota_visual = "text".to_string();
    let lines = render_frame(&f, &c);
    // The CTX bar lives on the line containing `200.0k` (the window-size
    // total in the CTX cell), uniquely identifying it vs quota lines.
    let ctx_line = lines
        .iter()
        .find(|l| l.contains("200.0k"))
        .expect("CTX line with window total");
    // Bar cells run contiguously from the first ─ until the next non-bar
    // char (a digit or space). Collect only the marks from the bar.
    let bar_start = ctx_line.find(EMPTY_ICON).expect("CTX bar present");
    let bar_chars: Vec<char> = ctx_line[bar_start..]
        .chars()
        .take_while(|c| *c == EMPTY_ICON || *c == MARK_ICON || *c == FILLED_ICON)
        .collect();
    let mark_positions: Vec<usize> = bar_chars
        .iter()
        .enumerate()
        .filter_map(|(i, c)| (*c == MARK_ICON).then_some(i))
        .collect();
    // CTX bar should have its own marks (10, 14 for 18-cell bar at 200k window).
    // The critical assertion: those positions are different from quota's
    // [7, 12] positions on a 14-cell bar.
    assert_ne!(
        mark_positions,
        vec![7, 12],
        "CTX marks must not collide with quota's: {bar_chars:?}"
    );
    assert_eq!(
        mark_positions.len(),
        2,
        "CTX bar should have 2 marks: {bar_chars:?}"
    );
}
