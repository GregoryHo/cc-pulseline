//! `rail` — one connected Powerline bar (height 1, stdin-only).
//!
//! A single row of segments joined by real Powerline seams. The left cluster
//! runs identity → pressure (`model · effort · cwd · git · ctx`); the right
//! cluster (`cost · version`) is pushed toward the far edge with seams
//! pointing inward at the middle gap.
//!
//! The bar is **monochrome by default** — every segment rides a 3-step gray
//! ink ramp. **Colour is reserved for the live signal**: a segment leaves the
//! ramp and takes a render-role tint only when its state crosses a threshold
//! (effort ≥ high, ctx ≥ 55%). In the default session that is exactly one
//! segment — no rainbow. Git-dirty tints only the `~N` modified count; the
//! branch stays on the ramp.
//!
//! Owns its full pipeline (dispatched from `layout::render_frame`, like
//! Ledger) — the seam rhythm doesn't compose via `apply_pane`. See
//! `designs/powerline-rail-anchor.md`.

use crate::config::RenderConfig;
use crate::render::color::{visible_width, ThemePalette};
use crate::render::frames::powerline::{self, RampLevel, Segment};
use crate::render::icons::{
    ICON_CONTEXT, ICON_EFFORT, ICON_GIT, ICON_MODEL, ICON_PROJECT, ICON_VERSION,
};
use crate::render::layout;
use crate::render::pane::LayoutStyle;
use crate::types::{Line1Metrics, RenderFrame};

/// CTX tints at/above this percentage — the first `ctx_marks()` mark.
const CTX_TINT_AT: u64 = 55;

/// Droppable cells, in the order they shed under width pressure:
/// version → cost → cwd → git → effort. Model and ctx are never dropped
/// (the hero + the live signal survive longest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Version,
    Cost,
    Cwd,
    Git,
    Effort,
}

const DROP_ORDER: [Cell; 5] = [
    Cell::Version,
    Cell::Cost,
    Cell::Cwd,
    Cell::Git,
    Cell::Effort,
];

