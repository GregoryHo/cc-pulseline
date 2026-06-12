//! Ledger — label-value pairs aligned in a fixed left column.
//!
//! Each metric occupies its own row, prefixed by a 6-char TAG column.
//! Blank rows separate logical groups (ENV / CTX-TOK-COST / 5h-7d /
//! TOOL / AGENT-TODO). The layout owns its full pipeline (framed).
//!
//! ```text
//!   ╭─ <identity> ──────────────────────────────────────╮
//!   │  ENV     󰈙 2 CLAUDE.md   󰱇 10 rules   ...         │
//!   │                                                   │
//!   │  CTX     43%   86.0k / 200.0k                     │
//!   │  TOK     1 in   8 out   5.7k / 114.5k cache       │
//!   │  COST    $4.56   $4.42/h                          │
//!   │  5h      62%   resets 1h 59m                      │
//!   │  7d      28%   resets 4d 23h 59m                  │
//!   │                                                   │
//!   │  TOOL    ✓ Read ×2   ✓ Bash ×1                    │
//!   │          ▶ Read   .../console.rs                  │
//!   │                                                   │
//!   │  AGENT   󱦻 Explore   ...   [haiku]   <1s          │
//!   │  TODO    0/3 done · 3 pending                     │
//!   ╰───────────────────────────────────────────────────╯
//! ```
//!
//! Blank rows separate three boundaries: Config → Budget (after ENV),
//! Quota → Activity-tools (after 7d), and Activity-tools →
//! Activity-actors (after the last tool row, before AGENT). Other
//! transitions pack consecutively. The bottom frame closes flush
//! against the last AGENT/TODO row — no trailing blank padding.
//!
//! The CTX row appends a 6-cell braille sparkline + delta-time tail
//! (`30→43% in 5m`) when `context_visual` includes `sparkline` (the
//! ledger default). Sparkline color tracks CTX consumption *velocity*
//! via `widgets::sparkline::aurora_for_velocity`.
//!
//! Below 90 cols ledger falls back to `console` so the user gets
//! readable output rather than a mangled frame.

use crate::config::{GlyphMode, RenderConfig};
use crate::render::activity::budget::pack_with_separator;
use crate::render::activity::builder::{
    build_agent_cells, TodoVisualSpec, ToolsVisualSpec, ROW_SEPARATOR, ROW_SEPARATOR_W,
};
use crate::render::activity::cell::CellPriority;
use crate::render::activity::cells::recent_tool::target_strategy_for;
use crate::render::activity::truncate;
use crate::render::color::{colorize, visible_width, ThemePalette};
use crate::render::fmt::{format_number, format_reset_duration};
use crate::render::layout;
use crate::render::widgets;
use crate::types::{Line1Metrics, Line3Metrics, RenderFrame};

use super::shared::{self, FrameGlyphs};

/// Visual density: 6 braille cells × 2 samples per cell = 12 samples.
const SPARK_TARGET_SAMPLES: usize = 12;
/// Adaptive 1-minute floor: if 12 samples cover < 60 s of wall time,
/// expand the window backward until ≥ 60 s or out of samples.
const SPARK_MIN_WINDOW_MS: u64 = 60_000;

/// Cells consumed by the surrounding frame: `│  ` (3) on the left and
/// ` │` (2) on the right = 5. Subtract from terminal width to get the
/// inside-the-frame interior cells available for content + padding.
const FRAME_INNER_PAD: usize = 5;
/// `2-space indent + 6-char TAG + 3-space gap` = 11 cells before
/// content starts on a TAG-anchored row.
const TAG_INDENT: usize = 2;
const TAG_WIDTH: usize = 6;
const TAG_GAP: usize = 3;
/// Cells consumed by the TAG column on a tagged row. Continuation
/// rows (no TAG) reserve the same width with spaces so content lines
/// up across rows.
const TAG_COL_WIDTH: usize = TAG_INDENT + TAG_WIDTH + TAG_GAP;
/// Spacing between data items inside a content cell.
const ITEM_GAP: &str = "   ";
const ITEM_GAP_W: usize = 3;
/// Cells reserved on the right edge so truncated content doesn't kiss
/// the frame border.
const RIGHT_MARGIN: usize = 3;

