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
//! Sparkline on the CTX row is wired in a follow-up commit; this
//! skeleton renders the typographic rhythm + TAG column + frame only.
//! Below 90 cols it falls back to `sections`.

use crate::config::{GlyphMode, RenderConfig};
use crate::render::color::{colorize, visible_width, ThemePalette};
use crate::render::fmt::{format_number, format_reset_duration};
use crate::render::icons::{FRAME_BL, FRAME_BR, FRAME_H, FRAME_TL, FRAME_TR, FRAME_V};
use crate::render::layout;
use crate::render::widgets;
use crate::types::{Line1Metrics, Line3Metrics, RenderFrame};

use super::shared;

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
/// Spacing between data items inside a content cell. 3 spaces reads as
/// ledger's "rhythm" — matches `ledger-v2.md` Round-1 decision.
const ITEM_GAP: &str = "   ";

pub fn render(frame: &RenderFrame, config: &RenderConfig, p: &ThemePalette) -> Vec<String> {
    let width = config
        .terminal_width
        .unwrap_or(140)
        .min(config.pane_max_width);

    // Below 90 cols ledger gets cramped — defer to sections.
    if width < 90 {
        return fallback_to_sections(frame, config);
    }

    let inner = width.saturating_sub(FRAME_INNER_PAD);
    let content_width = inner.saturating_sub(2); // leading "  " indent inside frame

    let color = config.color_enabled;
    let mut lines: Vec<String> = Vec::with_capacity(20);

    lines.push(top_frame(&frame.line1, config, p, inner));
    lines.push(blank_row(p, inner, color));

    // ENV row — only when at least one config-segment toggle is on AND
    // the assembled body is non-empty.
    if shared::config_row_enabled(config) {
        let body = env_row_body(frame, config, p, content_width);
        if !body.is_empty() {
            lines.push(framed_tag_row("ENV", &body, p, inner, color));
            lines.push(blank_row(p, inner, color));
        }
    }

    // Budget group: CTX / TOK / COST. Each row only emits if its toggle
    // and underlying data agree it has something to show.
    let mut budget_emitted = false;
    if config.show_context {
        let body = ctx_row_body(
            &frame.line3,
            &frame.ctx_history,
            config.effective_context_visual(),
            config.glyph_mode,
            p,
            color,
        );
        if !body.is_empty() {
            lines.push(framed_tag_row("CTX", &body, p, inner, color));
            budget_emitted = true;
        }
    }
    if config.show_tokens {
        let body = tok_row_body(&frame.line3, p, color);
        if !body.is_empty() {
            lines.push(framed_tag_row("TOK", &body, p, inner, color));
            budget_emitted = true;
        }
    }
    if config.show_cost {
        let body = cost_row_body(&frame.line3, p, color);
        if !body.is_empty() {
            lines.push(framed_tag_row("COST", &body, p, inner, color));
            budget_emitted = true;
        }
    }
    if budget_emitted {
        lines.push(blank_row(p, inner, color));
    }

    // Quota group: 5h / 7d.
    let mut quota_emitted = false;
    if config.show_quota && frame.quota.has_data() {
        if config.show_quota_five_hour {
            if let Some(body) = quota_row_body(
                frame.quota.five_hour_pct,
                frame.quota.five_hour_reset_minutes,
                p,
                color,
            ) {
                lines.push(framed_tag_row("5h", &body, p, inner, color));
                quota_emitted = true;
            }
        }
        if config.show_quota_seven_day {
            if let Some(body) = quota_row_body(
                frame.quota.seven_day_pct,
                frame.quota.seven_day_reset_minutes,
                p,
                color,
            ) {
                lines.push(framed_tag_row("7d", &body, p, inner, color));
                quota_emitted = true;
            }
        }
    }
    if quota_emitted {
        lines.push(blank_row(p, inner, color));
    }

    // Tools — completed counts on the TOOL row, then one continuation
    // row per running tool. Skipped entirely when no activity.
    let tool_rows = build_tool_rows(frame, config, p, content_width, color);
    if !tool_rows.is_empty() {
        for (i, body) in tool_rows.iter().enumerate() {
            let tag = if i == 0 { "TOOL" } else { "" };
            lines.push(framed_tag_row(tag, body, p, inner, color));
        }
        lines.push(blank_row(p, inner, color));
    }

    // Agents + Todo — share the same blank-row gate. Both can be empty.
    let mut agent_todo_emitted = false;
    if config.show_agents {
        let agent_rows = build_agent_rows(frame, config, p, content_width);
        for (i, body) in agent_rows.iter().enumerate() {
            let tag = if i == 0 { "AGENT" } else { "" };
            lines.push(framed_tag_row(tag, body, p, inner, color));
            agent_todo_emitted = true;
        }
    }
    if config.show_todo {
        if let Some(body) = todo_row_body(frame, p, color) {
            lines.push(framed_tag_row("TODO", &body, p, inner, color));
            agent_todo_emitted = true;
        }
    }
    if agent_todo_emitted {
        lines.push(blank_row(p, inner, color));
    }

    lines.push(bottom_frame(p, inner, color));
    lines
}

