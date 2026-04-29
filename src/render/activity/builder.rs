//! Build `Cell`s from activity-frame data and pack them into row strings.
//!
//! Public entry point: `build_activity_rows(frame, config, palette, available_width)`.
//! See `designs/activity-width-budget.md` §4 for cell descriptors.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{GlyphMode, RenderConfig};
use crate::render::color::{colorize, visible_width, ThemePalette};
use crate::render::fmt::format_agent_elapsed;
use crate::render::icons::{glyph, ICON_AGENT, ICON_AGENT_DONE, ICON_GROUP_PARALLEL, ICON_TODO};
use super::cells::recent_tool::build_recent_tool_cell;
use crate::types::{AgentSummary, CompletedToolCount, RenderFrame, TodoSummary};

use super::agent_groups::{avg_elapsed_ms, classify, AgentGroup};
use super::budget::{pack_multi_row, pack_with_separator};
use super::cell::{Cell, CellBody, CellPriority, TailFragment};
use super::truncate::TruncationStrategy;

/// Visible separator between cells in an activity row. Cluster layouts
/// reuse this so flat-row and cluster paths render identical separators.
pub const ROW_SEPARATOR: &str = " | ";
pub const ROW_SEPARATOR_W: usize = 3;
/// Sub-item separator inside a heterogeneous parallel group cell. Space-padded
/// `+` reads as "and"; visually distinct from the row-level ` | ` (point/cross
/// vs vertical bar) and in-text `+` (e.g. `C++`) almost never carries spaces.
const GROUP_SUBITEM_SEPARATOR: &str = " + ";

/// Render the L4+ activity rows for the given frame, ordered as: completed
/// tool counts, recent/running tools, agent groups, todo. Each row is
/// independently width-fitted by `pack_with_separator`.
pub fn build_activity_rows(
    frame: &RenderFrame,
    config: &RenderConfig,
    palette: &ThemePalette,
    available_width: usize,
) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let color = config.color_enabled;
    let sep = colorize(ROW_SEPARATOR, &palette.separator, color);

    if config.show_tools {
        if !frame.completed_tools.is_empty() {
            let cells: Vec<Cell> = frame
                .completed_tools
                .iter()
                .take(config.max_completed_tools.max(1))
                .map(|c| build_completed_tool_cell(c, palette, color))
                .collect();
            rows.extend(build_completed_tool_rows(
                &cells,
                available_width,
                &sep,
                palette,
                config,
            ));
        }

        if !frame.tools.is_empty() {
            let cells: Vec<Cell> = frame
                .tools
                .iter()
                .take(config.max_tool_lines.max(1))
                .map(|t| build_recent_tool_cell(t, config.glyph_mode, palette, color))
                .collect();
            let row = pack_with_separator(&cells, available_width, &sep, ROW_SEPARATOR_W, color);
            if !row.is_empty() {
                rows.push(row);
            }
        }
    }

    if config.show_agents {
        rows.extend(build_agent_rows(
            &frame.agents,
            config,
            palette,
            available_width,
            &sep,
        ));
    }

    if config.show_todo {
        if let Some(todo) = &frame.todo {
            rows.extend(build_todo_rows(todo, config, palette, available_width));
        }
    }

    rows
}

// ── Shared row helpers ────────────────────────────────────────────────

/// `… + N more {label}` summary used by both the completed-tool overflow
/// and the agent overflow. Picks the singular noun when `count == 1`.
fn overflow_summary(
    count: usize,
    singular: &str,
    plural: &str,
    p: &ThemePalette,
    color: bool,
) -> String {
    let label = if count == 1 { singular } else { plural };
    colorize(
        &format!("\u{2026} + {count} more {label}"),
        &p.structural,
        color,
    )
}

/// First non-empty line of an agent's description — the canonical short
/// blurb used for cell bodies. Multi-line descriptions are common when the
/// caller pastes a paragraph; everything past the first line is dropped
/// to keep the row 1-line-tall.
fn first_desc_line(a: &AgentSummary) -> &str {
    a.description.lines().next().unwrap_or("")
}

/// Group agents by `agent_type`, preserving first-seen order. Each type
/// appears exactly once; its descriptions accumulate in arrival order.
/// Used by the heterogeneous parallel cell to surface type-runs as
/// `Type ×N [d1 + d2]` instead of repeating the type prefix per agent.
fn bucket_by_type<'a>(group: &[&'a AgentSummary]) -> Vec<(&'a str, Vec<&'a str>)> {
    let mut buckets: Vec<(&str, Vec<&str>)> = Vec::with_capacity(group.len());
    for a in group {
        let t = a.agent_type.as_deref().unwrap_or("agent");
        let d = first_desc_line(a);
        if let Some(b) = buckets.iter_mut().find(|(ty, _)| *ty == t) {
            b.1.push(d);
        } else {
            buckets.push((t, vec![d]));
        }
    }
    buckets
}

// ── Tail fragment helpers (shared across the agent cell builders) ───

/// `[model]` slack tail — shown when the row has body slack to spare. Returns
/// `None` if the agent has no model attached so callers can chain `.extend`.
fn model_slack_tail(model: &Option<String>, p: &ThemePalette, color: bool) -> Option<TailFragment> {
    let m = model.as_ref()?;
    let text = colorize(&format!(" [{m}]"), &p.structural, color);
    Some(TailFragment::Slack {
        text,
        width: 3 + m.chars().count(), // " [" + model + "]"
    })
}