pub fn render(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> Vec<String> {
    // Below the layout's min_width, a connected bar can't read — bypass to
    // the flat `none` identity rows (existing narrow-terminal behaviour).
    if let Some(w) = config.terminal_width {
        if w < config.pane_min_width {
            return fallback_to_none(frame, config);
        }
    }

    let tier = powerline::tier(config);
    let mode = config.glyph_mode;
    let color = config.color_enabled;

    let mut dropped: Vec<Cell> = Vec::new();
    loop {
        let (left, right) = build_clusters(frame, config, palette, &dropped);
        let row = powerline::render_bar(
            &left,
            &right,
            config.terminal_width,
            tier,
            mode,
            color,
            palette,
        );
        match config.terminal_width {
            None => return vec![row],
            Some(w) if visible_width(&row) <= w => return vec![row],
            Some(_) => match DROP_ORDER.iter().find(|c| !dropped.contains(c)) {
                Some(next) => dropped.push(*next),
                // Even model + ctx overflow → the bar can't render; fall to flat.
                None => return fallback_to_none(frame, config),
            },
        }
    }
}

/// Build the (left, right) segment clusters, honouring the per-segment `show_*`
/// toggles and the dropped-cell set. The tinting rule lives here: a cell is a
/// `Tint` only when its state crosses threshold, else a ramp step.
fn build_clusters(
    frame: &RenderFrame,
    config: &RenderConfig,
    palette: &ThemePalette,
    dropped: &[Cell],
) -> (Vec<Segment>, Vec<Segment>) {
    let l1 = &frame.line1;
    let l3 = &frame.line3;
    let mut left: Vec<Segment> = Vec::with_capacity(5);
    let mut right: Vec<Segment> = Vec::with_capacity(2);

    // model — the hero, brightest ramp step, never tints, never drops.
    if config.show_model && !l1.model.is_empty() {
        left.push(Segment::ramp(
            ICON_MODEL,
            "M:",
            l1.model.clone(),
            RampLevel::High,
        ));
    }

    // effort — tints warn→crit when the level is high or above.
    if config.show_effort && !dropped.contains(&Cell::Effort) {
        if let Some(level) = &l1.effort_level {
            if powerline::effort_tints(level) {
                let code =
                    crate::render::color::extract_ansi_code(palette.color_for_effort_level(level))
                        .unwrap_or(0);
                left.push(Segment::tint(ICON_EFFORT, "E:", level.clone(), code));
            } else {
                left.push(Segment::ramp(
                    ICON_EFFORT,
                    "E:",
                    level.clone(),
                    RampLevel::Raised,
                ));
            }
        }
    }

    // cwd — basename only (single row is width-tight); never tints.
    if config.show_project && !dropped.contains(&Cell::Cwd) && !l1.project_path.is_empty() {
        left.push(Segment::ramp(
            ICON_PROJECT,
            "P:",
            basename(&l1.project_path),
            RampLevel::Base,
        ));
    }

    // git — branch on the ramp; only the `~N` modified count tints (dirty).
    if config.show_git && !dropped.contains(&Cell::Git) && l1.has_git_branch() {
        left.push(Segment::ramp(
            ICON_GIT,
            "G:",
            git_text(l1, config.color_enabled, palette),
            RampLevel::Raised,
        ));
    }

    // ctx — the canonical live signal; tints warn≥55 / crit≥70. Never drops.
    if config.show_context {
        if let Some(pct) = l3.context_used_percentage {
            let text = format!("{pct}%");
            if pct >= CTX_TINT_AT {
                let code = crate::render::color::extract_ansi_code(palette.color_for_ctx_pct(pct))
                    .unwrap_or(0);
                left.push(Segment::tint(ICON_CONTEXT, "CTX:", text, code));
            } else {
                left.push(Segment::ramp(ICON_CONTEXT, "CTX:", text, RampLevel::Raised));
            }
        }
    }

    // cost — informational, NOT a signal: stays base gray even at high burn.
    if config.show_cost && !dropped.contains(&Cell::Cost) {
        if let Some(cost) = l3.total_cost_usd {
            right.push(Segment::ramp(
                "",
                "",
                format!("${cost:.2}"),
                RampLevel::Base,
            ));
        }
    }

    // version — far-right, first to drop.
    if config.show_version
        && !dropped.contains(&Cell::Version)
        && !l1.claude_code_version.is_empty()
    {
        right.push(Segment::ramp(
            ICON_VERSION,
            "CC:",
            format!("v{}", l1.claude_code_version),
            RampLevel::Base,
        ));
    }

    (left, right)
}

/// The git cell text: `branch +staged ~modified`. The `~N` modified count is
/// the only tinted part (alert_orange, fg-only so the segment bg persists);
/// the branch stays on the ramp. With colour off, no escapes are embedded.
fn git_text(l1: &Line1Metrics, color: bool, palette: &ThemePalette) -> String {
    let mut t = l1.git_branch.clone();
    if l1.git_added > 0 {
        t.push_str(&format!(" +{}", l1.git_added));
    }
    if l1.git_modified > 0 {
        if color {
            // fg-only orange, then restore to the ramp's secondary fg. No
            // RESET (`\x1b[0m`) — that would clear the segment's bg fill.
            t.push_str(&format!(
                " {}~{}{}",
                palette.alert_orange, l1.git_modified, palette.secondary
            ));
        } else {
            t.push_str(&format!(" ~{}", l1.git_modified));
        }
    }
    t
}

/// Last path component (the single row favours basename over full path).
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
/// overflows. `render_frame` already subtracted `pane_cc_margin`; restore it
/// so the re-entrant call doesn't subtract twice (same pattern as Ledger /
/// Badge fallbacks).
fn fallback_to_none(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let mut shrunk = config.clone();
    shrunk.pane_style = LayoutStyle::None;
    if let Some(w) = shrunk.terminal_width {
        shrunk.terminal_width = Some(w + shrunk.pane_cc_margin);
    }
    layout::render_frame(frame, &shrunk)
}