/// When ledger can't fit (`< 90` cols), fall back to `sections` so the
/// user gets readable output rather than a mangled frame.
fn fallback_to_sections(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let mut shrunk = config.clone();
    shrunk.pane_style = crate::render::pane::LayoutStyle::Sections;
    layout::render_frame(frame, &shrunk)
}

fn top_frame(line1: &Line1Metrics, config: &RenderConfig, p: &ThemePalette, inner: usize) -> String {
    let color = config.color_enabled;
    let head = shared::identity_headline(line1, config, p);
    let head_w = visible_width(&head);
    let dashes_after = inner.saturating_sub(head_w + 4);
    let lhs = colorize(&format!("{FRAME_TL}{FRAME_H} "), &p.separator, color);
    let rhs_dashes = colorize(
        &FRAME_H.to_string().repeat(dashes_after),
        &p.separator,
        color,
    );
    let rhs = colorize(&format!("{FRAME_H}{FRAME_TR}"), &p.separator, color);
    format!("{lhs}{head} {rhs_dashes}{rhs}")
}

fn bottom_frame(p: &ThemePalette, inner: usize, color: bool) -> String {
    let dashes = colorize(&FRAME_H.to_string().repeat(inner), &p.separator, color);
    let lhs = colorize(&FRAME_BL.to_string(), &p.separator, color);
    let rhs = colorize(&FRAME_BR.to_string(), &p.separator, color);
    format!("{lhs}{dashes}{rhs}")
}

fn blank_row(p: &ThemePalette, inner: usize, color: bool) -> String {
    let bar = colorize(&FRAME_V.to_string(), &p.separator, color);
    format!("{bar}{}{bar}", " ".repeat(inner))
}

/// `│  TAG     <body>          │` — TAG empty for continuation rows.
fn framed_tag_row(tag: &str, body: &str, p: &ThemePalette, inner: usize, color: bool) -> String {
    let bar = colorize(&FRAME_V.to_string(), &p.separator, color);
    let tag_cell = if tag.is_empty() {
        " ".repeat(TAG_COL_WIDTH)
    } else {
        let padded = format!("{tag:<width$}", tag = tag, width = TAG_WIDTH);
        let coloured = colorize(&padded, &p.secondary, color);
        format!("{}{}{}", " ".repeat(TAG_INDENT), coloured, " ".repeat(TAG_GAP))
    };
    let body_w = visible_width(body);
    // Total cells: leading "  " indent (2) + body + right pad to `inner`
    // (frame side bars consume their own cells). `inner` = width inside
    // the bars; we already accounted for left "  " in TAG_INDENT.
    let consumed = TAG_COL_WIDTH + body_w;
    let pad = inner.saturating_sub(consumed);
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
    let pct = match line3.context_used_percentage {
        Some(pct) => pct,
        None => return String::new(),
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
            // Sparkline glyph (icon-only widget — empty under Ascii).
            let pcts: Vec<u8> = window.iter().map(|(p, _)| *p).collect();
            let glyph = widgets::sparkline::render(&pcts, mode, p, color);
            if !glyph.is_empty() {
                out.push_str(ITEM_GAP);
                out.push_str(&glyph);
            }
            // Delta-time label: `30→43% in 5m` or `30→43% in 47s`. Always
            // rendered (even under Ascii) — text carries the trend even
            // when braille can't.
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
    if window.len() < 2 {
        return None;
    }
    let first = window.first()?.0;
    let last = window.last()?.0;
    let span_ms = window
        .last()?
        .1
        .saturating_sub(window.first()?.1);
    if span_ms == 0 {
        return None;
    }
    Some(format!(
        "{first}→{last}% in {}",
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

fn build_tool_rows(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    _max_width: usize,
    color: bool,
) -> Vec<String> {
    if !config.show_tools {
        return Vec::new();
    }
    let mut rows: Vec<String> = Vec::new();

    // Completed counts on the head row.
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

    // Running / recent tools — one row per tool. The first row uses TOOL
    // tag if no completed-counts row exists; otherwise continuation row.
    let recent = &frame.tools;
    for t in recent.iter().take(config.max_tool_lines.max(1)) {
        let arrow = colorize("\u{25B6}", p.tool_blue(), color);
        let name = colorize(&t.name, p.tool_blue(), color);
        let target = match &t.target {
            Some(tgt) => format!("{ITEM_GAP}{}", colorize(tgt, &p.secondary, color)),
            None => String::new(),
        };
        rows.push(format!("{arrow} {name}{target}"));
    }

    rows
}

fn build_agent_rows(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    _max_width: usize,
) -> Vec<String> {
    if frame.agents.is_empty() {
        return Vec::new();
    }
    let color = config.color_enabled;
    let max = config.max_agent_lines.max(1);
    frame
        .agents
        .iter()
        .take(max)
        .map(|a| {
            let icon = colorize("\u{f19bb}", &p.stable_blue, color);
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

