use std::borrow::Cow;
use std::ops::Range;

use crate::{
    config::{RenderConfig, WidthDegradeStrategy},
    types::{Line1Metrics, Line3Metrics, QuotaMetrics, RenderFrame},
};

use super::color::{colorize, take_visible_chars, visible_width, ThemePalette, RESET};
use super::fmt::{format_duration, format_number, format_reset_duration, format_speed};
use super::frames;
use super::icons::*;
use super::pane::{apply_pane, LayoutStyle, LineKind, PaneConfig, PaneGroup};

/// Borrow `base` or yield an owned variant whose `separator` is overridden with
/// the row-appropriate strata tier. See `designs/tonal-strata-redesign.md`:
/// `is_activity = true` → `strata_activity`, otherwise → `strata_state`.
fn tinted_palette(base: &ThemePalette, is_activity: bool, tonal: bool) -> Cow<'_, ThemePalette> {
    if !tonal {
        return Cow::Borrowed(base);
    }
    let mut out = base.clone();
    out.separator = if is_activity {
        base.strata_activity.clone()
    } else {
        base.strata_state.clone()
    };
    Cow::Owned(out)
}

pub fn render_frame(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    // CC allocates the statusline a sub-region narrower than the raw
    // terminal (see DEFAULT_PANE_CC_MARGIN). The flat path and the ledger
    // path must both size against the sub-region, not the raw terminal —
    // otherwise CC's overflow detection wraps the over-wide line and
    // collapses the whole multi-line render to one visible line.
    let adjusted = config.terminal_width.map(|w| {
        let mut c = config.clone();
        c.terminal_width = Some(w.saturating_sub(config.pane_cc_margin));
        c
    });
    let config: &RenderConfig = adjusted.as_ref().unwrap_or(config);

    // Ledger owns its full pipeline; every other layout flows through the
    // flat-row assembly below and gets decorated by `apply_pane`. The
    // height-degradation ladder therefore doesn't reach Ledger — its
    // height lever is `ledger_dense`.
    let palette = &config.palette;
    match config.pane_style {
        LayoutStyle::Ledger => return frames::ledger::render(frame, config, palette),
        // Every other layout flows through the flat-line pipeline below
        // and is decorated by `apply_pane`.
        LayoutStyle::None
        | LayoutStyle::Compact
        | LayoutStyle::Budgets
        | LayoutStyle::Velocity
        | LayoutStyle::Console => {}
    }

    let mut lines = assemble_flat(frame, config, HeightSqueeze::default());
    if let Some(cap) = config.max_total_lines {
        let cap = cap.max(1);
        // Rungs are cumulative: each pass re-assembles the full render
        // (microseconds — far inside the 50ms budget) with one more group
        // collapsed, so chrome rows added by `apply_pane` count against
        // the cap too.
        let mut squeeze = HeightSqueeze::default();
        for strategy in &config.height_degrade_order {
            if lines.len() <= cap {
                break;
            }
            squeeze.enable(*strategy);
            lines = assemble_flat(frame, config, squeeze);
        }
        // Ladder exhausted and still over budget — hard truncate from the
        // bottom. The cap is a hard contract: the statusline must never
        // squeeze CC's footer, even at the cost of a cut frame border.
        if lines.len() > cap {
            lines.truncate(cap);
        }
    }
    lines
}

/// Cumulative collapse state for the height-degradation ladder. Each
/// `HeightDegradeStrategy` rung flips one flag; `assemble_flat` consults
/// them while assembling.
#[derive(Debug, Clone, Copy, Default)]
struct HeightSqueeze {
    drop_running_tools: bool,
    collapse_completed_tools: bool,
    collapse_agents: bool,
    collapse_todo: bool,
    merge_activity: bool,
    merge_quota_into_l3: bool,
    drop_line2: bool,
    fuse_core: bool,
}

impl HeightSqueeze {
    fn enable(&mut self, strategy: crate::config::HeightDegradeStrategy) {
        use crate::config::HeightDegradeStrategy as H;
        match strategy {
            H::DropRunningTools => self.drop_running_tools = true,
            H::CollapseCompletedTools => self.collapse_completed_tools = true,
            H::CollapseAgents => self.collapse_agents = true,
            H::CollapseTodo => self.collapse_todo = true,
            H::MergeActivity => self.merge_activity = true,
            H::MergeQuotaIntoL3 => self.merge_quota_into_l3 = true,
            H::DropLine2 => self.drop_line2 = true,
            H::FuseCore => self.fuse_core = true,
        }
    }

    /// True when any flag requires an activity-cap override (the cheap
    /// rungs implemented by tightening `max_*_lines` on a config clone).
    fn overrides_activity_caps(&self) -> bool {
        self.drop_running_tools
            || self.collapse_completed_tools
            || self.collapse_agents
            || self.collapse_todo
    }
}

