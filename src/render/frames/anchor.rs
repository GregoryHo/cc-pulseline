//! `anchor` v2 — three grouped capsule+trail rows (identity · context · quota).
//!
//! Each row leads with a banded **capsule hero** (`render_capsule`,
//! `CapStyle::Round` — the canonical capsule silhouette; `rail` keeps angled
//! seams, the contrast is intentional) and trails as dim text where state
//! flags light in their role colour. Two channels, no competition: shape (the
//! capsule) = the row's subject; colour = state. The capsule is always filled;
//! only trail flags light up.
//!
//! Trail separator: ` ❯ ` (`PL_TICK`) in the Powerline tier; ` · ` (structural)
//! in Blocks / AsciiFloor. Row 2 carries an inline gauge — the shipped
//! `widgets::gauge` (one gauge dialect across the product).
//!
//! Height ladder (reuses `max_total_lines`): 3 rows → 2 (drop quota) → the v1
//! single capsule+trail bar. Quota drops when `!quota.has_data()`. See
//! `designs/rail-anchor-grouped-rows.md`.

use crate::config::RenderConfig;
use crate::render::color::{colorize, extract_ansi_code, visible_width, ThemePalette};
use crate::render::fmt::{format_number, format_reset_duration};
use crate::render::frames::powerline::{self, CapStyle, SeamTier};
use crate::render::icons::{
    fail_mark, glyph, ICON_CONTEXT, ICON_EFFORT, ICON_GIT, ICON_MODEL, ICON_PROJECT, ICON_QUOTA,
    ICON_TODO, ICON_TOOL, PL_TICK,
};
use crate::render::layout;
use crate::render::pane::LayoutStyle;
use crate::render::widgets::gauge;
use crate::types::{Line1Metrics, RenderFrame};

// The capsule hero is always filled with its band colour (quota included), so
// anchor has no left-flag threshold for quota — only ctx in the fused rung.
const CTX_TINT_AT: u64 = 55;
const GAUGE_WIDTH: usize = 14;

fn code(escape: &str) -> u8 {
    extract_ansi_code(escape).unwrap_or(0)
}

