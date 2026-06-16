//! `anchor` — hero capsule + dim trail (height 1, stdin-only).
//!
//! One reverse-video **capsule** (angled Powerline caps) anchors the line by
//! silhouette; the remaining fields trail as dim text. Two orthogonal
//! channels: **shape = identity, colour = state.** They don't compete, so the
//! capsule is a stable identity colour *and* the trail can still flash one
//! signal.
//!
//! - **Hero** = `model.display_name` (capsule body = the model role colour,
//!   text = reverse-video).
//! - **Trail** = `effort · cwd · git · ctx · cost · version`, joined by ` · `
//!   in the structural tier. Every item is dim **except** the one whose state
//!   crosses threshold (same tinting rule as `rail`), which renders in its
//!   full render-role colour.
//!
//! Owns its full pipeline (dispatched from `layout::render_frame`). Single row,
//! so no height ladder; under width pressure trail cells drop in priority order
//! (version → cost → cwd → git → effort — the capsule + ctx survive longest),
//! then it bypasses to flat `none` below `min_width`. See
//! `designs/powerline-rail-anchor.md`.

use crate::config::RenderConfig;
use crate::render::color::{colorize, extract_ansi_code, visible_width, ThemePalette};
use crate::render::frames::powerline::{self, CapStyle};
use crate::render::icons::{
    glyph, ICON_CONTEXT, ICON_EFFORT, ICON_GIT, ICON_MODEL, ICON_PROJECT, ICON_VERSION,
};
use crate::render::layout;
use crate::render::pane::LayoutStyle;
use crate::types::{Line1Metrics, RenderFrame};

/// CTX tints at/above this percentage (first `ctx_marks()` mark).
const CTX_TINT_AT: u64 = 55;

/// Droppable trail cells, in the order they shed under width pressure. The
/// capsule (model) and ctx (the live signal) are never dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrailCell {
    Version,
    Cost,
    Cwd,
    Git,
    Effort,
}

const TRAIL_DROP_ORDER: [TrailCell; 5] = [
    TrailCell::Version,
    TrailCell::Cost,
    TrailCell::Cwd,
    TrailCell::Git,
    TrailCell::Effort,
];