/// One full flat-pipeline pass: assemble core + activity rows, apply
/// width degradation, decorate with pane chrome. Pure function of
/// `(frame, config, squeeze)` so the height ladder can re-run it with
/// progressively stronger collapse states.
fn assemble_flat(
    frame: &RenderFrame,
    config: &RenderConfig,
    squeeze: HeightSqueeze,
) -> Vec<String> {
    let color = config.color_enabled;
    let base_palette = &config.palette;
    let tonal = config.pane_tonal_strata;

    // Two strata tiers, not four: state rows (Identity/Config/Budget/Quota)
    // share `p_state`; activity rows (Tools/Agents/Todos) get `p_activity`.
    let p_state = tinted_palette(base_palette, false, tonal);
    let p_activity = tinted_palette(base_palette, true, tonal);

    // Compact is permanently "fully squeezed": it never grows past 3 rows,
    // so the ladder flags have nothing left to collapse.
    if matches!(config.pane_style, LayoutStyle::Compact) {
        return assemble_compact(frame, config, &p_state, &p_activity);
    }

    // Budgets is a fixed three-gauge dashboard with its own bespoke row
    // composition (label column + pct + bar), so it short-circuits the
    // generic core-row assembly like compact does. Like compact it ignores
    // `squeeze`: it has no collapsible activity ladder of its own, so a
    // `max_total_lines` cap degrades it via the blunt bottom truncation in
    // `render_frame` rather than the graceful rungs (acceptable — the cap is
    // a hard contract and budgets is a deliberately tall dashboard).
    if matches!(config.pane_style, LayoutStyle::Budgets) {
        return assemble_budgets(frame, config, &p_state, &p_activity);
    }

    let mut lines: Vec<String> = Vec::new();
    let mut groups: Vec<(LineKind, Range<usize>)> = Vec::new();
    let mut line2_idx: Option<usize> = None;

    if squeeze.fuse_core {
        // `FuseCore` rung — the ladder's floor: L1/L2/L3/quota give up
        // their individual rows and the compact layout's priority-packed
        // head takes their place as a single Budget-group row. Framed
        // layouts keep their chrome around it.
        let start = lines.len();
        let head = fused_head_row(frame, config, &p_state);
        if !head.is_empty() {
            lines.push(head);
            groups.push((LineKind::Budget, start..lines.len()));
        }
    } else {
        // Core rows are pushed only when non-empty — a fully toggled-off
        // segment must not cost a blank row (vertical footprint discipline).
        let start = lines.len();
        // Console hoists the identity into the top frame title, so it gets a
        // prefix-less ` · `-separated headline (no `M:` / `G:` icon labels —
        // the title format is its own visual vocabulary). Other layouts keep
        // `format_line1`'s body-row format.
        let identity_str = if matches!(config.pane_style, LayoutStyle::Console) {
            frames::shared::identity_headline(&frame.line1, config, &p_state, " · ")
        } else {
            format_line1(frame, config, &p_state)
        };
        if !identity_str.is_empty() {
            lines.push(identity_str);
            groups.push((LineKind::Identity, start..lines.len()));
        }

        let start = lines.len();
        let line2 = if squeeze.drop_line2 {
            String::new()
        } else {
            format_line2(frame, config, " | ", &p_state)
        };
        line2_idx = (!line2.is_empty()).then_some(lines.len());
        if !line2.is_empty() {
            lines.push(line2);
            groups.push((LineKind::Config, start..lines.len()));
        }

        let start = lines.len();
        let mut line3 = format_line3(frame, config, &p_state);
        if config.show_quota && squeeze.merge_quota_into_l3 {
            // `MergeQuotaIntoL3` rung: quota gives up its row; the threshold-
            // colored percentages survive as a compact L3 suffix.
            if let Some(compact) = format_quota_compact(&frame.quota, config, &p_state) {
                if line3.is_empty() {
                    line3 = compact;
                } else {
                    let sep = colorize(" | ", &p_state.separator, color);
                    line3 = format!("{line3}{sep}{compact}");
                }
            }
        }
        if !line3.is_empty() {
            lines.push(line3);
        }
        if config.show_quota && !squeeze.merge_quota_into_l3 {
            if let Some(line) = format_quota_line(&frame.quota, config, &p_state) {
                lines.push(line);
            }
        }
        if lines.len() > start {
            groups.push((LineKind::Budget, start..lines.len()));
        }
    }

    // The cheap collapse rungs are implemented by tightening the activity
    // caps on a config clone; `MergeActivity` swaps the whole area for a
    // single packed row.
    let eff: Cow<'_, RenderConfig> = if squeeze.overrides_activity_caps() {
        let mut c = config.clone();
        if squeeze.drop_running_tools {
            c.max_tool_lines = 0;
        }
        if squeeze.collapse_completed_tools {
            c.max_completed_lines = 1;
        }
        if squeeze.collapse_agents {
            c.max_agent_lines = 1;
        }
        if squeeze.collapse_todo {
            c.max_todo_lines = 1;
        }
        Cow::Owned(c)
    } else {
        Cow::Borrowed(config)
    };

    let activity_start = lines.len();
    let activity_width = config.terminal_width.unwrap_or(usize::MAX);
    if squeeze.merge_activity {
        lines.extend(crate::render::activity::build_activity_inline_row(
            frame,
            &eff,
            &p_activity,
            activity_width,
        ));
    } else {
        lines.extend(crate::render::activity::build_activity_rows(
            frame,
            &eff,
            &p_activity,
            activity_width,
        ));
    }
    if lines.len() > activity_start {
        groups.push((LineKind::Activity, activity_start..lines.len()));
    }

    // Deduct the active style's horizontal overhead from the degradation
    // budget so content + decoration together fit the terminal.
    // None/Compact/Budgets/Velocity are frameless (overhead 0): Compact and
    // Budgets return from their own bespoke assembly before reaching here;
    // None and Velocity flow through this generic path with no chrome.
    let pane_active = !matches!(
        config.pane_style,
        LayoutStyle::None | LayoutStyle::Compact | LayoutStyle::Budgets | LayoutStyle::Velocity
    );
    let style_overhead = match config.pane_style {
        LayoutStyle::None | LayoutStyle::Compact | LayoutStyle::Budgets | LayoutStyle::Velocity => {
            0
        }
        // Console uses a wall-on-both-sides layout (`│ ` left + internal
        // ` │ ` divider + ` │` right) around a label column whose default
        // group labels (Identity/Config/Budget/Activity) top out at 8
        // chars — ~16 cols total. Budget value for width degradation.
        LayoutStyle::Console => 16,
        // Ledger owns its own pipeline and never reaches this branch.
        LayoutStyle::Ledger => 0,
    };
    let effective_width = config
        .terminal_width
        .map(|w| w.saturating_sub(style_overhead));

    if let Some(width) = effective_width {
        let compressed_line2 = format_line2(frame, config, " ", &p_state);
        lines = apply_width_degradation(
            lines,
            width,
            &config.degrade_order,
            compressed_line2,
            color,
            activity_start,
            line2_idx,
        );
        // `apply_width_degradation` may have truncated `lines`; clamp every
        // group range so the downstream renderer (console) doesn't push
        // chrome for indices that no longer exist (which produces empty
        // framed boxes).
        groups.retain_mut(|(_, range)| {
            range.end = range.end.min(lines.len());
            range.start < range.end
        });
    }

    if pane_active {
        lines = apply_pane(lines, &groups, &pane_config_from(config));
    }

    lines
}

