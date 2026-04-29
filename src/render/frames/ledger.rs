//! Ledger — label-value pairs aligned in a fixed left column.
//!
//! Each metric occupies its own row, prefixed by a 6-char TAG column.
//! Blank rows separate logical groups (ENV / CTX-TOK-COST / 5h-7d /
//! TOOL / AGENT-TODO). The layout owns its full pipeline (framed).
//!
//! ```text
//!   ╭─ <identity> ──────────────────────────────────────╮
//!   │                                                   │
//!   │  ENV     󰈙 2 CLAUDE.md   󰱇 10 rules   ...         │
//!   │                                                   │
//!   │  CTX     43%   86.0k / 200.0k                     │
//!   │  TOK     1 in   8 out   5.7k / 114.5k cache       │
//!   │  COST    $4.56   $4.42/h                          │
//!   │                                                   │
//!   │  5h      62%   resets 1h 59m                      │
//!   │  7d      28%   resets 4d 23h 59m                  │
//!   │                                                   │
//!   │  TOOL    ✓ Read ×2   ✓ Bash ×1                    │
//!   │          ▶ Read   .../console.rs                  │
//!   │                                                   │
//!   │  AGENT   󱦻 Explore   ...   [haiku]   <1s          │
//!   │  TODO    0/3 done · 3 pending                     │
//!   │                                                   │
//!   ╰───────────────────────────────────────────────────╯
//! ```
//!
//! The CTX row appends a 6-cell braille sparkline + delta-time tail
//! (`30→43% in 5m`) when `context_visual` includes `sparkline` (the
//! ledger default). Sparkline color tracks CTX consumption *velocity*
//! via `widgets::sparkline::aurora_for_velocity`.
//!
//! Below 90 cols ledger falls back to `sections` so the user gets
//! readable output rather than a mangled frame.

use crate::config::{GlyphMode, RenderConfig};
use crate::render::color::{colorize, visible_width, ThemePalette};
use crate::render::fmt::{format_number, format_reset_duration};
use crate::render::icons::{glyph, ICON_AGENT};
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
    // Ledger renders fixed-width framed rows, so it MUST know the
    // terminal width — otherwise a hardcoded default overflows narrower
    // terminals and CC's wrap-collapse behaviour hides every body row.
    // Fall back to sections (content-sized, never overflows) when width
    // detection fails or the terminal is too narrow for the TAG column
    // rhythm to read.
    let Some(width) = config.terminal_width.map(|w| w.min(config.pane_max_width)) else {
        return fallback_to_sections(frame, config);
    };
    if width < 90 {
        return fallback_to_sections(frame, config);
    }

    let inner = width.saturating_sub(FRAME_INNER_PAD);
    // `content_width` is the body budget AFTER the TAG column (indent +
    // 6-char tag + gap = 11 cells). Anything wider than this would push
    // the row past the right frame edge.
    let content_width = inner.saturating_sub(TAG_COL_WIDTH);

    let ctx = LedgerCtx {
        p,
        g: shared::glyphs(config.glyph_mode),
        inner,
        color: config.color_enabled,
    };
    let mut lines: Vec<String> = Vec::with_capacity(16);

    lines.push(top_frame(&frame.line1, config, &ctx));
    lines.push(blank_row(&ctx));

    if shared::config_row_enabled(config) {
        let body = env_row_body(frame, config, p, content_width);
        if !body.is_empty() {
            lines.push(framed_tag_row("ENV", &body, &ctx));
            lines.push(blank_row(&ctx));
        }
    }

    let mut budget_emitted = false;
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
            lines.push(framed_tag_row("CTX", &body, &ctx));
            budget_emitted = true;
        }
    }
    if config.show_tokens {
        let body = tok_row_body(&frame.line3, p, ctx.color);
        if !body.is_empty() {
            lines.push(framed_tag_row("TOK", &body, &ctx));
            budget_emitted = true;
        }
    }
    if config.show_cost {
        let body = cost_row_body(&frame.line3, p, ctx.color);
        if !body.is_empty() {
            lines.push(framed_tag_row("COST", &body, &ctx));
            budget_emitted = true;
        }
    }
    if budget_emitted {
        lines.push(blank_row(&ctx));
    }

    let mut quota_emitted = false;
    if config.show_quota && frame.quota.has_data() {
        if config.show_quota_five_hour {
            if let Some(body) = quota_row_body(
                frame.quota.five_hour_pct,
                frame.quota.five_hour_reset_minutes,
                p,
                ctx.color,
            ) {
                lines.push(framed_tag_row("5h", &body, &ctx));
                quota_emitted = true;
            }
        }
        if config.show_quota_seven_day {
            if let Some(body) = quota_row_body(
                frame.quota.seven_day_pct,
                frame.quota.seven_day_reset_minutes,
                p,
                ctx.color,
            ) {
                lines.push(framed_tag_row("7d", &body, &ctx));
                quota_emitted = true;
            }
        }
    }
    if quota_emitted {
        lines.push(blank_row(&ctx));
    }

    let tool_rows = build_tool_rows(frame, config, p, ctx.color);
    if !tool_rows.is_empty() {
        for (i, body) in tool_rows.iter().enumerate() {
            let tag = if i == 0 { "TOOL" } else { "" };
            lines.push(framed_tag_row(tag, body, &ctx));
        }
        lines.push(blank_row(&ctx));
    }

    let mut agent_todo_emitted = false;
    if config.show_agents {
        let agent_rows = build_agent_rows(frame, config, p);
        for (i, body) in agent_rows.iter().enumerate() {
            let tag = if i == 0 { "AGENT" } else { "" };
            lines.push(framed_tag_row(tag, body, &ctx));
            agent_todo_emitted = true;
        }
    }
    if config.show_todo {
        if let Some(body) = todo_row_body(frame, p, ctx.color) {
            lines.push(framed_tag_row("TODO", &body, &ctx));
            agent_todo_emitted = true;
        }
    }
    if agent_todo_emitted {
        lines.push(blank_row(&ctx));
    }

    lines.push(bottom_frame(&ctx));
    lines
}