/// Per-render context bundle, threaded through every row builder. Bundles
/// the four values (palette, glyph table, interior cell count, color flag)
/// that every helper needs.
struct LedgerCtx<'a> {
    p: &'a ThemePalette,
    g: &'static FrameGlyphs,
    inner: usize,
    color: bool,
}

pub fn render(frame: &RenderFrame, config: &RenderConfig, p: &ThemePalette) -> Vec<String> {
    // Ledger needs a known terminal width — its rows are fixed-width
    // framed. When detection fails (statusline hook context with no
    // accessible /dev/tty), assume `pane_max_width` rather than falling
    // back — falling back loses the user's chosen ledger layout for what
    // is the common-case invocation context. Only when the terminal is
    // *known* to be narrower than the TAG-column rhythm does Console win.
    let width = config
        .terminal_width
        .map(|w| w.min(config.pane_max_width))
        .unwrap_or(config.pane_max_width);
    if width < 90 {
        return fallback_to_console(frame, config);
    }

    let inner = width.saturating_sub(FRAME_INNER_PAD);
    // Body budget after the TAG column.
    let content_width = inner.saturating_sub(TAG_COL_WIDTH);

    let ctx = LedgerCtx {
        p,
        g: shared::glyphs(config.glyph_mode),
        inner,
        color: config.color_enabled,
    };
    let mut lines: Vec<String> = Vec::with_capacity(16);

    lines.push(top_frame(&frame.line1, config, &ctx));

    // Lazy separator: blank row pushed before each non-empty group after the
    // first, so no trailing blank can reach `bottom_frame`.
    let mut groups: Vec<Vec<String>> = Vec::with_capacity(4);

    // G1 ENV
    if shared::config_row_enabled(config) {
        let body = env_row_body(frame, config, p, content_width);
        if !body.is_empty() {
            groups.push(vec![framed_tag_row("ENV", &body, &ctx)]);
        }
    }

    // G2 Budget + Quota — CTX/TOK/COST/5h/7d are visually packed (no
    // blank between Budget and Quota in the legacy output), so we treat
    // them as a single group.
    let mut budget_quota: Vec<String> = Vec::with_capacity(5);
    if config.show_context {
        let body = ctx_row_body(
            &frame.line3,
            &frame.ctx_history,
            config.effective_context_visual(),
            config.glyph_mode,
            p,
            ctx.color,
        );
        if !body.is_empty() {
            budget_quota.push(framed_tag_row("CTX", &body, &ctx));
        }
    }
    if config.show_tokens {
        let body = tok_row_body(&frame.line3, p, ctx.color);
        if !body.is_empty() {
            budget_quota.push(framed_tag_row("TOK", &body, &ctx));
        }
    }
    if config.show_cost {
        let body = cost_row_body(&frame.line3, p, ctx.color);
        if !body.is_empty() {
            budget_quota.push(framed_tag_row("COST", &body, &ctx));
        }
    }
    if config.show_quota && frame.quota.has_data() {
        if config.show_quota_five_hour {
            if let Some(body) = quota_row_body(
                frame.quota.five_hour_pct,
                frame.quota.five_hour_reset_minutes,
                config,
                p,
                ctx.color,
            ) {
                budget_quota.push(framed_tag_row("5h", &body, &ctx));
            }
        }
        if config.show_quota_seven_day {
            if let Some(body) = quota_row_body(
                frame.quota.seven_day_pct,
                frame.quota.seven_day_reset_minutes,
                config,
                p,
                ctx.color,
            ) {
                budget_quota.push(framed_tag_row("7d", &body, &ctx));
            }
        }
    }
    if !budget_quota.is_empty() {
        groups.push(budget_quota);
    }

    // G3 Tools
    let tool_rows = build_tool_rows(frame, config, p, content_width, ctx.color);
    if !tool_rows.is_empty() {
        let mut tools: Vec<String> = Vec::with_capacity(tool_rows.len());
        for (i, body) in tool_rows.iter().enumerate() {
            let tag = if i == 0 { "TOOL" } else { "" };
            tools.push(framed_tag_row(tag, body, &ctx));
        }
        groups.push(tools);
    }

    // G4 Actors (AGENT rows + TODO — visually packed)
    let mut actors: Vec<String> = Vec::new();
    if config.show_agents {
        let agent_rows = build_agent_rows(frame, config, p, content_width);
        for (i, body) in agent_rows.iter().enumerate() {
            let tag = if i == 0 { "AGENT" } else { "" };
            actors.push(framed_tag_row(tag, body, &ctx));
        }
    }
    if config.show_todo {
        if let Some(body) = todo_row_body(frame, config, p, ctx.color) {
            actors.push(framed_tag_row("TODO", &body, &ctx));
        }
    }
    if !actors.is_empty() {
        groups.push(actors);
    }

    let dense = config.pane_ledger_dense;
    for (i, group) in groups.into_iter().enumerate() {
        if i > 0 && !dense {
            lines.push(blank_row(&ctx));
        }
        lines.extend(group);
    }

    lines.push(bottom_frame(&ctx));
    lines
}