/// The `compact` layout: everything on 2–3 rows, no chrome.
///
/// Row 1 packs the identity (L1) cells; row 2 packs budget (L3) plus the
/// compact quota — which cells appear is still governed by the user's
/// `show_*` toggles. Splitting identity from budget gives the budget row
/// its own width budget: the TOK cell (incl. the `C:%` hit-rate) only
/// competes with other budget cells, so it survives much narrower
/// terminals than the old single fused row. Row 3 is the single packed
/// activity ticker (`build_activity_inline_row`) and is emitted only when
/// there is activity — idle footprint is exactly 2 rows. L2 config counts
/// have no home here by design (reference data; see `docs/layouts.md`).
///
/// The fully-fused 1-row form lives on as the `FuseCore` height-degrade
/// floor (`fused_head_row`), not as this layout.
fn assemble_compact(
    frame: &RenderFrame,
    config: &RenderConfig,
    p_state: &ThemePalette,
    p_activity: &ThemePalette,
) -> Vec<String> {
    use crate::render::activity::cell::CellPriority;

    let color = config.color_enabled;

    let mut lines: Vec<String> = Vec::new();
    let identity = packed_parts_row(line1_parts(frame, config, p_state), config, p_state);
    if !identity.is_empty() {
        lines.push(identity);
    }
    let mut budget_parts = line3_parts(frame, config, p_state, true);
    if config.show_quota {
        if let Some(quota) = format_quota_compact(&frame.quota, config, p_state) {
            budget_parts.push((quota, CellPriority::Required));
        }
    }
    let budget = packed_parts_row(budget_parts, config, p_state);
    if !budget.is_empty() {
        lines.push(budget);
    }

    let activity_width = config.terminal_width.unwrap_or(usize::MAX);
    lines.extend(crate::render::activity::build_activity_inline_row(
        frame,
        config,
        p_activity,
        activity_width,
    ));

    // Single-pass width fit: no compress/drop ladder needed at 2–3 rows.
    if let Some(width) = config.terminal_width {
        lines = lines
            .into_iter()
            .map(|line| truncate_to_width(&line, width, color))
            .collect();
    }
    lines
}

/// Budgets layout column widths. The label column fits the longest label
/// (`5H QUOTA` / `7D QUOTA`, 8 cells); the percentage column fits `100%`.
/// The gauge is wide (`▰─·` reused at `BUDGET_GAUGE_W`) so the three budget
/// windows share a long, comparable baseline.
const BUDGET_LABEL_W: usize = 8;
const BUDGET_PCT_W: usize = 4;
const BUDGET_GAUGE_W: usize = 24;

/// One budgets-layout gauge row: `LABEL    NN%  ▰▰▰▰▰▰───·──   <trailing>`.
///
/// The percentage is the D2 "anchor" — the row's hero metric, rendered in
/// its threshold color; the label (tag tier) and trailing context
/// (structural) recede. There is no SGR bold primitive in this codebase,
/// so the colour hierarchy carries the emphasis, exactly like the ledger
/// budget rows. The gauge bar comes from the hub's `gauge_bar` helper so
/// the `widgets::gauge::render` call stays inside the dispatch-hub module.
#[allow(clippy::too_many_arguments)]
fn budget_gauge_row(
    label: &str,
    pct: f64,
    pct_color: &str,
    marks: &[u64],
    gauge_w: usize,
    trailing: &str,
    config: &RenderConfig,
    p: &ThemePalette,
) -> String {
    let color = config.color_enabled;
    let label_cell = frames::shared::pad_to(&colorize(label, &p.tag_label, color), BUDGET_LABEL_W);
    let pct_cell = frames::shared::pad_to(
        &colorize(&format!("{pct:.0}%"), pct_color, color),
        BUDGET_PCT_W,
    );
    let bar = frames::shared::gauge_bar(
        pct as u64,
        gauge_w,
        marks,
        pct_color,
        p,
        config.glyph_mode,
        color,
    );
    let mut row = format!("{label_cell}  {pct_cell}  {bar}");
    if !trailing.is_empty() {
        row.push_str("   ");
        row.push_str(trailing);
    }
    row
}