/// Drop down to the next-best layout when ledger can't fit (or width
/// detection failed). Console (sections + identity-in-title) preserves
/// the title-in-frame look and is content-sized — never overflows the
/// terminal even when `terminal_width` is `None`.
fn fallback_to_sections(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let mut shrunk = config.clone();
    shrunk.pane_style = crate::render::pane::LayoutStyle::Console;
    layout::render_frame(frame, &shrunk)
}

fn top_frame(line1: &Line1Metrics, config: &RenderConfig, ctx: &LedgerCtx) -> String {
    let head = shared::identity_headline(line1, config, ctx.p, " · ");
    let head_w = visible_width(&head);
    let dashes_after = ctx.inner.saturating_sub(head_w + 4);
    let lhs = colorize(&format!("{}{} ", ctx.g.tl, ctx.g.h), &ctx.p.separator, ctx.color);
    let rhs_dashes = colorize(&ctx.g.h.repeat(dashes_after), &ctx.p.separator, ctx.color);
    let rhs = colorize(&format!("{}{}", ctx.g.h, ctx.g.tr), &ctx.p.separator, ctx.color);
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
        let coloured = colorize(&padded, &ctx.p.secondary, ctx.color);
        format!(
            "{}{}{}",
            " ".repeat(TAG_INDENT),
            coloured,
            " ".repeat(TAG_GAP)
        )
    };
    let pad = ctx.inner.saturating_sub(TAG_COL_WIDTH + visible_width(body));
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
    let pct_color = p.color_for_ctx_pct(pct, line3.context_window_size);
    let pct_str = colorize(&format!("{pct}%"), pct_color, color);
    let used = ((size as f64) * (pct as f64) / 100.0) as u64;
    let used_str = colorize(&format_number(used), &p.primary, color);
    let slash = colorize("/", &p.separator, color);
    let total_str = colorize(&format_number(size), &p.primary, color);
    let mut out = format!("{pct_str}{ITEM_GAP}{used_str} {slash} {total_str}");

    let wants_sparkline = visual.split('+').any(|w| w.trim() == "sparkline");
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
    format!(
        "{in_v} {in_lbl}{ITEM_GAP}{out_v} {out_lbl}{ITEM_GAP}{create_v} {slash} {read_v} {cache_lbl}"
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
    let resets_str = reset_minutes
        .map(|m| {
            let dur = format_reset_duration(m);
            colorize(&format!("resets {dur}"), &p.structural, color)
        })
        .unwrap_or_default();
    if resets_str.is_empty() {
        Some(pct_str)
    } else {
        Some(format!("{pct_str}{ITEM_GAP}{resets_str}"))
    }
}