/// Pinned ` (content)` tail in `separator`/`structural`/`separator` colors.
/// Used for elapsed `(2m)`, average `(avg 1m 30s)`, etc.
fn parens_pinned_tail(content: &str, p: &ThemePalette, color: bool) -> TailFragment {
    let open = colorize(" (", &p.separator, color);
    let body = colorize(content, &p.structural, color);
    let close = colorize(")", &p.separator, color);
    TailFragment::Pinned {
        text: format!("{open}{body}{close}"),
        width: 3 + content.chars().count(),
    }
}

// ── Cell builders ─────────────────────────────────────────────────────

/// Width-adaptive multi-row packer for the completed-tool segment.
/// `cells` are importance-sorted; greedily fills rows up to
/// `config.max_completed_lines`, then collapses the remainder into a
/// `… + N more tools` summary at the BOTTOM — hidden cells are the
/// least-important, so the summary reads naturally below the visible rows.
fn build_completed_tool_rows(
    cells: &[Cell],
    available_width: usize,
    sep: &str,
    palette: &ThemePalette,
    config: &RenderConfig,
) -> Vec<String> {
    let color = config.color_enabled;
    let max_rows = config.max_completed_lines.max(1);
    let (mut rows, shown) = pack_multi_row(
        cells,
        available_width,
        sep,
        ROW_SEPARATOR_W,
        color,
        Some(max_rows),
    );
    let hidden = cells.len().saturating_sub(shown);
    if hidden > 0 {
        rows.push(overflow_summary(hidden, "tool", "tools", palette, color));
    }
    rows
}

fn build_completed_tool_cell(c: &CompletedToolCount, p: &ThemePalette, color: bool) -> Cell {
    // `✓ Name ×N` — `×N` reads as "N occurrences", disambiguating this
    // row's frequency-count idiom from L2's existence-count idiom
    // (`36 hooks`). Label-only, dropped from the right under width pressure.
    let check = colorize("\u{2713}", &p.completed_check, color);
    let name = colorize(&c.name, &p.completed_check, color);
    let count = colorize(&format!(" \u{00D7}{}", c.count), &p.secondary, color);
    let head = format!("{check} {name}{count}");
    // Visible width: ✓ + space + name + " ×" + digits
    let head_w = 1 + 1 + c.name.chars().count() + 2 + count_digits(c.count as u64);
    Cell::label(head, head_w, CellPriority::Optional)
}

/// Build cells for the agent activity segment, one per group.
///
/// Pipeline: `classify(agents)` → bucket into `Single` / `Homogeneous` /
/// `Heterogeneous` parallel groups via shared `message_id` → produce
/// one `Cell` per group via the appropriate cell builder. Cells inherit
/// the group kind's body (description, parallel summary, etc.).
///
/// Ordering: active groups (any running agent) come first, then
/// completed. Within each tier the most recent group (tail of insertion
/// order) is preferred. Cell priority encodes the same rule for the
/// budgeter: active groups → `Required`, completed → `Optional` so a
/// still-running task is never dropped to surface a finished one.
///
/// Parsed `agents_visual` spec — selects which optional pieces appear in
/// the cell. The agent name (type or first description line) is always
/// rendered; this struct only gates the opt-in pieces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AgentVisualSpec {
    pub show_description: bool,
    pub show_model: bool,
}

impl AgentVisualSpec {
    /// Parse a `+`-joined spec like `"name+description+model"`. Unknown
    /// atoms are ignored (forward-compat). `name` is treated as a no-op
    /// because the name is always rendered.
    pub fn parse(spec: &str) -> Self {
        let mut s = Self::default();
        for atom in spec.split('+').map(str::trim).filter(|a| !a.is_empty()) {
            match atom {
                "description" => s.show_description = true,
                "model" => s.show_model = true,
                _ => {}
            }
        }
        s
    }
}

/// Caller decides packing: flat-row puts each cell on its own row;
/// cluster layouts pack them into one row via `pack_with_separator`.
pub fn build_agent_cells(
    agents: &[AgentSummary],
    config: &RenderConfig,
    p: &ThemePalette,
) -> Vec<Cell> {
    let groups = classify(agents);
    let spec = AgentVisualSpec::parse(config.effective_agents_visual());
    let mut active: Vec<Cell> = Vec::new();
    let mut completed: Vec<Cell> = Vec::new();
    for group in &groups {
        let is_active = group_has_active(group);
        let mut cell = match group {
            AgentGroup::Single(a) => build_agent_single_cell(a, config, p, spec),
            AgentGroup::Homogeneous(g) => build_agent_homogeneous_cell(g, config, p, spec),
            AgentGroup::Heterogeneous(g) => build_agent_heterogeneous_cell(g, config, p, spec),
        };
        cell.priority = if is_active {
            CellPriority::Required
        } else {
            CellPriority::Optional
        };
        if is_active {
            active.push(cell);
        } else {
            completed.push(cell);
        }
    }
    active.append(&mut completed);
    active
}