/// The `budgets` layout: identity, then CONTEXT / 5H QUOTA / 7D QUOTA as
/// three column-aligned equal-weight gauges, then a TOKENS + cost row.
/// The three gauges share one width (scaled together to the terminal) so
/// burn across the windows reads on a common spatial axis — the inline
/// `CTX% … 5h% … 7d%` form scatters them with no shared baseline. Each
/// gauge keeps its own `pct` + `used/total` (or reset) text, so nothing
/// is traded for the bar. Frameless — routes through `apply_pane` like
/// `none`. Quota rows appear only when `[segments.quota]` is enabled.
fn assemble_budgets(
    frame: &RenderFrame,
    config: &RenderConfig,
    p_state: &ThemePalette,
    p_activity: &ThemePalette,
) -> Vec<String> {
    let color = config.color_enabled;
    let mode = config.glyph_mode;
    let mut lines: Vec<String> = Vec::new();

    // Row 1 — identity (flat `M: | E: | P: | G:` body format).
    let identity = format_line1(frame, config, p_state);
    if !identity.is_empty() {
        lines.push(identity);
    }

    // All three gauges share one width so their fills line up vertically.
    // Reserve the label/pct columns, gaps, and room for the trailing text;
    // scale the bar down on narrow terminals (clamped so it never vanishes).
    let gauge_w = match config.terminal_width {
        Some(w) => {
            // label + gaps + pct + gap + room for the trailing cell. 24 cells
            // covers the longest trailing string (`resets 10d 23h 59m`, ~18,
            // + the 3-space gap); a longer one only clips via the truncate
            // backstop below, never overflows the row.
            let reserved = BUDGET_LABEL_W + 2 + BUDGET_PCT_W + 2 + 24;
            w.saturating_sub(reserved).clamp(6, BUDGET_GAUGE_W)
        }
        None => BUDGET_GAUGE_W,
    };

    // Row 2 — CONTEXT gauge (with the ⟳N compaction marker as trailing).
    if config.show_context {
        if let (Some(pct), Some(size)) = (
            frame.line3.context_used_percentage,
            frame.line3.context_window_size,
        ) {
            let pct_color = p_state.color_for_ctx_pct(pct);
            let used = ((size as f64) * (pct as f64) / 100.0) as u64;
            let mut trailing = format!(
                "{}{}{}",
                colorize(&format_number(used), &p_state.primary, color),
                colorize("/", &p_state.separator, color),
                colorize(&format_number(size), &p_state.secondary, color),
            );
            if frame.compact_count > 0 {
                let marker = format!(
                    "   {}{}",
                    glyph(mode, ICON_COMPACT, "~"),
                    frame.compact_count
                );
                trailing.push_str(&colorize(&marker, &p_state.structural, color));
            }
            lines.push(budget_gauge_row(
                "CONTEXT",
                pct as f64,
                pct_color,
                &ThemePalette::ctx_marks(),
                gauge_w,
                &trailing,
                config,
                p_state,
            ));
        }
    }

    // Rows 3/4 — 5H / 7D quota gauges (only when quota is enabled + present).
    if config.show_quota && frame.quota.has_data() {
        let reset_trailing = |minutes: Option<u64>| -> String {
            minutes
                .map(|m| {
                    colorize(
                        &format!("resets {}", format_reset_duration(m)),
                        &p_state.structural,
                        color,
                    )
                })
                .unwrap_or_default()
        };
        if config.show_quota_five_hour {
            if let Some(pct) = frame.quota.five_hour_pct {
                lines.push(budget_gauge_row(
                    "5H QUOTA",
                    pct,
                    p_state.color_for_quota_pct(pct),
                    &frames::shared::QUOTA_MARKS,
                    gauge_w,
                    &reset_trailing(frame.quota.five_hour_reset_minutes),
                    config,
                    p_state,
                ));
            }
        }
        if config.show_quota_seven_day {
            if let Some(pct) = frame.quota.seven_day_pct {
                lines.push(budget_gauge_row(
                    "7D QUOTA",
                    pct,
                    p_state.color_for_quota_pct(pct),
                    &frames::shared::QUOTA_MARKS,
                    gauge_w,
                    &reset_trailing(frame.quota.seven_day_reset_minutes),
                    config,
                    p_state,
                ));
            }
        }
    }

    // Row 5 — tokens + cost (reuses the shared cells; `TOK` is its label).
    let mut budget_cells: Vec<String> = Vec::new();
    if config.show_tokens {
        let speed = if config.show_speed {
            frame.line3.output_speed_toks_per_sec
        } else {
            None
        };
        budget_cells.push(format_tokens_segment(&frame.line3, speed, config, p_state));
    }
    if config.show_cost {
        budget_cells.push(format_cost_segment(&frame.line3, config, p_state));
    }
    if !budget_cells.is_empty() {
        lines.push(budget_cells.join("   "));
    }

    // Activity rows — same multi-row treatment as `none`.
    let activity_width = config.terminal_width.unwrap_or(usize::MAX);
    lines.extend(crate::render::activity::build_activity_rows(
        frame,
        config,
        p_activity,
        activity_width,
    ));

    // Safety net: clamp any over-wide row (identity / tokens can run long).
    // The gauge rows already fit via `gauge_w` scaling.
    if let Some(width) = config.terminal_width {
        lines = lines
            .into_iter()
            .map(|line| truncate_to_width(&line, width, color))
            .collect();
    }
    lines
}

/// Packs `(text, priority)` parts into one row joined with the standard
/// ` | ` separator, width-aware: Optional cells drop first, rightmost
/// first — never a blind right-tail truncation. Returns `""` when there
/// are no parts.
fn packed_parts_row(
    parts: Vec<(String, crate::render::activity::cell::CellPriority)>,
    config: &RenderConfig,
    p_state: &ThemePalette,
) -> String {
    use crate::render::activity::budget::pack_with_separator;
    use crate::render::activity::cell::Cell;

    let color = config.color_enabled;
    let sep = colorize(" | ", &p_state.separator, color);

    let cells: Vec<Cell> = parts
        .into_iter()
        .map(|(part, priority)| {
            let w = visible_width(&part);
            Cell::label(part, w, priority)
        })
        .collect();
    if cells.is_empty() {
        return String::new();
    }
    let row_width = config.terminal_width.unwrap_or(usize::MAX);
    pack_with_separator(&cells, row_width, &sep, 3, color)
}