/// Running-tool arrow. `▶` in icon mode, `>` in Ascii.
const ICON_RUNNING: (&str, &str) = ("\u{25B6}", ">");

fn build_tool_rows(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    color: bool,
) -> Vec<String> {
    if !config.show_tools {
        return Vec::new();
    }
    let mut rows: Vec<String> =
        Vec::with_capacity(1 + config.max_tool_lines.max(1));

    let counts = &frame.completed_tools;
    if !counts.is_empty() {
        let parts: Vec<String> = counts
            .iter()
            .take(config.max_completed_tools.max(1))
            .map(|c| {
                let check = colorize("\u{2713}", &p.completed_check, color);
                let name = colorize(&c.name, &p.completed_check, color);
                let count = colorize(
                    &format!("\u{00D7}{}", c.count),
                    &p.completed_check,
                    color,
                );
                format!("{check} {name} {count}")
            })
            .collect();
        if !parts.is_empty() {
            rows.push(parts.join(ITEM_GAP));
        }
    }

    let arrow_glyph = match config.glyph_mode {
        GlyphMode::Icon => ICON_RUNNING.0,
        GlyphMode::Ascii => ICON_RUNNING.1,
    };
    for t in frame.tools.iter().take(config.max_tool_lines.max(1)) {
        let arrow = colorize(arrow_glyph, p.tool_blue(), color);
        let name = colorize(&t.name, p.tool_blue(), color);
        // `sanitize_single_line` defends against newlines / tabs leaking
        // out of stale session caches written before the activity-row
        // builder started sanitising. CC's statusline parser splits on
        // `\n` — an unsanitised target shatters the frame.
        let target = match &t.target {
            Some(tgt) => {
                let safe = crate::render::fmt::sanitize_single_line(tgt);
                format!("{ITEM_GAP}{}", colorize(&safe, &p.secondary, color))
            }
            None => String::new(),
        };
        rows.push(format!("{arrow} {name}{target}"));
    }

    rows
}

fn build_agent_rows(frame: &RenderFrame, config: &RenderConfig, p: &ThemePalette) -> Vec<String> {
    if frame.agents.is_empty() {
        return Vec::new();
    }
    let color = config.color_enabled;
    let max = config.max_agent_lines.max(1);
    let icon_glyph = glyph(config.glyph_mode, ICON_AGENT, "A:");
    frame
        .agents
        .iter()
        .take(max)
        .map(|a| {
            let icon = colorize(&icon_glyph, &p.stable_blue, color);
            let name = colorize(
                a.agent_type.as_deref().unwrap_or("agent"),
                &p.stable_blue,
                color,
            );
            let desc = colorize(&a.description, &p.secondary, color);
            let model = a
                .model
                .as_ref()
                .map(|m| format!("{ITEM_GAP}{}", colorize(&format!("[{m}]"), &p.secondary, color)))
                .unwrap_or_default();
            format!("{icon} {name}{ITEM_GAP}{desc}{model}")
        })
        .collect()
}

fn todo_row_body(frame: &RenderFrame, p: &ThemePalette, color: bool) -> Option<String> {
    let todo = frame.todo.as_ref()?;
    if todo.all_done {
        let s = format!("\u{2713} All complete ({}/{})", todo.completed, todo.total);
        return Some(colorize(&s, &p.completed_check, color));
    }
    let done = todo.completed.max(todo.total.saturating_sub(todo.pending));
    let body = format!("{}/{} done · {} pending", done, todo.total, todo.pending);
    Some(colorize(&body, p.todo_teal(), color))
}