/// Drop down to the next-best layout when ledger can't fit (or width
/// detection failed). Console (framed, identity-in-title) preserves
/// the title-in-frame look and is content-sized — never overflows the
/// terminal even when `terminal_width` is `None`.
fn fallback_to_console(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let mut shrunk = config.clone();
    shrunk.pane_style = crate::render::pane::LayoutStyle::Console;
    // The outer `render_frame` call already subtracted `pane_cc_margin` from
    // `terminal_width` before dispatching to ledger. Restoring it here
    // prevents the re-entrant `render_frame` from subtracting it a second
    // time and shrinking the fallback render by `cc_margin` cells.
    if let Some(w) = shrunk.terminal_width {
        shrunk.terminal_width = Some(w + shrunk.pane_cc_margin);
    }
    layout::render_frame(frame, &shrunk)
}

/// `╭─ {head} {dashes}─╮` — fixed overhead = 3 (`╭─ `) + 1 (sep) + 2 (`─╮`)
/// = 6 cells. Reserving 1 dash for visual continuity leaves headline
/// budget = `ctx.inner - 5`.
const HEADLINE_FRAME_OVERHEAD: usize = 5;

/// Fraction of headline budget allocated to path / branch when stage C
/// compression kicks in. Generous enough that a compressed `~/…/leaf`
/// always fits, but tight enough to leave room for model + pills.
/// Floors guarantee a useful leaf even at narrow terminals.
const PATH_BUDGET_DIVISOR: usize = 3;
const PATH_BUDGET_FLOOR: usize = 12;
const BRANCH_BUDGET_DIVISOR: usize = 4;
const BRANCH_BUDGET_FLOOR: usize = 8;

