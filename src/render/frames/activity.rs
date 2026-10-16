//! `activity` — the band's layout: activity rows and nothing else.
//!
//! The band above the prompt is a second canvas beside the statusline, not a
//! copy of it. The statusline owns the steady state — identity, context,
//! tokens, cost, quota — because only the transcript path can sample tokens
//! often enough to compute a rate, and that row is always on screen. This
//! canvas owns the turn in flight: the tools running and what they are
//! pointed at, the agents and how long they have been going, the todo, and
//! the per-tool completed counts.
//!
//! Two things make it a layout of its own rather than a configuration of
//! `none`:
//!
//! 1. **It draws no core rows.** Every other layout leads with identity and
//!    budget, and the height ladder's floor (`FuseCore`) keeps one fused core
//!    row no matter how hard it is squeezed — so "zero core rows" is not
//!    expressible through config.
//! 2. **Its row order is inverted.** `build_activity_rows_running_first`
//!    leads with the running tools, so the bottom-truncation below can never
//!    reach the one row the band was added for. The generic ladder does the
//!    opposite: its first rung is `DropRunningTools`.
//!
//! Idle output is empty. The caller decides what an empty band means — the
//! mod yields the band to the engine rather than drawing a blank row.

use crate::config::RenderConfig;
use crate::render::color::ThemePalette;
use crate::types::RenderFrame;

/// Render the activity rows for `frame`, honouring `max_total_lines`.
pub fn render(frame: &RenderFrame, config: &RenderConfig, palette: &ThemePalette) -> Vec<String> {
    let width = config.terminal_width.unwrap_or(usize::MAX);
    let mut rows = build(frame, config, palette, width);

    let Some(cap) = config.max_total_lines.map(|c| c.max(1)) else {
        return rows;
    };
    if rows.len() <= cap {
        return rows;
    }

    // Tighten the activity caps one group at a time and re-assemble, in the
    // same least-valuable-first order the rows are laid out in reverse:
    // counts, then todo, then agents. Shrinking beats dropping — four groups
    // at one row each says more than two groups at three — so this runs
    // before the truncation below, and it runs on the expendable end so the
    // live rows keep their rows for as long as possible.
    //
    // Every cap already defaults to 1, so on a default config all three are
    // no-ops and truncation does the work. They earn their keep only where
    // the person raised one (`[segments.agents] max_lines`, say).
    //
    // Same technique as the flat pipeline's cheap rungs
    // (`layout::HeightSqueeze::overrides_activity_caps`): a config clone,
    // re-run in microseconds, far inside the 50ms budget.
    let mut eff = config.clone();
    for tighten in [
        |c: &mut RenderConfig| c.max_completed_lines = 1,
        |c: &mut RenderConfig| c.max_todo_lines = 1,
        |c: &mut RenderConfig| c.max_agent_lines = 1,
    ] {
        tighten(&mut eff);
        rows = build(frame, &eff, palette, width);
        if rows.len() <= cap {
            return rows;
        }
    }

    // Still over. Truncating from the bottom is the right cut here and only
    // here: the rows are ordered running → agents → todo → counts, so what
    // goes is the cumulative count — the most statusline-like figure in the
    // area — and what stays is whatever is happening right now.
    rows.truncate(cap);
    rows
}

fn build(
    frame: &RenderFrame,
    config: &RenderConfig,
    palette: &ThemePalette,
    width: usize,
) -> Vec<String> {
    // Activity rows take the activity strata tier, the same as they do in the
    // flat pipeline — every row here is an activity row, so there is no state
    // tier to pair it with.
    let tinted = super::super::layout::tinted_palette(palette, true, config.pane_tonal_strata);
    crate::render::activity::build_activity_rows_running_first(frame, config, &tinted, width)
}
