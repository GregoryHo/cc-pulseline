//! Console — v2 framed dashboard layout (4-5 rows, highest "quality feel").
//!
//! Output rows (full width):
//!   ╭─ <identity> ───────────────╮
//!   │  CTX  <gauge>  <pct/total>     <sparkline>          │
//!   │  TOK  <rate>   COST <total> <arc>   Q5h <gauge>     │
//!   │  ──────────────────────────────────                 │
//!   │  <tools tape>                  <completed chips>    │
//!   │  <agent rows>                  <todo chip>          │
//!   ╰────────────────────────────────────────────────────╯
//!
//! Width handling:
//!   ≥ 130   full framed
//!   110-129 inner rule dropped, 4 rows
//!   90-109  fall back to Cockpit (caller — `auto::resolve` decides)
//!   < 90    fall back to Flightstrip
//!
//! Quota gets a real gauge bar here — the frame gives it room.

use crate::config::RenderConfig;
use crate::render::color::{colorize, visible_width, ThemePalette};
use crate::render::icons::{FRAME_BL, FRAME_BR, FRAME_H, FRAME_TL, FRAME_TR, FRAME_V};

use super::{cockpit, flightstrip, shared};

const GAUGE_WIDTH: usize = 22;
const FRAME_INNER_PAD: usize = 4; // "│  " left + "│" right + 1 trailing space
/// Visible chars `framed()` consumes inside `inner` for its leading "  "
/// indent before the content starts. Subtract from `inner` to get the
/// width budget any row's content can actually use.
const FRAMED_CONTENT_INSET: usize = 2;