fn top_frame(line1: &Line1Metrics, config: &RenderConfig, ctx: &LedgerCtx) -> String {
    let budget = ctx.inner.saturating_sub(HEADLINE_FRAME_OVERHEAD);

    // Cascade prefers data compression (ellipsis on path/branch — visibly
    // recoverable) before pill drop (toggle off — entire segment vanishes).
    // A user who explicitly enabled `show_version` shouldn't lose the CC:
    // pill just because their project path is long; long paths should turn
    // into `~/…/leaf` first.

    // Stage 1: full headline, no compression, no pill drop.
    let mut head = shared::identity_headline(line1, config, ctx.p, " · ");
    let mut head_w = visible_width(&head);

    if head_w > budget {
        // Stage 2: compress project_path.
        let mut compressed = line1.clone();
        let path_budget = (budget / PATH_BUDGET_DIVISOR).max(PATH_BUDGET_FLOOR);
        compressed.project_path =
            truncate::compress_path_segments(&line1.project_path, path_budget);
        head = shared::identity_headline(&compressed, config, ctx.p, " · ");
        head_w = visible_width(&head);

        if head_w > budget {
            // Stage 3: compress git_branch (stacks on the path compression).
            let branch_budget = (budget / BRANCH_BUDGET_DIVISOR).max(BRANCH_BUDGET_FLOOR);
            compressed.git_branch =
                truncate::compress_path_segments(&line1.git_branch, branch_budget);
            head = shared::identity_headline(&compressed, config, ctx.p, " · ");
            head_w = visible_width(&head);

            if head_w > budget {
                // Stage 4: drop pills via DROP_ORDER on the already-compressed line1.
                head = shared::identity_headline_bounded(&compressed, config, ctx.p, " · ", budget);
                head_w = visible_width(&head);
            }
        }
    }

    // Safety net: tail-ellipsis the ANSI-colored headline when even pill
    // drop on top of full data compression couldn't bring it under budget.
    if head_w > budget {
        head = layout::truncate_to_width(&head, budget, ctx.color);
        head_w = visible_width(&head);
    }

    let dashes_after = ctx.inner.saturating_sub(head_w + 4);
    let lhs = colorize(
        &format!("{}{} ", ctx.g.tl, ctx.g.h),
        &ctx.p.separator,
        ctx.color,
    );
    let rhs_dashes = colorize(&ctx.g.h.repeat(dashes_after), &ctx.p.separator, ctx.color);
    let rhs = colorize(
        &format!("{}{}", ctx.g.h, ctx.g.tr),
        &ctx.p.separator,
        ctx.color,
    );
    format!("{lhs}{head} {rhs_dashes}{rhs}")
}

fn bottom_frame(ctx: &LedgerCtx) -> String {
    let dashes = colorize(&ctx.g.h.repeat(ctx.inner), &ctx.p.separator, ctx.color);
    let lhs = colorize(ctx.g.bl, &ctx.p.separator, ctx.color);
    let rhs = colorize(ctx.g.br, &ctx.p.separator, ctx.color);
    format!("{lhs}{dashes}{rhs}")
}

fn blank_row(ctx: &LedgerCtx) -> String {
    let bar = colorize(ctx.g.v, &ctx.p.separator, ctx.color);
    format!("{bar}{}{bar}", " ".repeat(ctx.inner))
}

/// `│  TAG     <body>          │` — TAG empty for continuation rows.
fn framed_tag_row(tag: &str, body: &str, ctx: &LedgerCtx) -> String {
    let bar = colorize(ctx.g.v, &ctx.p.separator, ctx.color);
    let tag_cell = if tag.is_empty() {
        " ".repeat(TAG_COL_WIDTH)
    } else {
        let padded = format!("{tag:<width$}", tag = tag, width = TAG_WIDTH);
        let coloured = colorize(&padded, &ctx.p.tag_label, ctx.color);
        format!(
            "{}{}{}",
            " ".repeat(TAG_INDENT),
            coloured,
            " ".repeat(TAG_GAP)
        )
    };
    let pad = ctx
        .inner
        .saturating_sub(TAG_COL_WIDTH + visible_width(body));
    format!("{bar}{tag_cell}{body}{}{bar}", " ".repeat(pad))
}

// ---------------------------------------------------------------------------
// Per-row content builders. Each returns the body string (no TAG prefix,
// no frame). Empty strings => caller drops the row.
// ---------------------------------------------------------------------------

fn env_row_body(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    max_width: usize,
) -> String {
    // ENV uses three-space rhythm between segments instead of L2's pipe.
    // `format_line2` is reused with our separator.
    let body = layout::format_line2(frame, config, ITEM_GAP, p);
    if body.is_empty() {
        return String::new();
    }
    if visible_width(&body) <= max_width {
        return body;
    }
    // Width-aware: if it overflows, drop to two-space gap before
    // resorting to the same drop-order config_row uses.
    let tighter = layout::format_line2(frame, config, "  ", p);
    if visible_width(&tighter) <= max_width {
        return tighter;
    }
    // shared::config_row already does aggressive low-value-first drop.
    shared::config_row(frame, config, p, max_width)
}