/// The fused head row used by the `FuseCore` height-degradation rung:
/// identity (L1) and budget (L3) cells joined with the standard ` | `
/// separator, plus the compact quota when shown.
///
/// The row packs width-aware so the RIGHT-side essentials (cost, quota)
/// survive narrow terminals: Optional reference cells (TOK breakdown,
/// project path, version, style, …) drop first, rightmost first —
/// never a blind right-tail truncation that would cut the budget area.
fn fused_head_row(frame: &RenderFrame, config: &RenderConfig, p_state: &ThemePalette) -> String {
    use crate::render::activity::cell::CellPriority;

    let mut parts = line1_parts(frame, config, p_state);
    parts.extend(line3_parts(frame, config, p_state, false));
    if config.show_quota {
        if let Some(quota) = format_quota_compact(&frame.quota, config, p_state) {
            parts.push((quota, CellPriority::Required));
        }
    }
    packed_parts_row(parts, config, p_state)
}

fn pane_config_from(config: &RenderConfig) -> PaneConfig {
    let default_groups = vec![
        PaneGroup {
            label: "Identity".to_string(),
            kinds: vec![LineKind::Identity],
        },
        PaneGroup {
            label: "ENV".to_string(),
            kinds: vec![LineKind::Config],
        },
        PaneGroup {
            label: "Budget".to_string(),
            kinds: vec![LineKind::Budget],
        },
        PaneGroup {
            label: "Activity".to_string(),
            kinds: vec![LineKind::Activity],
        },
    ];
    PaneConfig {
        style: config.pane_style,
        min_width: config.pane_min_width,
        max_width: config.pane_max_width,
        groups: default_groups,
        glyph_mode: config.glyph_mode,
        terminal_width: config.terminal_width,
        cc_margin: config.pane_cc_margin,
    }
}

fn format_line1(frame: &RenderFrame, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;
    let sep = colorize(" | ", &p.separator, color);
    line1_parts(frame, config, p)
        .into_iter()
        .map(|(part, _)| part)
        .collect::<Vec<_>>()
        .join(&sep)
}

/// L1 segments with a glance-priority tag for the compact layout's
/// width-aware packing: `Required` cells (model, git) survive width
/// pressure; `Optional` cells (style, version, project, …) drop first,
/// rightmost first. The flat layouts join all parts and ignore the tag.
fn line1_parts(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Vec<(String, crate::render::activity::cell::CellPriority)> {
    use crate::render::activity::cell::CellPriority::{Optional, Required};
    let mode = config.glyph_mode;
    let color = config.color_enabled;

    let mut parts: Vec<(String, crate::render::activity::cell::CellPriority)> = Vec::new();

    if config.show_model {
        let model_label = colorize(&glyph(mode, ICON_MODEL, "M:"), &p.stable_blue, color);
        let model_val = colorize(&frame.line1.model, &p.stable_blue, color);
        parts.push((format!("{model_label}{model_val}"), Required));
    }

    if config.show_effort {
        if let Some(level) = &frame.line1.effort_level {
            let cell = frames::shared::render_effort_visual(
                config.effective_effort_visual(),
                level,
                mode,
                p,
                color,
            );
            parts.push((cell, Optional));
        }
    }

    if config.show_thinking && frame.line1.thinking_enabled == Some(true) {
        // Label-only pill — no value; `enabled: false` or missing → omitted entirely.
        // Trim before colorizing so the ANSI reset stays tight against the glyph.
        let raw = glyph(mode, ICON_THINKING, "[T]");
        let label = colorize(raw.trim_end(), &p.head_thinking, color);
        parts.push((label, Optional));
    }

    if config.show_agent {
        if let Some(agent_name) = &frame.line1.agent_name {
            let label = colorize(&glyph(mode, ICON_AGENT, "AG:"), &p.head_agent, color);
            let val = colorize(agent_name, &p.head_agent, color);
            parts.push((format!("{label}{val}"), Optional));
        }
    }

    if config.show_style {
        let style_label = colorize(&glyph(mode, ICON_STYLE, "S:"), &p.secondary, color);
        let style_val = colorize(&frame.line1.output_style, &p.secondary, color);
        parts.push((format!("{style_label}{style_val}"), Optional));
    }

    if config.show_version {
        let version_label = colorize(&glyph(mode, ICON_VERSION, "CC:"), &p.secondary, color);
        let version_val = colorize(&frame.line1.claude_code_version, &p.secondary, color);
        parts.push((format!("{version_label}{version_val}"), Optional));
    }

    if config.show_project {
        let project_label = colorize(&glyph(mode, ICON_PROJECT, "P:"), &p.secondary, color);
        let project_val = colorize(&frame.line1.project_path, &p.secondary, color);
        parts.push((format!("{project_label}{project_val}"), Optional));
    }

    if config.show_git && frame.line1.has_git_branch() {
        let git_label = colorize(&glyph(mode, ICON_GIT, "G:"), p.git_green(), color);
        let git_val = format_git_status(&frame.line1, config, p);
        parts.push((format!("{git_label}{git_val}"), Required));
    }

    parts
}

pub(crate) fn format_line2(
    frame: &RenderFrame,
    config: &RenderConfig,
    separator: &str,
    p: &ThemePalette,
) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let sep = colorize(separator, &p.separator, color);

    let format_item = |icon: &str, indicator_color: &str, label: &str, count: u32| -> String {
        let count_str = colorize(&count.to_string(), &p.primary, color);
        let label_str = colorize(label, &p.structural, color);

        match mode {
            crate::config::GlyphMode::Icon => {
                let icon_str = colorize(&format!("{icon} "), indicator_color, color);
                format!("{icon_str}{count_str} {label_str}")
            }
            crate::config::GlyphMode::Ascii => {
                format!("{count_str} {label_str}")
            }
        }
    };

    let mut parts: Vec<String> = Vec::new();

    if config.show_claude_md {
        parts.push(format_item(
            ICON_CLAUDE_MD,
            &p.indicator_claude_md,
            "CLAUDE.md",
            frame.line2.claude_md_count,
        ));
    }
    if config.show_claude_md && frame.line2.agents_md_count > 0 {
        // Same semantic class as CLAUDE.md — reuse its icon and indicator
        // color, and zero-suppress (like plugins) so the cell only appears
        // when a root AGENTS.md exists.
        parts.push(format_item(
            ICON_CLAUDE_MD,
            &p.indicator_claude_md,
            "AGENTS.md",
            frame.line2.agents_md_count,
        ));
    }
    if config.show_rules {
        parts.push(format_item(
            ICON_RULES,
            &p.indicator_rules,
            "rules",
            frame.line2.rules_count,
        ));
    }
    if config.show_memory {
        parts.push(format_item(
            ICON_MEMORY,
            &p.indicator_memory,
            "memories",
            frame.line2.memory_count,
        ));
    }
    if config.show_hooks {
        parts.push(format_item(
            ICON_HOOKS,
            &p.indicator_hooks,
            "hooks",
            frame.line2.hooks_count,
        ));
    }
    if config.show_mcp {
        parts.push(format_item(
            ICON_MCP,
            &p.indicator_mcp,
            "MCPs",
            frame.line2.mcp_count,
        ));
    }
    if config.show_skills {
        parts.push(format_item(
            ICON_SKILLS,
            &p.indicator_skills,
            "skills",
            frame.line2.skills_count,
        ));
    }
    if config.show_plugins && frame.line2.plugins_count > 0 {
        // Reuse indicator_mcp tier — plugins are heterogeneous bundles of
        // MCPs / skills / hooks, and sharing the MCP indicator color keeps
        // L2 visually grouped without a palette change.
        parts.push(format_item(
            ICON_PLUGIN,
            &p.indicator_mcp,
            "plugins",
            frame.line2.plugins_count,
        ));
    }
    if config.show_duration {
        let duration_text = format_duration(frame.line2.elapsed_minutes);
        let item = match mode {
            crate::config::GlyphMode::Icon => {
                let icon_str =
                    colorize(&format!("{} ", ICON_ELAPSED), &p.indicator_duration, color);
                let time_str = colorize(&duration_text, &p.primary, color);
                format!("{icon_str}{time_str}")
            }
            crate::config::GlyphMode::Ascii => colorize(&duration_text, &p.primary, color),
        };
        parts.push(item);
    }

    parts.join(&sep)
}