pub fn render(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Vec<String> {
    // Unknown width → assume 140 (console's intended bracket). Clamp to
    // `pane_max_width` (default 140, configurable via `[layout].max_width`)
    // so the framed dashboard never grows past the user's chosen ceiling
    // even when the terminal is much wider — matches `apply_pane`'s
    // behaviour for flat-row layouts.
    let width = config
        .terminal_width
        .unwrap_or(140)
        .min(config.pane_max_width);

    // Below 110 cols the framed dashboard becomes claustrophobic; defer to
    // smaller siblings rather than mangle our own borders.
    if width < 90 {
        return flightstrip::render(frame, config, p);
    }
    if width < 110 {
        return cockpit::render(frame, config, p);
    }

    let inner = width.saturating_sub(FRAME_INNER_PAD);
    let drop_inner_rule = width < 130;

    let color = config.color_enabled;
    let mut lines: Vec<String> = Vec::with_capacity(7);

    lines.push(top_frame(frame, config, p, inner));

    if shared::config_row_enabled(config) {
        let row = shared::config_row(frame, config, p, inner);
        if !row.is_empty() {
            lines.push(framed(&row, p, inner, color));
        }
    }

    lines.push(framed(&ctx_row(frame, config, p), p, inner, color));
    lines.push(framed(
        &tok_cost_quota_row(frame, config, p),
        p,
        inner,
        color,
    ));

    if !drop_inner_rule {
        lines.push(framed(&inner_rule(p, inner, color), p, inner, color));
    }

    for row in tools_rows(frame, config, p, inner) {
        lines.push(framed(&row, p, inner, color));
    }
    for row in agent_todo_rows(frame, config, p, inner) {
        lines.push(framed(&row, p, inner, color));
    }

    lines.push(bottom_frame(p, inner, color));
    lines
}

fn top_frame(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    inner: usize,
) -> String {
    let color = config.color_enabled;
    let head = shared::identity_headline(&frame.line1, config, p);
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

fn framed(content: &str, p: &ThemePalette, inner: usize, color: bool) -> String {
    let content_w = visible_width(content);
    let pad = inner.saturating_sub(content_w + FRAMED_CONTENT_INSET);
    let bar = colorize(&FRAME_V.to_string(), &p.separator, color);
    format!("{bar}  {content}{}{bar}", " ".repeat(pad))
}

fn inner_rule(p: &ThemePalette, inner: usize, color: bool) -> String {
    colorize(
        &FRAME_H.to_string().repeat(inner.saturating_sub(2)),
        &p.separator,
        color,
    )
}

fn ctx_row(frame: &crate::types::RenderFrame, config: &RenderConfig, p: &ThemePalette) -> String {
    // Console's CTX row honours `context_visual`. Pass GAUGE_WIDTH as the
    // sizing hint — render_context_visual passes this through to the gauge
    // widget so console keeps its wider bar regardless of the spec. The
    // gauge cell now embeds `<used>/<total>` itself, so no further
    // annotation is needed here.
    shared::render_context_visual(
        config.effective_context_visual(),
        &frame.line3,
        &frame.ctx_history,
        GAUGE_WIDTH,
        config.glyph_mode,
        p,
        config.color_enabled,
    )
}

fn tok_cost_quota_row(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> String {
    let color = config.color_enabled;
    let mut parts: Vec<String> = Vec::new();
    if config.show_tokens {
        parts.push(shared::token_rate_cell(
            &frame.line3,
            frame.line3.output_speed_toks_per_sec,
            p,
            color,
        ));
    }
    if config.show_cost {
        let cost_lbl = colorize("COST  ", &p.structural, color);
        let cost_body = shared::render_cost_visual(
            config.effective_cost_visual(),
            &frame.line3,
            config.glyph_mode,
            p,
            color,
        );
        if !cost_body.is_empty() {
            parts.push(format!("{cost_lbl}{cost_body}"));
        }
    }
    if config.show_quota {
        let quota_spec = config.effective_quota_visual();
        if config.show_quota_five_hour {
            let q = shared::render_quota_visual(
                quota_spec,
                "5h  ",
                frame.quota.five_hour_pct,
                frame.quota.five_hour_reset_minutes,
                config.glyph_mode,
                p,
                color,
            );
            if !q.is_empty() {
                parts.push(q);
            }
        }
        if config.show_quota_seven_day {
            let q = shared::render_quota_visual(
                quota_spec,
                "7d  ",
                frame.quota.seven_day_pct,
                frame.quota.seven_day_reset_minutes,
                config.glyph_mode,
                p,
                color,
            );
            if !q.is_empty() {
                parts.push(q);
            }
        }
    }
    parts.join("    ")
}

/// Tools segment as zero or more framed rows.
///
/// Layout rule (per user preference):
/// - When the running tape + the completed-count chips fit together on a
///   single row, render them side-by-side (`tape    counts`).
/// - When the running tape alone is too long for the budget, push the
///   completed counts to a row of their own. Counts then render on a new
///   line below the running tape rather than overflowing the pane.
///
/// Returns rows in display order (top→bottom). Caller frames each row.
fn tools_rows(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    inner: usize,
) -> Vec<String> {
    let color = config.color_enabled;
    if !config.show_tools {
        return Vec::new();
    }
    let budget = inner.saturating_sub(FRAMED_CONTENT_INSET);

    let tape = if frame.tools.is_empty() {
        String::new()
    } else {
        shared::render_tools_visual_inline(
            config.effective_tools_visual(),
            &frame.tools,
            config.max_tool_lines.max(2),
            budget,
            config.glyph_mode,
            p,
            color,
        )
    };
    let counts = if frame.completed_tools.is_empty() {
        String::new()
    } else {
        shared::completed_tool_chips(&frame.completed_tools, 4, p, color)
    };

    let mut rows: Vec<String> = Vec::with_capacity(2);
    if tape.is_empty() && counts.is_empty() {
        return rows;
    }
    if tape.is_empty() {
        rows.push(counts);
        return rows;
    }
    if counts.is_empty() {
        rows.push(tape);
        return rows;
    }
    // Both present — try side-by-side first.
    const INLINE_GAP: usize = 4;
    let combined_w = visible_width(&tape) + INLINE_GAP + visible_width(&counts);
    if combined_w <= budget {
        rows.push(format!("{tape}    {counts}"));
    } else {
        rows.push(tape);
        rows.push(counts);
    }
    rows
}

/// Agents + todo segment as zero or more framed rows.
///
/// Agent cells wrap to additional rows when they don't fit on one (via
/// `pack_agent_cells`'s multi-row packer, capped by `max_agent_lines`).
/// The todo chip — when present — sits inline at the end of the *last*
/// agent row if it fits, else gets its own row.
fn agent_todo_rows(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    inner: usize,
) -> Vec<String> {
    let color = config.color_enabled;
    let budget = inner.saturating_sub(FRAMED_CONTENT_INSET);

    let mut rows: Vec<String> = if config.show_agents && !frame.agents.is_empty() {
        shared::pack_agent_cells(&frame.agents, config, p, budget)
    } else {
        Vec::new()
    };

    if config.show_todo {
        if let Some(todo) = &frame.todo {
            let bullet = colorize("\u{2022}", p.todo_teal(), color);
            let txt = colorize(
                &format!(" TODO {}/{}", todo.completed, todo.total),
                p.todo_teal(),
                color,
            );
            let chip = format!("{bullet}{txt}");
            const INLINE_GAP: usize = 4;
            // Try to attach to the last agent row; fall back to its own row.
            if let Some(last) = rows.last_mut() {
                if visible_width(last) + INLINE_GAP + visible_width(&chip) <= budget {
                    last.push_str("    ");
                    last.push_str(&chip);
                } else {
                    rows.push(chip);
                }
            } else {
                rows.push(chip);
            }
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RenderConfig;
    use crate::render::color::resolve_palette;
    use crate::types::RenderFrame;

    fn frame_basic() -> RenderFrame {
        let mut f = RenderFrame::default();
        f.line1.model = "Opus 4.7".to_string();
        f.line1.git_branch = "feat/status-pane".to_string();
        f.line1.project_path = "~/cc-pulseline".to_string();
        f.line3.context_window_size = Some(200_000);
        f.line3.context_used_percentage = Some(43);
        f.line3.total_cost_usd = Some(3.50);
        f.line3.total_duration_ms = Some(60_000 * 30);
        f.ctx_history = vec![20, 30, 40, 43];
        f
    }

    fn cfg(width: usize) -> RenderConfig {
        RenderConfig {
            color_enabled: false,
            terminal_width: Some(width),
            palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
            ..RenderConfig::default()
        }
    }

    #[test]
    fn console_emits_framed_top_and_bottom_at_full_width() {
        let f = frame_basic();
        let c = cfg(140);
        let lines = render(&f, &c, &c.palette.clone());
        assert!(lines.first().unwrap().starts_with(FRAME_TL));
        assert!(lines.last().unwrap().starts_with(FRAME_BL));
        // CTX row is identified by `used/total` numbers (label dropped).
        // 200k window × 43% = 86k → `86.0k/200.0k`.
        assert!(lines.iter().any(|l| l.contains("/200.0k")));
        assert!(lines.iter().any(|l| l.contains("$3.50")));
    }

    #[test]
    fn console_falls_back_to_cockpit_below_110_cols() {
        let f = frame_basic();
        let c = cfg(100);
        let lines = render(&f, &c, &c.palette.clone());
        // No frame characters
        assert!(!lines.first().unwrap().starts_with(FRAME_TL));
    }

    #[test]
    fn console_falls_back_to_flightstrip_below_90_cols() {
        let f = frame_basic();
        let c = cfg(80);
        let lines = render(&f, &c, &c.palette.clone());
        assert!(!lines.first().unwrap().starts_with(FRAME_TL));
        // Flightstrip at <90 cols drops cost from L1
        assert!(!lines[0].contains("$3.50"));
    }
}