fn ctx_row_body(
    line3: &Line3Metrics,
    history: &[(u8, u64)],
    visual: &str,
    mode: GlyphMode,
    p: &ThemePalette,
    color: bool,
) -> String {
    let Some(pct) = line3.context_used_percentage else {
        return String::new();
    };
    let size = line3.context_window_size.unwrap_or(0);
    let pct_color = p.color_for_ctx_pct(pct);
    let pct_str = colorize(&format!("{pct}%"), pct_color, color);
    let used = ((size as f64) * (pct as f64) / 100.0) as u64;
    let used_str = colorize(&format_number(used), &p.primary, color);
    let slash = colorize("/", &p.separator, color);
    let total_str = colorize(&format_number(size), &p.primary, color);

    // Bar precedes the percentage when `context_visual` includes `gauge`
    // (matches the D2 convention used by quota's bar).
    let bar = if shared::spec_has(visual, shared::WIDGET_GAUGE) {
        let marks = ThemePalette::ctx_marks();
        widgets::gauge::render(
            pct,
            shared::CTX_BAR_WIDTH,
            &marks,
            pct_color,
            p,
            mode,
            color,
        )
    } else {
        String::new()
    };
    let bar_part = if bar.is_empty() {
        String::new()
    } else {
        format!("{bar}{ITEM_GAP}")
    };
    let mut out = format!("{bar_part}{pct_str}{ITEM_GAP}{used_str} {slash} {total_str}");

    let wants_sparkline = shared::spec_has(visual, shared::WIDGET_SPARKLINE);
    if wants_sparkline {
        let window = sparkline_window(history);
        if !window.is_empty() {
            let fill = widgets::sparkline::aurora_for_velocity(window, p);
            let glyph = widgets::sparkline::render(window, fill, mode, color);
            if !glyph.is_empty() {
                out.push_str(ITEM_GAP);
                out.push_str(&glyph);
            }
            // Delta-time label always rendered — text carries the trend
            // even under Ascii where the braille glyph drops out.
            if let Some(label) = sparkline_delta_label(window) {
                let coloured = colorize(&label, &p.structural, color);
                out.push_str(ITEM_GAP);
                out.push_str(&coloured);
            }
        }
    }
    out
}

/// Slice of `history` that the ledger sparkline draws — most-recent 12
/// samples, expanded back if those 12 cover < 60 s of wall time.
fn sparkline_window(history: &[(u8, u64)]) -> &[(u8, u64)] {
    let n = history.len();
    if n == 0 {
        return history;
    }
    let now = history.last().map(|(_, t)| *t).unwrap_or(0);
    let mut take = SPARK_TARGET_SAMPLES.min(n).max(1);
    while take < n {
        let oldest_ts = history[n - take].1;
        if now.saturating_sub(oldest_ts) >= SPARK_MIN_WINDOW_MS {
            break;
        }
        take += 1;
    }
    &history[n - take..]
}

/// `30→43% in 5m`. Returns `None` for windows shorter than ~1 sample
/// of meaningful elapsed time.
fn sparkline_delta_label(window: &[(u8, u64)]) -> Option<String> {
    let (Some(first), Some(last)) = (window.first(), window.last()) else {
        return None;
    };
    if window.len() < 2 {
        return None;
    }
    let span_ms = last.1.saturating_sub(first.1);
    if span_ms == 0 {
        return None;
    }
    Some(format!(
        "{}→{}% in {}",
        first.0,
        last.0,
        format_window_duration(span_ms)
    ))
}

/// Granular elapsed-time format for ledger sparkline:
/// - `< 60 s`     → `47s` (seconds — fmt::format_duration starts at 1m)
/// - `< 60 min`   → `5m`
/// - `< 24 h`     → `1h` or `1h 5m`
/// - `≥ 24 h`     → `1d+`
fn format_window_duration(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    let days = mins / 1440;
    if days >= 1 {
        return "1d+".to_string();
    }
    let hours = mins / 60;
    let leftover = mins % 60;
    if hours == 0 {
        format!("{mins}m")
    } else if leftover == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {leftover}m")
    }
}