fn format_line3(frame: &RenderFrame, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;
    let sep = colorize(" | ", &p.separator, color);
    line3_parts(frame, config, p, false)
        .into_iter()
        .map(|(part, _)| part)
        .collect::<Vec<_>>()
        .join(&sep)
}

/// L3 segments with a glance-priority tag — see `line1_parts`. CTX and
/// cost are the alert-bearing cells (`Required`); the token breakdown is
/// reference data (`Optional`, dropped first under compact width
/// pressure).
///
/// `include_cache_spark` opts the row into the knob-gated cache-trend
/// sparkline as a separate Optional cell pushed AFTER the TOK cell:
/// `pack_with_separator` drops the RIGHTMOST Optional first, so the
/// drop order is spark → TOK → never the Required CTX/cost cells. Only
/// the compact layout passes `true`; the flat L3 row and the FuseCore
/// height-degrade floor stay unchanged.
fn line3_parts(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
    include_cache_spark: bool,
) -> Vec<(String, crate::render::activity::cell::CellPriority)> {
    use crate::render::activity::cell::CellPriority::{Optional, Required};
    let color = config.color_enabled;
    let mode = config.glyph_mode;

    let mut parts: Vec<(String, crate::render::activity::cell::CellPriority)> = Vec::new();

    if config.show_context {
        let mut ctx = format_context_segment(&frame.line3, config, p, &frame.ctx_history);

        // 2d: compact boundary marker — ⟳N (icon) / ~N (ascii), structural color.
        if frame.compact_count > 0 {
            let marker = format!(" {}{}", glyph(mode, ICON_COMPACT, "~"), frame.compact_count);
            ctx.push_str(&colorize(&marker, &p.structural, color));
        }

        // 2e: API-error badge — ⚠API (icon) / !API (ascii), alert_red color.
        if frame.last_api_error_ms.is_some() {
            let badge = format!(" {}API", glyph(mode, ICON_API_ERROR, "!"));
            ctx.push_str(&colorize(&badge, &p.alert_red, color));
        }

        parts.push((ctx, Required));
    }
    if config.show_tokens {
        let speed = if config.show_speed {
            frame.line3.output_speed_toks_per_sec
        } else {
            None
        };
        parts.push((
            format_tokens_segment(&frame.line3, speed, config, p),
            Optional,
        ));
        // Cache-trend spark rides with the C cell (no C cell → no spark)
        // as its own Optional cell so it drops before TOK under pressure.
        if include_cache_spark && config.show_cache_trend {
            let spark = frames::shared::cache_trend_sparkline(
                &frame.cache_history,
                &frame.line3,
                mode,
                p,
                color,
            );
            if !spark.is_empty() {
                parts.push((spark, Optional));
            }
        }
    }
    if config.show_cost {
        parts.push((format_cost_segment(&frame.line3, config, p), Required));
    }

    parts
}

