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

pub fn render(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Vec<String> {
    // Unknown width → assume 140 (console's intended bracket).
    let width = config.terminal_width.unwrap_or(140);

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

    let tools_row = tools_row(frame, config, p);
    if !tools_row.is_empty() {
        lines.push(framed(&tools_row, p, inner, color));
    }
    let agent_todo = agent_todo_row(frame, config, p);
    if !agent_todo.is_empty() {
        lines.push(framed(&agent_todo, p, inner, color));
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
    // 2 = leading "  " visual indent inside the frame walls.
    let pad = inner.saturating_sub(content_w + 2);
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
    let color = config.color_enabled;
    // Console's CTX row honours `context_visual`. Pass GAUGE_WIDTH as the
    // sizing hint — render_context_visual passes this through to the gauge
    // widget so console keeps its wider bar regardless of the spec.
    let cell = shared::render_context_visual(
        config.effective_context_visual(),
        &frame.line3,
        &frame.ctx_history,
        GAUGE_WIDTH,
        config.glyph_mode,
        p,
        color,
    );
    // Console-specific annotation: append `/ <total>` after the cell when
    // a context window size is known. Reads as supplementary info to the
    // pct, distinct from text widget's `(used/total)` form.
    let total_str = frame
        .line3
        .context_window_size
        .map(|s| {
            colorize(
                &format!(" / {}", crate::render::fmt::format_number(s)),
                &p.secondary,
                color,
            )
        })
        .unwrap_or_default();
    if total_str.is_empty() {
        cell
    } else {
        // Insert the `/ total` annotation right after the pct text. The
        // gauge/text widgets end at `<pct>%`; sparkline (if present) sits
        // beyond. Splitting on the last `%` keeps the rendered output
        // compatible with each visual spec.
        match cell.rfind('%') {
            Some(idx) => {
                let mut buf = String::with_capacity(cell.len() + total_str.len());
                buf.push_str(&cell[..=idx]);
                buf.push_str(&total_str);
                buf.push_str(&cell[idx + 1..]);
                buf
            }
            None => format!("{cell}{total_str}"),
        }
    }
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

fn tools_row(frame: &crate::types::RenderFrame, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;
    if !config.show_tools {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    if !frame.tools.is_empty() {
        let tape = shared::render_tools_visual_inline(
            config.effective_tools_visual(),
            &frame.tools,
            config.max_tool_lines.max(2),
            config.glyph_mode,
            p,
            color,
        );
        if !tape.is_empty() {
            parts.push(tape);
        }
    }
    if !frame.completed_tools.is_empty() {
        parts.push(shared::completed_tool_chips(
            &frame.completed_tools,
            4,
            p,
            color,
        ));
    }
    parts.join("    ")
}

fn agent_todo_row(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> String {
    let color = config.color_enabled;
    let mut parts: Vec<String> = Vec::new();

    if config.show_agents {
        let agents: Vec<String> = frame
            .agents
            .iter()
            .take(config.max_agent_lines.max(1))
            .map(|a| {
                let prefix = shared::agent_prefix(config, p);
                let name = match &a.agent_type {
                    Some(t) => t.clone(),
                    None => a
                        .description
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(28)
                        .collect::<String>(),
                };
                let model_part = a
                    .model
                    .as_ref()
                    .map(|m| colorize(&format!(" [{m}]"), &p.structural, color))
                    .unwrap_or_default();
                format!(
                    "{prefix}{}{model_part}",
                    colorize(&name, p.agent_purple(), color)
                )
            })
            .collect();
        if !agents.is_empty() {
            parts.push(agents.join("  "));
        }
    }

    if config.show_todo {
        if let Some(todo) = &frame.todo {
            let bullet = colorize("\u{2022}", p.todo_teal(), color);
            let txt = colorize(
                &format!(" TODO {}/{}", todo.completed, todo.total),
                p.todo_teal(),
                color,
            );
            parts.push(format!("{bullet}{txt}"));
        }
    }

    parts.join("    ")
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
        assert!(lines.iter().any(|l| l.contains("CTX")));
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