fn tok_row_body(line3: &Line3Metrics, p: &ThemePalette, color: bool) -> String {
    let has_data = line3.input_tokens.is_some()
        || line3.output_tokens.is_some()
        || line3.cache_creation_tokens.is_some()
        || line3.cache_read_tokens.is_some();
    if !has_data {
        return String::new();
    }
    let val_color = &p.primary;
    let in_lbl = colorize("in", &p.structural, color);
    let out_lbl = colorize("out", &p.structural, color);
    let cache_lbl = colorize("cache", &p.structural, color);
    let in_v = colorize(
        &line3
            .input_tokens
            .map(format_number)
            .unwrap_or_else(|| "--".into()),
        val_color,
        color,
    );
    let out_v = colorize(
        &line3
            .output_tokens
            .map(format_number)
            .unwrap_or_else(|| "--".into()),
        val_color,
        color,
    );
    let slash = colorize("/", &p.separator, color);
    let create_v = colorize(
        &format_number(line3.cache_creation_tokens.unwrap_or(0)),
        val_color,
        color,
    );
    let read_v = colorize(
        &format_number(line3.cache_read_tokens.unwrap_or(0)),
        val_color,
        color,
    );
    // Cache hit rate appended after the raw pair — `50% hit`,
    // threshold-colored; omitted entirely when there is no cache signal.
    let hit_part = line3
        .cache_hit_pct()
        .map(|pct| {
            let pct_v = colorize(
                &format!("{pct:.0}%"),
                p.color_for_cache_hit_pct(pct, line3.cache_creation_share()),
                color,
            );
            let hit_lbl = colorize("hit", &p.structural, color);
            format!("  {pct_v} {hit_lbl}")
        })
        .unwrap_or_default();
    format!(
        "{in_v} {in_lbl}{ITEM_GAP}{out_v} {out_lbl}{ITEM_GAP}{create_v} {slash} {read_v} {cache_lbl}{hit_part}"
    )
}

fn cost_row_body(line3: &Line3Metrics, p: &ThemePalette, color: bool) -> String {
    let total = line3.total_cost_usd.unwrap_or(0.0);
    let per_hour = line3
        .total_duration_ms
        .filter(|d| *d > 0)
        .map(|d| total / ((d as f64) / 3_600_000.0))
        .unwrap_or(0.0);
    let total_str = colorize(&format!("${total:.2}"), &p.cost_base, color);
    let rate_color = p.color_for_burn_rate(per_hour);
    let rate_str = colorize(&format!("${per_hour:.2}/h"), rate_color, color);
    format!("{total_str}{ITEM_GAP}{rate_str}")
}

fn quota_row_body(
    pct: Option<f64>,
    reset_minutes: Option<u64>,
    config: &RenderConfig,
    p: &ThemePalette,
    color: bool,
) -> Option<String> {
    let pct_val = pct?;
    let pct_color = p.color_for_quota_pct(pct_val);
    let pct_str = if pct_val >= 100.0 {
        colorize("Limit reached", p.ctx_critical(), color)
    } else {
        colorize(&format!("{pct_val:.0}%"), pct_color, color)
    };
    // Bar precedes the percentage (D2). Empty string when
    // `quota_visual` doesn't include `gauge` — caller falls through
    // to text-only rendering.
    let bar = shared::render_quota_visual(
        config.effective_quota_visual(),
        pct_val,
        p,
        config.glyph_mode,
        color,
    );
    let resets_str = reset_minutes
        .map(|m| {
            let dur = format_reset_duration(m);
            colorize(&format!("resets {dur}"), &p.structural, color)
        })
        .unwrap_or_default();

    let mut parts: Vec<&str> = Vec::with_capacity(3);
    if !bar.is_empty() {
        parts.push(&bar);
    }
    parts.push(&pct_str);
    if !resets_str.is_empty() {
        parts.push(&resets_str);
    }
    Some(parts.join(ITEM_GAP))
}

/// Running-tool arrow. `▶` in icon mode, `>` in Ascii.
const ICON_RUNNING: (&str, &str) = ("\u{25B6}", ">");