pub fn render(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> Vec<String> {
    if let Some(w) = config.terminal_width {
        if w < config.pane_min_width {
            return fallback_to_none(frame, config);
        }
    }

    let budget = config.max_total_lines.unwrap_or(3).max(1);
    if budget == 1 {
        return vec![build_fused_row(frame, config, palette)];
    }

    let mut rows: Vec<String> = vec![
        row_identity(frame, config, palette),
        row_context(frame, config, palette),
    ];
    if budget >= 3 && frame.quota.has_data() {
        rows.push(row_quota(frame, config, palette));
    }
    // Drop blank rows (e.g. context before the first API call) — never emit an
    // empty line; the row simply isn't there, like the lazy quota row.
    rows.retain(|r| visible_width(r) > 0);
    rows
}

// ── Trail plumbing ──────────────────────────────────────────────────────────

/// The trail separator for the active tier: powerline thin tick, else a
/// structural middot. Coloured structural either way.
fn trail_sep(tier: SeamTier, palette: &ThemePalette, color: bool) -> String {
    let glyph = if tier == SeamTier::Powerline {
        format!(" {PL_TICK} ")
    } else {
        " · ".to_string()
    };
    colorize(&glyph, &palette.structural, color)
}

/// A dim (or flag-lit) trail text cell.
fn cell(
    icon: &'static str,
    text: &str,
    lit: Option<&str>,
    config: &RenderConfig,
    palette: &ThemePalette,
) -> String {
    let body = format!("{}{}", glyph(config.glyph_mode, icon, ""), text);
    let fg = lit.unwrap_or(&palette.structural);
    colorize(&body, fg, config.color_enabled)
}

/// Assemble a row (capsule + dim trail), dropping trailing trail cells under
/// width pressure until it fits the terminal. The capsule (the hero) always
/// survives; the trail sheds from the end.
fn assemble(
    capsule: &str,
    mut trail: Vec<String>,
    config: &RenderConfig,
    palette: &ThemePalette,
) -> String {
    let tier = powerline::tier(config);
    let color = config.color_enabled;
    loop {
        let mut out = String::from(capsule);
        if !trail.is_empty() {
            if !out.is_empty() {
                out.push_str("  ");
            }
            out.push_str(&trail.join(&trail_sep(tier, palette, color)));
        }
        match config.terminal_width {
            None => return out,
            Some(w) if visible_width(&out) <= w => return out,
            // Too wide: shed the lowest-priority (trailing) trail cell; if the
            // trail is exhausted, render the capsule as-is.
            Some(_) => {
                if trail.pop().is_none() {
                    return out;
                }
            }
        }
    }
}

fn capsule(icon: &'static str, text: &str, bg: u8, config: &RenderConfig) -> String {
    powerline::render_capsule(
        icon,
        text,
        bg,
        powerline::tier(config),
        CapStyle::Round,
        config.glyph_mode,
        config.color_enabled,
    )
}

// ── Row 1 · identity ──────────────────────────────────────────────────────
fn row_identity(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> String {
    let l1 = &frame.line1;
    let hero = if !l1.model.is_empty() {
        capsule(ICON_MODEL, &l1.model, code(&palette.stable_blue), config)
    } else {
        String::new()
    };

    let mut trail: Vec<String> = Vec::new();
    if let Some(level) = &l1.effort_level {
        let lit = powerline::effort_tints(level).then(|| palette.color_for_effort_level(level));
        trail.push(cell(ICON_EFFORT, level, lit, config, palette));
    }
    if !l1.claude_code_version.is_empty() {
        trail.push(cell(
            "",
            &format!("v{}", l1.claude_code_version),
            None,
            config,
            palette,
        ));
    }
    if !l1.project_path.is_empty() {
        trail.push(cell(
            ICON_PROJECT,
            &basename(&l1.project_path),
            None,
            config,
            palette,
        ));
    }
    if l1.has_git_branch() {
        let lit = l1.git_dirty.then_some(palette.alert_orange.as_str());
        trail.push(cell(ICON_GIT, &git_text(l1), lit, config, palette));
    }
    assemble(&hero, trail, config, palette)
}

// ── Row 2 · context ─────────────────────────────────────────────────────────
fn row_context(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> String {
    let l3 = &frame.line3;
    let Some(pct) = l3.context_used_percentage else {
        return String::new();
    };
    let hero = capsule(
        ICON_CONTEXT,
        &format!("CTX {pct}%"),
        code(palette.color_for_ctx_pct(pct)),
        config,
    );

    let mut trail: Vec<String> = Vec::new();
    // Inline gauge — the one shipped gauge dialect (ctx marks, ctx fill).
    let g = gauge::render(
        pct,
        GAUGE_WIDTH,
        &ThemePalette::ctx_marks(),
        palette.color_for_ctx_pct(pct),
        palette,
        config.glyph_mode,
        config.color_enabled,
    );
    if !g.is_empty() {
        trail.push(g);
    }
    if let Some(size) = l3.context_window_size {
        let used = size.saturating_mul(pct) / 100;
        trail.push(cell(
            "",
            &format!("{}/{}", format_number(used), format_number(size)),
            None,
            config,
            palette,
        ));
    }
    if let Some(i) = l3.input_tokens {
        trail.push(cell(
            "",
            &format!("in {}", format_number(i)),
            None,
            config,
            palette,
        ));
    }
    if let Some(o) = l3.output_tokens {
        trail.push(cell(
            "",
            &format!("out {}", format_number(o)),
            None,
            config,
            palette,
        ));
    }
    push_traceability(&mut trail, frame, config, palette);
    assemble(&hero, trail, config, palette)
}

/// Append the traceability trail cells (task progress + tool-use volume) at the
/// trail tail, so they shed FIRST under width pressure (volatile activity, like
/// rail's drop-tier-1). Gated on the same `show_todo`/`show_tools` toggles as
/// the flat activity rows; colour stays structural except `✘M` failures light
/// `alert_red` (anchor's shape=subject / colour=state grammar). Honest counts:
/// `completed_tool_total` / `failed_tool_total` are the uncapped frame totals.
///
/// Note: anchor rides traceability on the CONTEXT row, so it shares that row's
/// lifecycle — absent until the first API call lands a context%. (rail's
/// usage-row cells render independently of ctx%.) The asymmetry is intentional:
/// anchor has no always-present detail row to host them, and a completed tool
/// implies an API turn has already occurred, so the gap is a narrow transient.
fn push_traceability(
    trail: &mut Vec<String>,
    frame: &RenderFrame,
    config: &RenderConfig,
    palette: &ThemePalette,
) {
    if config.show_todo {
        if let Some(t) = frame.todo.as_ref().filter(|t| t.total > 0) {
            trail.push(cell(
                ICON_TODO,
                &format!("{}/{}", t.completed, t.total),
                None,
                config,
                palette,
            ));
        }
    }
    if config.show_tools && frame.completed_tool_total > 0 {
        let failed = frame.failed_tool_total;
        let text = if failed > 0 {
            format!(
                "{} {}{}",
                frame.completed_tool_total,
                fail_mark(config.glyph_mode),
                failed
            )
        } else {
            frame.completed_tool_total.to_string()
        };
        let lit = (failed > 0).then_some(palette.alert_red.as_str());
        trail.push(cell(ICON_TOOL, &text, lit, config, palette));
    }
}

// ── Row 3 · quota ───────────────────────────────────────────────────────────
fn row_quota(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> String {
    let q = &frame.quota;
    let tier = powerline::tier(config);
    let color = config.color_enabled;
    let mut out = String::new();

    if let Some(pct) = q.five_hour_pct {
        let pct_u = pct.round() as u64;
        out.push_str(&capsule(
            ICON_QUOTA,
            &format!("5H {pct_u}%"),
            code(palette.color_for_quota_pct(pct)),
            config,
        ));
        if let Some(m) = q.five_hour_reset_minutes {
            out.push_str("  ");
            out.push_str(&cell(
                "",
                &format!("resets {}", format_reset_duration(m)),
                None,
                config,
                palette,
            ));
        }
    }
    if let Some(pct) = q.seven_day_pct {
        let pct_u = pct.round() as u64;
        if !out.is_empty() {
            out.push_str(&trail_sep(tier, palette, color));
        }
        out.push_str(&capsule(
            ICON_QUOTA,
            &format!("7D {pct_u}%"),
            code(palette.color_for_quota_pct(pct)),
            config,
        ));
        if let Some(m) = q.seven_day_reset_minutes {
            out.push_str("  ");
            out.push_str(&cell(
                "",
                &format!("resets {}", format_reset_duration(m)),
                None,
                config,
                palette,
            ));
        }
    }
    out
}

// ── Bottom rung: v1 single capsule + trail bar ──────────────────────────────
fn build_fused_row(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> String {
    let l1 = &frame.line1;
    let l3 = &frame.line3;
    let hero = if !l1.model.is_empty() {
        capsule(ICON_MODEL, &l1.model, code(&palette.stable_blue), config)
    } else {
        String::new()
    };

    // Traceability (todo/tools) is intentionally omitted from the most-compressed
    // fused rung — volatile activity sheds first (mirrors rail's build_fused_cells).
    let mut trail: Vec<String> = Vec::new();
    if let Some(level) = &l1.effort_level {
        let lit = powerline::effort_tints(level).then(|| palette.color_for_effort_level(level));
        trail.push(cell(ICON_EFFORT, level, lit, config, palette));
    }
    if !l1.project_path.is_empty() {
        trail.push(cell(
            ICON_PROJECT,
            &basename(&l1.project_path),
            None,
            config,
            palette,
        ));
    }
    if l1.has_git_branch() {
        let lit = l1.git_dirty.then_some(palette.alert_orange.as_str());
        trail.push(cell(ICON_GIT, &git_text(l1), lit, config, palette));
    }
    if let Some(pct) = l3.context_used_percentage {
        let lit = (pct >= CTX_TINT_AT).then(|| palette.color_for_ctx_pct(pct));
        trail.push(cell(ICON_CONTEXT, &format!("{pct}%"), lit, config, palette));
    }
    if let Some(cost) = l3.total_cost_usd {
        trail.push(cell("", &format!("${cost:.2}"), None, config, palette));
    }
    assemble(&hero, trail, config, palette)
}

// ── shared helpers ──────────────────────────────────────────────────────────
fn git_text(l1: &Line1Metrics) -> String {
    let mut t = l1.git_branch.clone();
    if l1.git_added > 0 {
        t.push_str(&format!(" +{}", l1.git_added));
    }
    if l1.git_modified > 0 {
        t.push_str(&format!(" ~{}", l1.git_modified));
    }
    if l1.git_dirty {
        t.push_str(" *");
    }
    t
}

fn basename(path: &str) -> String {
    let base = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path);
    if base.is_empty() {
        path.to_string()
    } else {
        base.to_string()
    }
}

fn fallback_to_none(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let mut shrunk = config.clone();
    shrunk.pane_style = LayoutStyle::None;
    if let Some(w) = shrunk.terminal_width {
        shrunk.terminal_width = Some(w + shrunk.pane_cc_margin);
    }
    layout::render_frame(frame, &shrunk)
}