fn build_agent_rows(
    agents: &[AgentSummary],
    config: &RenderConfig,
    p: &ThemePalette,
    available: usize,
    sep: &str,
) -> Vec<String> {
    let cells = build_agent_cells(agents, config, p);
    let max_lines = config.max_agent_lines.max(1);
    let mut rows: Vec<String> = Vec::with_capacity(cells.len().min(max_lines + 1));

    // Cells come back as `[active.., completed..]`, both tiers in
    // insertion order. Under the cap: keep the newest `max_lines` of
    // active first, then fill any remaining slots with the newest
    // completed. Active that doesn't fit IS dropped (rare — would mean
    // the user has more parallel running agents than `max_agent_lines`).
    let active_count = cells
        .iter()
        .position(|c| c.priority == CellPriority::Optional)
        .unwrap_or(cells.len());
    let active = &cells[..active_count];
    let completed = &cells[active_count..];

    let active_keep = active.len().min(max_lines);
    let active_skip = active.len() - active_keep;
    let remaining = max_lines - active_keep;
    let completed_keep = completed.len().min(remaining);
    let completed_skip = completed.len() - completed_keep;
    let dropped = (active.len() - active_keep) + (completed.len() - completed_keep);

    if dropped > 0 {
        // Overflow summary sits at the TOP so the rendered rows below it
        // read as "newest activity, with K older items hidden above".
        rows.push(overflow_summary(
            dropped,
            "agent",
            "agents",
            p,
            config.color_enabled,
        ));
    }

    let chosen = active
        .iter()
        .skip(active_skip)
        .chain(completed.iter().skip(completed_skip));
    for cell in chosen {
        let row = pack_with_separator(
            std::slice::from_ref(cell),
            available,
            sep,
            ROW_SEPARATOR_W,
            config.color_enabled,
        );
        if !row.is_empty() {
            rows.push(row);
        }
    }

    rows
}

fn group_has_active(g: &AgentGroup<'_>) -> bool {
    match g {
        AgentGroup::Single(a) => !a.is_completed(),
        AgentGroup::Homogeneous(group) | AgentGroup::Heterogeneous(group) => {
            group.iter().any(|a| !a.is_completed())
        }
    }
}

fn build_agent_single_cell(
    a: &AgentSummary,
    config: &RenderConfig,
    p: &ThemePalette,
    spec: AgentVisualSpec,
) -> Cell {
    let color = config.color_enabled;
    let mode = config.glyph_mode;
    let completed = a.is_completed();

    let prefix_glyph = if completed {
        match mode {
            GlyphMode::Icon => format!("{ICON_AGENT_DONE} "),
            GlyphMode::Ascii => "A:".to_string(),
        }
    } else {
        glyph(mode, ICON_AGENT, "A:")
    };
    let accent: &str = if completed {
        p.completed_check.as_str()
    } else {
        p.agent_purple()
    };
    let prefix = colorize(&prefix_glyph, accent, color);

    let raw_desc = first_desc_line(a).to_string();
    // When the user hides description AND the agent has no `agent_type`,
    // the description becomes the name (otherwise the cell would show
    // just an icon). Pre-promote it into the head as the display name.
    let name_in_head: Option<String> = match (&a.agent_type, spec.show_description) {
        (Some(_), _) | (None, true) => None,
        (None, false) if !raw_desc.is_empty() => Some(raw_desc.clone()),
        (None, false) => None,
    };

    let (head, head_w) = if let Some(t) = &a.agent_type {
        let trailing_colon = spec.show_description && !raw_desc.is_empty();
        let type_str = colorize(t, accent, color);
        let head = if trailing_colon {
            let colon = colorize(": ", accent, color);
            format!("{prefix}{type_str}{colon}")
        } else {
            format!("{prefix}{type_str}")
        };
        let head_w =
            visible_width(&prefix_glyph) + t.chars().count() + if trailing_colon { 2 } else { 0 };
        (head, head_w)
    } else if let Some(name) = name_in_head.as_ref() {
        let name_str = colorize(name, accent, color);
        (
            format!("{prefix}{name_str}"),
            visible_width(&prefix_glyph) + name.chars().count(),
        )
    } else {
        (prefix.clone(), visible_width(&prefix_glyph))
    };

    let body = if spec.show_description && a.agent_type.is_some() && !raw_desc.is_empty() {
        Some(CellBody {
            raw: raw_desc,
            truncator: TruncationStrategy::Sentence,
            min_width: 12,
            ideal_width: 80,
            color: p.secondary.clone(),
        })
    } else if spec.show_description && a.agent_type.is_none() && !raw_desc.is_empty() {
        // No type → description is BOTH the name and the body. Keep the
        // accent colour so it reads as the name not a body annotation.
        Some(CellBody {
            raw: raw_desc,
            truncator: TruncationStrategy::Sentence,
            min_width: 12,
            ideal_width: 80,
            color: accent.to_string(),
        })
    } else {
        None
    };

    let mut tail: Vec<TailFragment> = Vec::new();
    if spec.show_model {
        tail.extend(model_slack_tail(&a.model, p, color));
    }
    if completed && mode == GlyphMode::Ascii {
        tail.push(TailFragment::Slack {
            text: colorize(" [done]", &p.structural, color),
            width: 7,
        });
    }
    let elapsed_str = elapsed_for(a);
    if !elapsed_str.is_empty() {
        tail.push(parens_pinned_tail(&elapsed_str, p, color));
    }

    Cell {
        head,
        head_w,
        body,
        tail,
        priority: CellPriority::Required,
    }
}