fn build_tool_rows(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    max_width: usize,
    color: bool,
) -> Vec<String> {
    if !config.show_tools {
        return Vec::new();
    }
    // Same `tools_visual` contract as the flat builder: `ticker` fuses
    // everything onto one TOOL row; otherwise `counts`/`targets` gate
    // their respective rows.
    let spec = ToolsVisualSpec::parse(config.effective_tools_visual());
    if spec.ticker {
        return build_tool_ticker_row(frame, config, p, max_width, color);
    }
    let mut rows: Vec<String> = Vec::with_capacity(1 + config.max_tool_lines.max(1));

    let counts = &frame.completed_tools;
    if spec.show_counts && !counts.is_empty() {
        let parts: Vec<String> = counts
            .iter()
            .take(config.max_completed_tools.max(1))
            .map(|c| {
                let check = colorize("\u{2713}", &p.completed_check, color);
                let name = colorize(&c.name, &p.completed_check, color);
                let count = colorize(&format!("\u{00D7}{}", c.count), &p.completed_check, color);
                format!("{check} {name} {count}")
            })
            .collect();
        if !parts.is_empty() {
            rows.push(parts.join(ITEM_GAP));
        }
    }

    if !spec.show_targets {
        return rows;
    }
    let arrow_glyph = match config.glyph_mode {
        GlyphMode::Icon => ICON_RUNNING.0,
        GlyphMode::Ascii => ICON_RUNNING.1,
    };
    let arrow_w = visible_width(arrow_glyph) + 1; // arrow + trailing space
    for t in frame.tools.iter().take(config.max_tool_lines.max(1)) {
        let name_w = t.name.chars().count();
        let arrow = colorize(arrow_glyph, p.tool_blue(), color);
        let name = colorize(&t.name, p.tool_blue(), color);
        let target = match &t.target {
            Some(tgt) => {
                let safe = crate::render::fmt::sanitize_single_line(tgt);
                // Truncate to fit `max_width`. Without this, a long Bash
                // target (e.g. a heredoc-bearing `git commit` command)
                // can blow a 100+ cell line into 800+ cells, overflow
                // the terminal, and trigger CC's wrap-collapse to one
                // visible row.
                let budget = max_width.saturating_sub(arrow_w + name_w + ITEM_GAP_W + RIGHT_MARGIN);
                let (strategy, _ideal) = target_strategy_for(&t.name);
                let truncated = truncate::apply(strategy, &safe, budget);
                format!("{ITEM_GAP}{}", colorize(&truncated, &p.secondary, color))
            }
            None => String::new(),
        };
        rows.push(format!("{arrow} {name}{target}"));
    }

    rows
}

/// The tools `ticker` atom in ledger idiom: completed grand total plus
/// running tools fused onto ONE row joined by `ITEM_GAP`. Only the first
/// running tool keeps its (truncated) target so the row stays bounded;
/// later tools render name-only.
fn build_tool_ticker_row(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    max_width: usize,
    color: bool,
) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut used = 0usize;

    if !frame.completed_tools.is_empty() {
        let total: u64 = frame.completed_tools.iter().map(|c| c.count as u64).sum();
        let noun = if total == 1 { "tool" } else { "tools" };
        let check = colorize("\u{2713}", &p.completed_check, color);
        let label = colorize(&format!(" {total} {noun}"), &p.completed_check, color);
        used += 2 + total.to_string().len() + 1 + noun.len();
        parts.push(format!("{check}{label}"));
    }

    let arrow_glyph = match config.glyph_mode {
        GlyphMode::Icon => ICON_RUNNING.0,
        GlyphMode::Ascii => ICON_RUNNING.1,
    };
    let arrow_w = visible_width(arrow_glyph) + 1;
    let shown: Vec<&crate::types::ToolSummary> = frame
        .tools
        .iter()
        .take(config.max_tool_lines.max(1))
        .collect();
    // Account for every name cell up front so the first tool's target
    // budget already reserves room for the cells that follow it.
    for t in &shown {
        used += ITEM_GAP_W + arrow_w + t.name.chars().count();
    }
    for (i, t) in shown.iter().enumerate() {
        let arrow = colorize(arrow_glyph, p.tool_blue(), color);
        let name = colorize(&t.name, p.tool_blue(), color);
        let target = match (&t.target, i) {
            (Some(tgt), 0) => {
                let safe = crate::render::fmt::sanitize_single_line(tgt);
                let budget = max_width.saturating_sub(used + ITEM_GAP_W + RIGHT_MARGIN);
                let (strategy, _ideal) = target_strategy_for(&t.name);
                let truncated = truncate::apply(strategy, &safe, budget);
                format!("{ITEM_GAP}{}", colorize(&truncated, &p.secondary, color))
            }
            _ => String::new(),
        };
        parts.push(format!("{arrow} {name}{target}"));
    }

    if parts.is_empty() {
        Vec::new()
    } else {
        vec![parts.join(ITEM_GAP)]
    }
}