pub fn render(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> Vec<String> {
    // Below min_width a single dense line can't read — bypass to flat `none`.
    if let Some(w) = config.terminal_width {
        if w < config.pane_min_width {
            return fallback_to_none(frame, config);
        }
    }

    let l1 = &frame.line1;
    let tier = powerline::tier(config);
    let mode = config.glyph_mode;
    let color = config.color_enabled;

    // Capsule = model. Built once — it never drops (it's the hero silhouette).
    let capsule = if config.show_model && !l1.model.is_empty() {
        let bg = extract_ansi_code(&palette.stable_blue).unwrap_or(111);
        powerline::render_capsule(
            ICON_MODEL,
            &l1.model,
            bg,
            tier,
            CapStyle::Angle,
            mode,
            color,
        )
    } else {
        String::new()
    };

    let mut dropped: Vec<TrailCell> = Vec::new();
    loop {
        let trail = build_trail(frame, config, palette, &dropped);
        let row = assemble(&capsule, &trail, palette, color);
        match config.terminal_width {
            None => return vec![row],
            Some(w) if visible_width(&row) <= w => return vec![row],
            Some(_) => match TRAIL_DROP_ORDER.iter().find(|c| !dropped.contains(c)) {
                Some(next) => dropped.push(*next),
                // Capsule + ctx still overflow → bypass to flat.
                None => return fallback_to_none(frame, config),
            },
        }
    }
}

/// Join the capsule and the dim trail into one row (trail cells separated by
/// a structural ` · `).
fn assemble(capsule: &str, trail: &[String], palette: &ThemePalette, color: bool) -> String {
    let mut out = String::from(capsule);
    if !trail.is_empty() {
        let sep = colorize(" · ", &palette.structural, color);
        if !out.is_empty() {
            out.push_str("  ");
        }
        out.push_str(&trail.join(&sep));
    }
    out
}

/// Each trail cell, pre-coloured. Dim (`structural`) by default; the single
/// state-crossing cell renders in its full render-role colour. Droppable cells
/// honour the `dropped` set; ctx is never dropped.
fn build_trail(
    frame: &RenderFrame,
    config: &RenderConfig,
    palette: &ThemePalette,
    dropped: &[TrailCell],
) -> Vec<String> {
    let l1 = &frame.line1;
    let l3 = &frame.line3;
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let dim = &palette.structural;
    let mut cells: Vec<String> = Vec::new();

    // effort — lights up from high upward.
    if config.show_effort && !dropped.contains(&TrailCell::Effort) {
        if let Some(level) = &l1.effort_level {
            let body = format!("{}{}", glyph(mode, ICON_EFFORT, "E:"), level);
            let c = if powerline::effort_tints(level) {
                palette.color_for_effort_level(level)
            } else {
                dim
            };
            cells.push(colorize(&body, c, color));
        }
    }

    // cwd — basename, always dim.
    if config.show_project && !dropped.contains(&TrailCell::Cwd) && !l1.project_path.is_empty() {
        let body = format!(
            "{}{}",
            glyph(mode, ICON_PROJECT, "P:"),
            basename(&l1.project_path)
        );
        cells.push(colorize(&body, dim, color));
    }

    // git — branch dim; `~N` modified count lights orange (dirty).
    if config.show_git && !dropped.contains(&TrailCell::Git) && l1.has_git_branch() {
        cells.push(git_cell(l1, mode, color, palette));
    }

    // ctx — the canonical live signal; lights warn≥55 / crit≥70. Never drops.
    if config.show_context {
        if let Some(pct) = l3.context_used_percentage {
            let body = format!("{}{pct}%", glyph(mode, ICON_CONTEXT, "CTX:"));
            let c = if pct >= CTX_TINT_AT {
                palette.color_for_ctx_pct(pct)
            } else {
                dim
            };
            cells.push(colorize(&body, c, color));
        }
    }

    // cost — informational, always dim (not a signal).
    if config.show_cost && !dropped.contains(&TrailCell::Cost) {
        if let Some(cost) = l3.total_cost_usd {
            cells.push(colorize(&format!("${cost:.2}"), dim, color));
        }
    }

    // version — always dim, first to drop.
    if config.show_version
        && !dropped.contains(&TrailCell::Version)
        && !l1.claude_code_version.is_empty()
    {
        let body = format!(
            "{}v{}",
            glyph(mode, ICON_VERSION, "CC:"),
            l1.claude_code_version
        );
        cells.push(colorize(&body, dim, color));
    }

    cells
}

/// git trail cell: dim branch + staged, with the `~N` modified count in
/// alert_orange. Built as one coloured string so the ` · ` join stays clean.
fn git_cell(
    l1: &Line1Metrics,
    mode: crate::config::GlyphMode,
    color: bool,
    palette: &ThemePalette,
) -> String {
    let mut body = format!("{}{}", glyph(mode, ICON_GIT, "G:"), l1.git_branch);
    if l1.git_added > 0 {
        body.push_str(&format!(" +{}", l1.git_added));
    }
    let dim_str = colorize(&body, &palette.structural, color);
    if l1.git_modified == 0 {
        return dim_str;
    }
    let tail = colorize(
        &format!(" ~{}", l1.git_modified),
        &palette.alert_orange,
        color,
    );
    format!("{dim_str}{tail}")
}

/// Last path component (single row favours basename).
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

/// Narrow-terminal escape hatch: flat `none` is content-sized and never
/// overflows. `render_frame` already subtracted `pane_cc_margin`; restore it so
/// the re-entrant call doesn't subtract twice (same pattern as rail / ledger).
fn fallback_to_none(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let mut shrunk = config.clone();
    shrunk.pane_style = LayoutStyle::None;
    if let Some(w) = shrunk.terminal_width {
        shrunk.terminal_width = Some(w + shrunk.pane_cc_margin);
    }
    layout::render_frame(frame, &shrunk)
}