fn format_git_status(line1: &Line1Metrics, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;

    let mut status = colorize(&line1.git_branch, p.git_green(), color);
    if line1.git_dirty {
        status.push_str(&colorize("*", p.git_modified(), color));
    }
    if line1.git_ahead > 0 {
        status.push_str(&colorize(
            &format!(" ↑{}", line1.git_ahead),
            p.git_ahead(),
            color,
        ));
    }
    if line1.git_behind > 0 {
        status.push_str(&colorize(
            &format!(" ↓{}", line1.git_behind),
            p.git_behind(),
            color,
        ));
    }

    // File stats: !3 +1 ✘2 ?4 (Starship-style, zero counts omitted)
    if config.show_git_stats {
        let stats: Vec<String> = [
            ('!', line1.git_modified, p.git_modified()),
            ('+', line1.git_added, p.git_added()),
            ('✘', line1.git_deleted, p.git_deleted()),
            ('?', line1.git_untracked, &p.structural),
        ]
        .iter()
        .filter(|(_, count, _)| *count > 0)
        .map(|(prefix, count, stat_color)| colorize(&format!("{prefix}{count}"), stat_color, color))
        .collect();

        if !stats.is_empty() {
            status.push(' ');
            status.push_str(&stats.join(" "));
        }
    }

    if config.show_worktree && line1.in_worktree {
        status.push_str(&colorize(" (WT)", &p.structural, color));
    }

    status
}

fn format_context_segment(
    line3: &Line3Metrics,
    config: &RenderConfig,
    p: &ThemePalette,
    history: &[(u8, u64)],
) -> String {
    // Dispatches through the visual hub so flat layouts inherit per-segment
    // composability from `context_visual`. Default for every flat layout is
    // `text` (legacy `CTX:43% (86.0k/200.0k)` form). Users who set
    // `context_visual = "gauge"` get a gauge bar inside the same row;
    // `"text+sparkline"` adds a braille trend after the numbers.
    frames::shared::render_context_visual(
        config.effective_context_visual(),
        line3,
        history,
        frames::shared::CTX_BAR_WIDTH,
        config.glyph_mode,
        p,
        config.color_enabled,
    )
}

fn format_tokens_segment(
    line3: &Line3Metrics,
    speed: Option<f64>,
    config: &RenderConfig,
    p: &ThemePalette,
) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;

    let has_data = line3.input_tokens.is_some()
        || line3.output_tokens.is_some()
        || line3.cache_creation_tokens.is_some()
        || line3.cache_read_tokens.is_some();

    // Values use primary color when data exists, structural (dimmed) when absent
    let val_color = if has_data { &p.primary } else { &p.structural };

    let input_str = line3
        .input_tokens
        .map(format_number)
        .unwrap_or_else(|| "--".to_string());
    let output_str = line3
        .output_tokens
        .map(format_number)
        .unwrap_or_else(|| "--".to_string());
    // Cache hit rate — `C:50%` threshold-colored, or structural `C:--`
    // when there is no cache signal (never a misleading red 0%).
    let (cache_str, cache_color) = match line3.cache_hit_pct() {
        Some(pct) => (
            format!("{pct:.0}%"),
            p.color_for_cache_hit_pct(pct, line3.cache_creation_share()),
        ),
        None => ("--".to_string(), &p.structural as &str),
    };

    // Speed inline after output tokens: "O:20.0k ↗1.5K/s"
    let speed_part = speed
        .map(|s| colorize(&format!(" {}", format_speed(s)), val_color, color))
        .unwrap_or_default();

    let label = colorize("TOK ", &p.structural, color);
    let parts = [
        format!(
            "{}{}",
            colorize(&glyph(mode, ICON_TOKEN_INPUT, "I:"), &p.structural, color),
            colorize(&input_str, val_color, color),
        ),
        format!(
            "{}{}{}",
            colorize(&glyph(mode, ICON_TOKEN_OUTPUT, "O:"), &p.structural, color),
            colorize(&output_str, val_color, color),
            speed_part,
        ),
        format!(
            "{}{}",
            colorize(
                &glyph(mode, ICON_TOKEN_CACHE_CREATE, "C:"),
                &p.structural,
                color
            ),
            colorize(&cache_str, cache_color, color),
        ),
    ];
    format!("{label}{}", parts.join(" "))
}

fn format_cost_segment(line3: &Line3Metrics, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;

    let total_cost = line3.total_cost_usd.unwrap_or(0.0);
    let per_hour = line3
        .total_duration_ms
        .filter(|duration| *duration > 0)
        .map(|duration| total_cost / ((duration as f64) / 3_600_000.0))
        .unwrap_or(0.0);

    let rate_color = p.color_for_burn_rate(per_hour);

    let total_str = colorize(&format!("${total_cost:.2}"), &p.cost_base, color);
    let open_paren = colorize("(", &p.separator, color);
    let rate_str = colorize(&format!("${per_hour:.2}/h"), rate_color, color);
    let close_paren = colorize(")", &p.separator, color);
    format!("{total_str} {open_paren}{rate_str}{close_paren}")
}