/// Build per-row strings for the AGENT block by delegating to
/// `activity::builder::build_agent_cells`. Routing through the shared
/// builder gives ledger:
///   * correct completion icon/color (✓ + `completed_check` for finished
///     single agents — was hard-coded `ICON_AGENT`/`stable_blue` before)
///   * `agents_visual` toggle compliance (was ignored before)
///   * active-first-newest selection under `max_agent_lines` (the prior
///     `.take(max)` kept the OLDEST active groups)
fn build_agent_rows(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    max_width: usize,
) -> Vec<String> {
    if frame.agents.is_empty() {
        return Vec::new();
    }
    let cells = build_agent_cells(&frame.agents, config, p);
    let max = config.max_agent_lines.max(1);

    // Cells come back as `[active.., completed..]`. Under cap: keep the
    // newest `max` of active first, then fill remaining slots with newest
    // completed. Mirrors `activity::builder::build_agent_rows` so layouts
    // stay consistent.
    let active_count = cells
        .iter()
        .position(|c| c.priority == CellPriority::Optional)
        .unwrap_or(cells.len());
    let active = &cells[..active_count];
    let completed = &cells[active_count..];

    let active_keep = active.len().min(max);
    let active_skip = active.len() - active_keep;
    let remaining = max - active_keep;
    let completed_keep = completed.len().min(remaining);

    let color = config.color_enabled;
    let sep = colorize(ROW_SEPARATOR, &p.separator, color);
    let chosen = active
        .iter()
        .skip(active_skip)
        .chain(completed.iter().take(completed_keep));

    chosen
        .map(|cell| {
            pack_with_separator(
                std::slice::from_ref(cell),
                max_width,
                &sep,
                ROW_SEPARATOR_W,
                color,
            )
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn todo_row_body(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    color: bool,
) -> Option<String> {
    let todo = frame.todo.as_ref()?;
    let spec = TodoVisualSpec::parse(config.effective_todo_visual());
    let agent_suffix =
        crate::render::fmt::sub_agent_suffix(todo.sub_agent_count, &p.structural, color);
    if todo.all_done {
        let s = format!("\u{2713} All complete ({}/{})", todo.completed, todo.total);
        return Some(format!(
            "{}{}",
            colorize(&s, &p.completed_check, color),
            agent_suffix
        ));
    }
    let done = todo.completed.max(todo.total.saturating_sub(todo.pending));
    // `bar` atom: same 5-cell completed/total gauge as the flat builder.
    let bar = if spec.show_bar && todo.total > 0 {
        let pct = (done as u64) * 100 / (todo.total as u64);
        let g = widgets::gauge::render(pct, 5, &[], p.todo_teal(), p, config.glyph_mode, color);
        if g.is_empty() {
            String::new()
        } else {
            format!("{g} ")
        }
    } else {
        String::new()
    };
    let body = if spec.show_text {
        format!("{}/{} done · {} pending", done, todo.total, todo.pending)
    } else {
        format!("{}/{}", done, todo.total)
    };
    Some(format!(
        "{bar}{}{}",
        colorize(&body, p.todo_teal(), color),
        agent_suffix
    ))
}