fn build_agent_homogeneous_cell(
    group: &[&AgentSummary],
    config: &RenderConfig,
    p: &ThemePalette,
    spec: AgentVisualSpec,
) -> Cell {
    let color = config.color_enabled;
    let mode = config.glyph_mode;
    let n = group.len();
    let agent_type = group[0].agent_type.as_deref().unwrap_or("agent");
    let all_completed = group.iter().all(|a| a.is_completed());

    let descriptions: Vec<String> = group
        .iter()
        .map(|a| first_desc_line(a).to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let has_body = spec.show_description && !descriptions.is_empty();

    let prefix_glyph = if all_completed {
        match mode {
            GlyphMode::Icon => format!("{ICON_AGENT_DONE} "),
            GlyphMode::Ascii => "A:".to_string(),
        }
    } else {
        glyph(mode, ICON_AGENT, "A:")
    };
    let accent: &str = if all_completed {
        p.completed_check.as_str()
    } else {
        p.agent_purple()
    };
    let prefix = colorize(&prefix_glyph, accent, color);
    let type_str = colorize(agent_type, accent, color);
    let count_str = colorize(&format!(" \u{00D7}{n}"), accent, color);
    let bracket_open_raw = if has_body { " [" } else { "" };
    let bracket_open = colorize(bracket_open_raw, &p.structural, color);
    let head = format!("{prefix}{type_str}{count_str}{bracket_open}");
    let head_w = visible_width(&prefix_glyph)
        + agent_type.chars().count()
        + 2
        + count_digits(n as u64)
        + bracket_open_raw.chars().count();

    let body = if has_body {
        Some(CellBody {
            raw: descriptions.join(GROUP_SUBITEM_SEPARATOR),
            truncator: TruncationStrategy::Sentence,
            min_width: 16,
            ideal_width: 100,
            color: p.secondary.clone(),
        })
    } else {
        None
    };

    let mut tail: Vec<TailFragment> = Vec::new();
    if has_body {
        tail.push(TailFragment::Pinned {
            text: colorize("]", &p.structural, color),
            width: 1,
        });
    }
    if spec.show_model {
        tail.extend(model_slack_tail(&group[0].model, p, color));
    }
    if let Some(avg_ms) = avg_elapsed_ms(group) {
        let avg = format_agent_elapsed(avg_ms / 1000);
        tail.push(parens_pinned_tail(&format!("avg {avg}"), p, color));
    }

    Cell {
        head,
        head_w,
        body,
        tail,
        priority: CellPriority::Required,
    }
}

fn build_agent_heterogeneous_cell(
    group: &[&AgentSummary],
    config: &RenderConfig,
    p: &ThemePalette,
    spec: AgentVisualSpec,
) -> Cell {
    let color = config.color_enabled;
    let mode = config.glyph_mode;
    let n = group.len();
    // See homogeneous cell: a fully-completed batch flips to the done
    // glyph + completed_check accent so the row no longer looks running.
    let all_completed = group.iter().all(|a| a.is_completed());

    let prefix_glyph = if all_completed {
        match mode {
            GlyphMode::Icon => format!("{ICON_AGENT_DONE} "),
            GlyphMode::Ascii => "||".to_string(),
        }
    } else {
        glyph(mode, ICON_GROUP_PARALLEL.0, ICON_GROUP_PARALLEL.1)
    };
    let accent: &str = if all_completed {
        p.completed_check.as_str()
    } else {
        p.agent_purple()
    };
    let prefix = colorize(&prefix_glyph, accent, color);
    let count_str = colorize(&format!("\u{00D7}{n}"), accent, color);
    let parallel_lbl = colorize(" parallel", &p.structural, color);
    let avg_str = avg_elapsed_ms(group)
        .map(|ms| format_agent_elapsed(ms / 1000))
        .unwrap_or_default();
    let avg_part = if avg_str.is_empty() {
        String::new()
    } else {
        format!(" (avg {avg_str})")
    };
    let avg_colored = colorize(&avg_part, &p.structural, color);
    let head = format!("{prefix}{count_str}{parallel_lbl}{avg_colored}: ");
    let head_w = visible_width(&prefix_glyph)
        + 1 + count_digits(n as u64)
        + 9                                                  // " parallel"
        + avg_part.chars().count()
        + 2; // ": "

    // Body: bucket by `agent_type`. With description shown, type-runs
    // collapse to `Type ×N [a + b]`; without, they collapse to `Type ×N`
    // so the user still sees which types are running in parallel.
    let sub_items: Vec<String> = bucket_by_type(group)
        .into_iter()
        .map(|(t, descs)| match (spec.show_description, descs.len()) {
            (true, 1) => format!("{t}: {}", descs[0]),
            (true, n) => format!("{t} \u{00D7}{n} [{}]", descs.join(GROUP_SUBITEM_SEPARATOR)),
            (false, 1) => t.to_string(),
            (false, n) => format!("{t} \u{00D7}{n}"),
        })
        .collect();
    let body_raw = sub_items.join(GROUP_SUBITEM_SEPARATOR);
    let body = Some(CellBody {
        raw: body_raw,
        truncator: TruncationStrategy::Sentence,
        min_width: 24,
        ideal_width: 240,
        color: p.secondary.clone(),
    });

    Cell {
        head,
        head_w,
        body,
        tail: vec![],
        priority: CellPriority::Required,
    }
}

fn build_todo_rows(
    todo: &TodoSummary,
    config: &RenderConfig,
    p: &ThemePalette,
    available: usize,
) -> Vec<String> {
    let color = config.color_enabled;
    let mode = config.glyph_mode;
    let mut rows: Vec<String> = Vec::new();
    let sep = colorize(ROW_SEPARATOR, &p.separator, color);

    // All-done celebration line.
    if todo.all_done {
        let check = colorize("\u{2713}", &p.completed_check, color);
        let text = colorize(" All todos complete", &p.completed_check, color);
        let count = colorize(
            &format!(" ({}/{})", todo.completed, todo.total),
            &p.secondary,
            color,
        );
        rows.push(format!("{check}{text}{count}"));
        return rows;
    }

    // Task-API in-progress items (one row each, capped).
    if todo.is_task_api && !todo.in_progress_items.is_empty() {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        for item in todo
            .in_progress_items
            .iter()
            .take(config.max_todo_lines.max(1))
        {
            let cell = build_todo_inprogress_cell(item, todo, now_ms, mode, p, color);
            let row = pack_with_separator(&[cell], available, &sep, ROW_SEPARATOR_W, color);
            if !row.is_empty() {
                rows.push(row);
            }
        }
        return rows;
    }

    // Pending-only summary (task API, no in-progress items).
    if todo.is_task_api && todo.total > 0 {
        let prefix = colorize(&glyph(mode, ICON_TODO, "TODO:"), p.todo_teal(), color);
        let body = colorize(&format!(" {} tasks", todo.total), p.todo_teal(), color);
        let count = colorize(
            &format!(" ({}/{})", todo.completed, todo.total),
            &p.secondary,
            color,
        );
        rows.push(format!("{prefix}{body}{count}"));
        return rows;
    }

    // Legacy TodoWrite path — single line of raw text.
    if !todo.text.is_empty() {
        let prefix = colorize(&glyph(mode, ICON_TODO, "TODO:"), p.todo_teal(), color);
        let text = colorize(&todo.text, p.todo_teal(), color);
        rows.push(format!("{prefix}{text}"));
    }

    rows
}

fn build_todo_inprogress_cell(
    item: &crate::types::TodoInProgressItem,
    todo: &TodoSummary,
    now_ms: u64,
    mode: GlyphMode,
    p: &ThemePalette,
    color: bool,
) -> Cell {
    let prefix_glyph = glyph(mode, ICON_TODO, "TODO:");
    let prefix = colorize(&prefix_glyph, p.todo_teal(), color);
    let head = prefix.clone();
    let head_w = visible_width(&prefix_glyph);

    let body = Some(CellBody {
        raw: item.text.clone(),
        truncator: TruncationStrategy::Sentence,
        min_width: 12,
        ideal_width: 80,
        color: p.todo_teal().to_string(),
    });

    let mut tail: Vec<TailFragment> = Vec::new();
    let active_count = todo.in_progress_items.len();
    let count_str = if active_count > 1 {
        format!(
            " ({}/{}, {} active)",
            todo.completed, todo.total, active_count
        )
    } else {
        format!(" ({}/{})", todo.completed, todo.total)
    };
    let count_w = count_str.chars().count();
    tail.push(TailFragment::Pinned {
        text: colorize(&count_str, &p.secondary, color),
        width: count_w,
    });

    if let Some(start) = item.started_at {
        let secs = now_ms.saturating_sub(start) / 1000;
        let elapsed = format_agent_elapsed(secs);
        let txt = format!(" ({elapsed})");
        let w = txt.chars().count();
        tail.push(TailFragment::Slack {
            text: colorize(&txt, &p.structural, color),
            width: w,
        });
    }

    Cell {
        head,
        head_w,
        body,
        tail,
        priority: CellPriority::Required,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn elapsed_for(a: &AgentSummary) -> String {
    if a.is_completed() {
        match (a.started_at, a.completed_at) {
            (Some(start), Some(end)) => {
                let secs = end.saturating_sub(start) / 1000;
                format_agent_elapsed(secs)
            }
            _ => String::new(),
        }
    } else if let Some(start_ms) = a.started_at {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        format_agent_elapsed(now_ms.saturating_sub(start_ms) / 1000)
    } else {
        String::new()
    }
}

fn count_digits(n: u64) -> usize {
    n.checked_ilog10().map_or(1, |x| x as usize + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GlyphMode;
    use crate::render::color::resolve_palette;
    use crate::types::ToolSummary;

    fn palette() -> ThemePalette {
        resolve_palette("tokyo-night", Some("dark"), &Default::default())
    }

    fn cfg() -> RenderConfig {
        RenderConfig {
            color_enabled: false,
            glyph_mode: GlyphMode::Ascii,
            palette: palette(),
            show_tools: true,
            show_agents: true,
            show_todo: true,
            ..RenderConfig::default()
        }
    }

    fn agent(id: &str, msg: Option<&str>, ty: Option<&str>, desc: &str) -> AgentSummary {
        AgentSummary {
            id: id.to_string(),
            description: desc.to_string(),
            agent_type: ty.map(String::from),
            started_at: Some(1_000),
            model: None,
            completed_at: Some(61_000),
            message_id: msg.map(String::from),
        }
    }

    #[test]
    fn empty_frame_yields_no_rows() {
        let frame = RenderFrame::default();
        let rows = build_activity_rows(&frame, &cfg(), &palette(), 200);
        assert!(rows.is_empty());
    }

    #[test]
    fn completed_tools_pack_into_one_row_when_wide() {
        let frame = RenderFrame {
            completed_tools: vec![
                CompletedToolCount {
                    name: "Bash".to_string(),
                    count: 163,
                    last_completed_at: None,
                },
                CompletedToolCount {
                    name: "Edit".to_string(),
                    count: 95,
                    last_completed_at: None,
                },
                CompletedToolCount {
                    name: "Read".to_string(),
                    count: 86,
                    last_completed_at: None,
                },
            ],
            ..Default::default()
        };
        let c = RenderConfig {
            max_completed_tools: 10,
            max_completed_lines: 2,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 200);
        assert_eq!(rows.len(), 1, "expected 1 packed row, got {rows:?}");
        let row = &rows[0];
        assert!(row.contains("Bash") && row.contains("Edit") && row.contains("Read"));
        // Layout-standard ` | ` separator (vocabulary consistency w/ L1/L2/L3).
        assert!(
            row.contains(" | "),
            "row must use layout-standard separator: {row:?}"
        );
        // `×` is the frequency disambiguator — without it, `Name N` would
        // collide with L2's `count noun` idiom.
        assert!(row.contains('\u{00D7}'), "row must keep × glyph: {row:?}");
    }

    #[test]
    fn completed_tools_wrap_to_second_row_when_narrow() {
        // Width-adaptive: 6 cells at width 50 should split across 2 rows
        // (each cell ~12 visible chars + 3-char separator). No overflow
        // summary because everything fits within the 2-row cap.
        let frame = RenderFrame {
            completed_tools: (0..6)
                .map(|i| CompletedToolCount {
                    name: format!("Tool{i}"),
                    count: 100 + i as u32,
                    last_completed_at: None,
                })
                .collect(),
            ..Default::default()
        };
        let c = RenderConfig {
            max_completed_tools: 10,
            max_completed_lines: 2,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 50);
        assert!(
            (1..=c.max_completed_lines).contains(&rows.len()),
            "expected 1–{} rows at width 50, got {}: {rows:?}",
            c.max_completed_lines,
            rows.len()
        );
        // No overflow line — all 6 should fit within 2 rows.
        assert!(
            !rows.iter().any(|r| r.contains("more tool")),
            "no overflow summary expected: {rows:?}"
        );
        for i in 0..6 {
            let needle = format!("Tool{i}");
            assert!(
                rows.iter().any(|r| r.contains(&needle)),
                "missing {needle}: {rows:?}"
            );
        }
    }

    #[test]
    fn completed_tools_overflow_summary_at_bottom() {
        // 12 cells at width 50: 2 rows fit ~10 cells, the rest become
        // `… + N more tools` at the BOTTOM (importance-ordered semantics —
        // hidden items are the least-important, so they live below).
        let frame = RenderFrame {
            completed_tools: (0..12)
                .map(|i| CompletedToolCount {
                    name: format!("Tool{i:02}"),
                    count: 1000 - i as u32,
                    last_completed_at: None,
                })
                .collect(),
            ..Default::default()
        };
        let c = RenderConfig {
            max_completed_tools: 20,
            max_completed_lines: 2,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 50);
        // Exactly `max_completed_lines` packed rows + 1 overflow summary line.
        assert_eq!(
            rows.len(),
            c.max_completed_lines + 1,
            "expected {} rows + summary, got {rows:?}",
            c.max_completed_lines
        );
        let last = rows.last().unwrap();
        assert!(
            last.contains("more tool"),
            "bottom row must be the overflow summary: {last:?}"
        );
        assert!(
            last.starts_with('\u{2026}'),
            "summary must lead with `…`: {last:?}"
        );
        // Most-important (Tool00) must be in row 0; least-important (Tool11)
        // must be hidden in the summary.
        assert!(
            rows[0].contains("Tool00"),
            "Tool00 missing from top: {rows:?}"
        );
        assert!(
            !rows[0].contains("Tool11") && !rows[1].contains("Tool11"),
            "least-important Tool11 should be omitted: {rows:?}"
        );
    }

    #[test]
    fn completed_tools_no_orphan_under_max_completed_lines() {
        // 5 tool counts fit on a single packed row at typical widths.
        let frame = RenderFrame {
            completed_tools: vec![
                CompletedToolCount {
                    name: "Bash".to_string(),
                    count: 251,
                    last_completed_at: None,
                },
                CompletedToolCount {
                    name: "Edit".to_string(),
                    count: 193,
                    last_completed_at: None,
                },
                CompletedToolCount {
                    name: "Read".to_string(),
                    count: 131,
                    last_completed_at: None,
                },
                CompletedToolCount {
                    name: "Write".to_string(),
                    count: 16,
                    last_completed_at: None,
                },
                CompletedToolCount {
                    name: "Skill".to_string(),
                    count: 1,
                    last_completed_at: None,
                },
            ],
            ..Default::default()
        };
        let c = RenderConfig {
            max_completed_tools: 10,
            max_completed_lines: 2,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 200);
        assert_eq!(
            rows.len(),
            1,
            "must pack into ONE row regardless of cap: {rows:?}"
        );
        for name in ["Bash", "Edit", "Read", "Write", "Skill"] {
            assert!(rows[0].contains(name), "row missing {name}: {:?}", rows[0]);
        }
    }

    #[test]
    fn recent_tool_with_bash_target_keeps_verb() {
        let frame = RenderFrame {
            tools: vec![ToolSummary {
                id: "t1".to_string(),
                name: "Bash".to_string(),
                target: Some(
                    "sed -i '' 's/^name = \".*\"$/name = \"cards\"/' .claude/pulseline.toml"
                        .to_string(),
                ),
            }],
            ..Default::default()
        };
        let c = RenderConfig {
            max_tool_lines: 1,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 80);
        let row = &rows[0];
        // The verb (`sed`) anchors the cell so the operator can recognise the
        // command at a glance; later tokens may be truncated with `…`.
        assert!(
            row.contains("T:Bash: sed"),
            "verb must be at row start: {row:?}"
        );
    }

    #[test]
    fn single_agent_renders_as_one_row() {
        let frame = RenderFrame {
            agents: vec![agent("a1", None, Some("Explore"), "investigate auth flow")],
            ..Default::default()
        };
        let rows = build_activity_rows(&frame, &cfg(), &palette(), 200);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("Explore"));
        assert!(rows[0].contains("investigate"));
    }

    #[test]
    fn homogeneous_batch_uses_done_glyph_when_all_completed() {
        let frame = RenderFrame {
            agents: vec![
                agent("a1", Some("msg_X"), Some("general-purpose"), "review 1"),
                agent("a2", Some("msg_X"), Some("general-purpose"), "review 2"),
            ],
            ..Default::default()
        };
        let c = RenderConfig {
            glyph_mode: GlyphMode::Icon,
            max_agent_lines: 5,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 200);
        assert!(
            rows[0].contains(ICON_AGENT_DONE),
            "all-completed batch must use done glyph: {:?}",
            rows[0]
        );
        assert!(
            !rows[0].contains(ICON_AGENT),
            "all-completed batch must NOT use running glyph: {:?}",
            rows[0]
        );
    }

    #[test]
    fn homogeneous_batch_uses_running_glyph_when_partial() {
        let mut a1 = agent("a1", Some("msg_X"), Some("general-purpose"), "review 1");
        a1.completed_at = None; // still running
        let frame = RenderFrame {
            agents: vec![
                a1,
                agent("a2", Some("msg_X"), Some("general-purpose"), "review 2"),
            ],
            ..Default::default()
        };
        let c = RenderConfig {
            glyph_mode: GlyphMode::Icon,
            max_agent_lines: 5,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 200);
        assert!(
            rows[0].contains(ICON_AGENT),
            "partial batch must keep running glyph: {:?}",
            rows[0]
        );
    }

    #[test]
    fn homogeneous_batch_uses_bracketed_body() {
        let frame = RenderFrame {
            agents: vec![
                agent(
                    "a1",
                    Some("msg_X"),
                    Some("general-purpose"),
                    "Code reuse review",
                ),
                agent(
                    "a2",
                    Some("msg_X"),
                    Some("general-purpose"),
                    "Code quality review",
                ),
                agent(
                    "a3",
                    Some("msg_X"),
                    Some("general-purpose"),
                    "Efficiency review",
                ),
            ],
            ..Default::default()
        };
        let c = RenderConfig {
            max_agent_lines: 5,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 200);
        assert_eq!(
            rows.len(),
            1,
            "batch should collapse to one row, got {rows:?}"
        );
        assert!(
            rows[0].contains("\u{00D7}3"),
            "should show ×3 count: {:?}",
            rows[0]
        );
        // Bracket pair marks the type-bucket; the literal word "parallel"
        // is gone because `×3 [...]` already conveys the batch shape.
        assert!(
            rows[0].contains(" [") && rows[0].contains("]"),
            "body must be enclosed in brackets: {:?}",
            rows[0]
        );
        assert!(
            !rows[0].contains("parallel"),
            "redundant `parallel` label must be dropped: {:?}",
            rows[0]
        );
        assert!(
            !rows[0].contains("+ 2 more"),
            "summary phrase must be removed: {:?}",
            rows[0]
        );
        for desc in [
            "Code reuse review",
            "Code quality review",
            "Efficiency review",
        ] {
            assert!(
                rows[0].contains(desc),
                "missing description {desc:?}: {:?}",
                rows[0]
            );
        }
        assert!(
            rows[0].contains(" + "),
            "descriptions joined by ` + `: {:?}",
            rows[0]
        );
    }

    #[test]
    fn heterogeneous_group_uses_pipe_glyph_and_plus_separator() {
        let frame = RenderFrame {
            agents: vec![
                agent("a1", Some("msg_X"), Some("Explore"), "investigate auth"),
                agent("a2", Some("msg_X"), Some("general-purpose"), "code reuse"),
                agent("a3", Some("msg_X"), Some("code-reviewer"), "final pass"),
            ],
            ..Default::default()
        };
        let c = RenderConfig {
            max_agent_lines: 5,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 240);
        assert_eq!(rows.len(), 1);
        // ASCII fallback: `||` instead of `‖`
        assert!(
            rows[0].contains("||"),
            "should use group prefix: {:?}",
            rows[0]
        );
        assert!(rows[0].contains("\u{00D7}3"));
        assert!(
            rows[0].contains(" + "),
            "should join sub-items with ` + `: {:?}",
            rows[0]
        );
        assert!(rows[0].contains("Explore"));
        assert!(rows[0].contains("code-reviewer"));
        // All types unique → no bucketing → no `[a + b]` brackets.
        assert!(
            !rows[0].contains('['),
            "single-member buckets must not use brackets: {:?}",
            rows[0]
        );
    }

    #[test]
    fn heterogeneous_group_buckets_repeated_types() {
        // Mixed types with a repeat: 2× Explore + 1× code-reviewer should
        // render as `Explore ×2 [a + b] + code-reviewer: c`. The repeated
        // type prefix is collapsed; the unique type stays in flat form.
        let frame = RenderFrame {
            agents: vec![
                agent("a1", Some("msg_X"), Some("Explore"), "investigate auth"),
                agent("a2", Some("msg_X"), Some("Explore"), "parse JWT"),
                agent("a3", Some("msg_X"), Some("code-reviewer"), "final pass"),
            ],
            ..Default::default()
        };
        let c = RenderConfig {
            max_agent_lines: 5,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 240);
        assert_eq!(rows.len(), 1);
        // Repeated type collapses into a bucket: `Explore ×2 [a + b]`.
        assert!(
            rows[0].contains("Explore \u{00D7}2 ["),
            "expected `Explore ×2 […]` bucket: {:?}",
            rows[0]
        );
        assert!(
            rows[0].contains("investigate auth") && rows[0].contains("parse JWT"),
            "both Explore descriptions must surface: {:?}",
            rows[0]
        );
        // Unique type stays flat — no bracket, no `×N`.
        assert!(
            rows[0].contains("code-reviewer: final pass"),
            "single-member type must use flat `Type: desc`: {:?}",
            rows[0]
        );
        // Type prefix `Explore:` (with colon) must NOT appear — that's the
        // anti-pattern bucketing exists to remove.
        assert!(
            !rows[0].contains("Explore: "),
            "bucketed type must not also appear in flat form: {:?}",
            rows[0]
        );
    }

    #[test]
    fn sequential_overflow_emits_summary_line() {
        // 5 sequential agents (different message_id), max_agent_lines=2 → expect
        // 1 overflow summary (at top) + 2 full rows (most recent two).
        let frame = RenderFrame {
            agents: (0..5)
                .map(|i| {
                    agent(
                        &format!("a{i}"),
                        Some(&format!("msg_{i}")),
                        Some("general-purpose"),
                        &format!("review {i}"),
                    )
                })
                .collect(),
            ..Default::default()
        };
        let c = RenderConfig {
            max_agent_lines: 2,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 240);
        assert_eq!(rows.len(), 3, "expected 1 summary + 2 rows, got {rows:?}");
        assert!(
            rows[0].contains("3 more agents"),
            "summary must come first (older items hidden above): {:?}",
            rows[0]
        );
    }

    #[test]
    fn active_groups_outrank_completed_when_max_lines_forces_a_choice() {
        // 2 active + 3 completed, max_agent_lines = 2 → both active shown,
        // ALL completed dropped (because active fills the cap). Pins the
        // priority rule from `build_agent_rows`.
        let mut active1 = agent("a1", None, Some("Indexer"), "still indexing");
        active1.completed_at = None;
        let mut active2 = agent("a2", None, Some("Reviewer"), "still reviewing");
        active2.completed_at = None;
        let frame = RenderFrame {
            agents: vec![
                agent("c1", None, Some("Old"), "first finished"),
                active1,
                agent("c2", None, Some("Old"), "second finished"),
                active2,
                agent("c3", None, Some("Old"), "third finished"),
            ],
            ..Default::default()
        };
        let c = RenderConfig {
            max_agent_lines: 2,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 240);
        let blob = rows.join("\n");
        assert!(blob.contains("Indexer"), "active 'Indexer' missing: {blob}");
        assert!(
            blob.contains("Reviewer"),
            "active 'Reviewer' missing: {blob}"
        );
        for done_desc in ["first finished", "second finished", "third finished"] {
            assert!(
                !blob.contains(done_desc),
                "completed '{done_desc}' should be dropped when active fills the cap: {blob}"
            );
        }
        // Overflow summary still appears for the 3 hidden completed groups.
        assert!(
            blob.contains("3 more agents"),
            "overflow summary missing: {blob}"
        );
    }

    #[test]
    fn todo_all_done_celebration_line() {
        let frame = RenderFrame {
            todo: Some(TodoSummary {
                text: String::new(),
                pending: 0,
                completed: 6,
                total: 6,
                in_progress_items: vec![],
                all_done: true,
                is_task_api: true,
            }),
            ..Default::default()
        };
        let rows = build_activity_rows(&frame, &cfg(), &palette(), 200);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("All todos complete"));
        assert!(rows[0].contains("(6/6)"));
    }

    #[test]
    fn missing_message_id_renders_each_agent_singly() {
        // Safe degradation for legacy cache files: agents with `message_id = None`
        // never group, even when type matches.
        let frame = RenderFrame {
            agents: vec![
                agent("a1", None, Some("general-purpose"), "first"),
                agent("a2", None, Some("general-purpose"), "second"),
            ],
            ..Default::default()
        };
        let c = RenderConfig {
            max_agent_lines: 5,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 240);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].contains("parallel"));
        assert!(!rows[1].contains("parallel"));
    }
}