fn format_quota_line(
    quota: &QuotaMetrics,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Option<String> {
    if !quota.has_data() {
        return None;
    }

    let mode = config.glyph_mode;
    let color = config.color_enabled;

    let prefix = colorize(&glyph(mode, ICON_QUOTA, "Q:"), &p.structural, color);

    let mut parts: Vec<String> = Vec::new();

    if config.show_quota_five_hour {
        parts.push(format_quota_period(
            "5h",
            quota.five_hour_pct,
            quota.five_hour_reset_minutes,
            config,
            p,
        ));
    }

    if config.show_quota_seven_day {
        parts.push(format_quota_period(
            "7d",
            quota.seven_day_pct,
            quota.seven_day_reset_minutes,
            config,
            p,
        ));
    }

    if parts.is_empty() {
        return None;
    }

    Some(format!("{prefix}{}", parts.join(" ")))
}

/// Compact quota for the `MergeQuotaIntoL3` height-degradation rung:
/// `5h:62% 7d:41%` — threshold-colored percentages only. The reset time
/// is dropped; under height pressure the essential signal is how much
/// budget remains, not when it refills.
/// `("5h:", "62%")` — the colored label + threshold-colored percentage
/// cells shared by the full quota line (`format_quota_period`) and the
/// compact merged form (`format_quota_compact`). Single source so a
/// threshold or format change can't drift between the two.
fn quota_label_pct_cells(
    label: &str,
    pct_val: f64,
    p: &ThemePalette,
    color: bool,
) -> (String, String) {
    let label_str = colorize(&format!("{label}:"), &p.secondary, color);
    let pct_str = colorize(
        &format!("{pct_val:.0}%"),
        p.color_for_quota_pct(pct_val),
        color,
    );
    (label_str, pct_str)
}

fn format_quota_compact(
    quota: &QuotaMetrics,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Option<String> {
    if !quota.has_data() {
        return None;
    }
    let color = config.color_enabled;
    let mut parts: Vec<String> = Vec::new();
    let mut push_window = |label: &str, pct: Option<f64>| {
        if let Some(pct_val) = pct {
            let (label_str, pct_str) = quota_label_pct_cells(label, pct_val, p, color);
            parts.push(format!("{label_str}{pct_str}"));
        }
    };
    if config.show_quota_five_hour {
        push_window("5h", quota.five_hour_pct);
    }
    if config.show_quota_seven_day {
        push_window("7d", quota.seven_day_pct);
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn format_quota_period(
    label: &str,
    pct: Option<f64>,
    reset_minutes: Option<u64>,
    config: &RenderConfig,
    p: &ThemePalette,
) -> String {
    let color = config.color_enabled;

    match pct {
        Some(pct_val) => {
            let (label_str, pct_str) = quota_label_pct_cells(label, pct_val, p, color);

            let reset_part = reset_minutes
                .map(|m| {
                    let duration = format_reset_duration(m);
                    let open = colorize(" (", &p.separator, color);
                    let txt = colorize(&format!("resets {duration}"), &p.structural, color);
                    let close = colorize(")", &p.separator, color);
                    format!("{open}{txt}{close}")
                })
                .unwrap_or_default();

            // Bar precedes the percentage when `quota_visual = "gauge"`
            // (D2 from the F-revival plan). Empty string when the visual
            // spec doesn't include `gauge` — caller renders text only.
            let bar = crate::render::frames::shared::render_quota_visual(
                config.effective_quota_visual(),
                pct_val,
                p,
                config.glyph_mode,
                color,
            );
            let bar_part = if bar.is_empty() {
                String::new()
            } else {
                format!("{bar} ")
            };

            if pct_val >= 100.0 {
                let limit_text = colorize("Limit reached", p.ctx_critical(), color);
                format!("{label_str} {bar_part}{limit_text}{reset_part}")
            } else {
                format!("{label_str} {bar_part}{pct_str}{reset_part}")
            }
        }
        None => {
            let label_str = colorize(&format!("{label}:"), &p.secondary, color);
            let dash = colorize("--", &p.structural, color);
            format!("{label_str} {dash}")
        }
    }
}

fn apply_width_degradation(
    mut lines: Vec<String>,
    width: usize,
    strategies: &[WidthDegradeStrategy],
    compressed_line2: String,
    color_enabled: bool,
    activity_start: usize,
    line2_idx: Option<usize>,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    if lines_fit_width(&lines, width) {
        return lines;
    }

    // "Core" = everything before activity rows (L1 identity, L2 config, L3
    // budget, optional quota). Quota lives between L3 and the activity start,
    // so dropping activity lines must truncate to `activity_start`, NOT to a
    // hardcoded core count — otherwise quota gets dropped together with
    // activity even though it belongs to the Budget group.
    let core_end = activity_start;

    for strategy in strategies {
        if lines_fit_width(&lines, width) {
            break;
        }

        match strategy {
            WidthDegradeStrategy::DropActivityLinesFirst => {
                if lines.len() > core_end {
                    lines.truncate(core_end);
                }
            }
            WidthDegradeStrategy::CompressLine2 => {
                // L2's index is no longer fixed — empty core rows are
                // skipped during assembly, shifting everything below.
                if let Some(line2) = line2_idx.and_then(|i| lines.get_mut(i)) {
                    *line2 = compressed_line2.clone();
                }
            }
            WidthDegradeStrategy::CompressCoreLines => {
                for index in 0..lines.len().min(core_end) {
                    lines[index] = truncate_to_width(&lines[index], width, color_enabled);
                }
            }
        }
    }

    lines
        .into_iter()
        .map(|line| truncate_to_width(&line, width, color_enabled))
        .collect()
}

fn lines_fit_width(lines: &[String], width: usize) -> bool {
    lines.iter().all(|line| visible_width(line) <= width)
}

pub fn truncate_to_width(line: &str, width: usize, color_enabled: bool) -> String {
    if visible_width(line) <= width {
        return line.to_string();
    }

    if width <= 3 {
        let mut result = take_visible_chars(line, width);
        if color_enabled {
            result.push_str(RESET);
        }
        return result;
    }

    let mut truncated = take_visible_chars(line, width - 3);
    if color_enabled {
        truncated.push_str(RESET);
    }
    truncated.push_str("...");
    truncated
}
