//! Build `Cell`s from activity-frame data and pack them into row strings.
//!
//! Public entry point: `build_activity_rows(frame, config, palette, available_width)`.
//! See `designs/activity-width-budget.md` §4 for cell descriptors.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{GlyphMode, RenderConfig};
use crate::render::color::{colorize, visible_width, ThemePalette};
use crate::render::fmt::format_agent_elapsed;
use crate::render::icons::{
    glyph, ICON_AGENT, ICON_AGENT_DONE, ICON_GROUP_PARALLEL, ICON_TODO, ICON_TOOL,
};
use crate::types::{AgentSummary, CompletedToolCount, RenderFrame, TodoSummary, ToolSummary};

use super::agent_groups::{avg_elapsed_ms, classify, AgentGroup};
use super::budget::pack_with_separator;
use super::cell::{Cell, CellBody, CellPriority, TailFragment};
use super::truncate::TruncationStrategy;

const ROW_SEPARATOR: &str = " | ";
const ROW_SEPARATOR_W: usize = 3;
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
            rows.extend(pack_chunked(
                &cells,
                available_width,
                &sep,
                ROW_SEPARATOR_W,
                config.tools_per_line,
                color,
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

/// Per-tool truncation strategy + ideal target width. Single source of
/// truth that replaced the per-tool `truncate_str(_, N)` constants
/// scattered across `providers/transcript.rs::extract_target`.
pub fn target_strategy_for(tool_name: &str) -> (TruncationStrategy, usize) {
    use TruncationStrategy::*;
    match tool_name {
        "Read" | "Write" | "Edit" | "NotebookEdit" => (KeepTail, 40),
        "Bash" | "PowerShell" => (CommandSmart, 50),
        "Glob" | "Grep" => (KeepHead, 30),
        "WebFetch" => (KeepMiddle, 40),
        "WebSearch" | "Skill" | "Advisor" | "MCPSearch" | "AskUserQuestion" => (Sentence, 50),
        "SendMessage" | "LSP" | "Monitor" | "PushNotification" => (KeepHead, 30),
        _ => (KeepHead, 30),
    }
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

fn build_completed_tool_cell(c: &CompletedToolCount, p: &ThemePalette, color: bool) -> Cell {
    // `✓ Name ×N` — label-only, dropped from the right under width pressure.
    let check = colorize("\u{2713}", &p.completed_check, color);
    let name = colorize(&c.name, &p.completed_check, color);
    let count = colorize(&format!(" \u{00D7}{}", c.count), &p.secondary, color);
    let head = format!("{check} {name}{count}");
    // Visible width: ✓ + space + name + " ×" + digits
    let head_w = 1 + 1 + c.name.chars().count() + 2 + count_digits(c.count as u64);
    Cell::label(head, head_w, CellPriority::Optional)
}

fn build_recent_tool_cell(t: &ToolSummary, mode: GlyphMode, p: &ThemePalette, color: bool) -> Cell {
    let prefix_glyph = glyph(mode, ICON_TOOL, "T:");
    let prefix = colorize(&prefix_glyph, p.tool_blue(), color);
    let name = colorize(&t.name, p.tool_blue(), color);
    let head = match &t.target {
        Some(_) => format!("{prefix}{name}: "),
        None => format!("{prefix}{name}"),
    };
    let head_w = visible_width(&prefix_glyph)
        + t.name.chars().count()
        + if t.target.is_some() { 2 } else { 0 };
    let body = t.target.as_ref().map(|raw| {
        let (truncator, ideal) = target_strategy_for(&t.name);
        CellBody {
            raw: raw.clone(),
            truncator,
            min_width: 8,
            ideal_width: ideal,
            color: p.secondary.clone(),
        }
    });
    Cell {
        head,
        head_w,
        body,
        tail: vec![],
        priority: CellPriority::Required,
    }
}

fn build_agent_rows(
    agents: &[AgentSummary],
    config: &RenderConfig,
    p: &ThemePalette,
    available: usize,
    sep: &str,
) -> Vec<String> {
    let groups = classify(agents);
    let max_lines = config.max_agent_lines.max(1);
    let mut rows: Vec<String> = Vec::with_capacity(groups.len().min(max_lines + 1));

    // Picking which groups to render under width pressure follows two rules:
    //   1. Groups with any running agent ("active") win priority over fully-
    //      completed groups — a still-running task should never be hidden by
    //      finished history.
    //   2. Within each tier, prefer the most recent (tail of insertion order).
    // The overflow summary, when emitted, sits at the TOP so the rendered
    // rows below it read as "newest activity, with K older items hidden above".
    let active_groups: Vec<&AgentGroup> = groups.iter().filter(|g| group_has_active(g)).collect();
    let completed_groups: Vec<&AgentGroup> =
        groups.iter().filter(|g| !group_has_active(g)).collect();

    let active_skip = active_groups.len().saturating_sub(max_lines);
    let mut chosen: Vec<&AgentGroup> = active_groups.iter().copied().skip(active_skip).collect();
    let remaining = max_lines.saturating_sub(chosen.len());
    if remaining > 0 {
        let completed_skip = completed_groups.len().saturating_sub(remaining);
        chosen.extend(completed_groups.iter().copied().skip(completed_skip));
    }

    let dropped = groups.len().saturating_sub(chosen.len());
    if dropped > 0 {
        rows.push(colorize(
            &format!("\u{2026} + {dropped} more agents"),
            &p.structural,
            config.color_enabled,
        ));
    }

    for group in chosen {
        let cell = match group {
            AgentGroup::Single(a) => build_agent_single_cell(a, config, p),
            AgentGroup::Homogeneous(g) => build_agent_homogeneous_cell(g, config, p),
            AgentGroup::Heterogeneous(g) => build_agent_heterogeneous_cell(g, config, p),
        };
        let row = pack_with_separator(
            &[cell],
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

fn build_agent_single_cell(a: &AgentSummary, config: &RenderConfig, p: &ThemePalette) -> Cell {
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

    let head = match &a.agent_type {
        Some(t) => {
            let type_str = colorize(t, accent, color);
            let colon = colorize(": ", accent, color);
            format!("{prefix}{type_str}{colon}")
        }
        None => prefix.clone(),
    };
    let type_w = a
        .agent_type
        .as_ref()
        .map(|t| t.chars().count() + 2)
        .unwrap_or(0);
    let head_w = visible_width(&prefix_glyph) + type_w;

    let raw_desc = a.description.lines().next().unwrap_or("").to_string();
    let body = if raw_desc.is_empty() {
        None
    } else {
        let body_color = if a.agent_type.is_some() {
            p.secondary.clone()
        } else {
            accent.to_string()
        };
        Some(CellBody {
            raw: raw_desc,
            truncator: TruncationStrategy::Sentence,
            min_width: 12,
            ideal_width: 80,
            color: body_color,
        })
    };

    let mut tail: Vec<TailFragment> = Vec::new();
    tail.extend(model_slack_tail(&a.model, p, color));
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
) -> Cell {
    let color = config.color_enabled;
    let mode = config.glyph_mode;
    let n = group.len();
    let agent_type = group[0].agent_type.as_deref().unwrap_or("agent");

    let prefix_glyph = glyph(mode, ICON_AGENT, "A:");
    let prefix = colorize(&prefix_glyph, p.agent_purple(), color);
    let type_str = colorize(agent_type, p.agent_purple(), color);
    let count_str = colorize(&format!(" \u{00D7}{n}"), p.agent_purple(), color);
    let parallel_lbl = colorize(" parallel", &p.structural, color);
    let head = format!("{prefix}{type_str}{count_str}{parallel_lbl}: ");
    let head_w = visible_width(&prefix_glyph)
        + agent_type.chars().count()
        + 2 + count_digits(n as u64)         // " ×N"
        + 9                                    // " parallel"
        + 2; // ": "

    let first_desc = group[0]
        .description
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let body_raw = if n > 1 {
        format!("{first_desc} + {} more", n - 1)
    } else {
        first_desc
    };
    let body = if body_raw.is_empty() {
        None
    } else {
        Some(CellBody {
            raw: body_raw,
            truncator: TruncationStrategy::Sentence,
            min_width: 16,
            ideal_width: 100,
            color: p.secondary.clone(),
        })
    };

    let mut tail: Vec<TailFragment> = Vec::new();
    tail.extend(model_slack_tail(&group[0].model, p, color));
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
) -> Cell {
    let color = config.color_enabled;
    let mode = config.glyph_mode;
    let n = group.len();

    let prefix_glyph = glyph(mode, ICON_GROUP_PARALLEL.0, ICON_GROUP_PARALLEL.1);
    let prefix = colorize(&prefix_glyph, p.agent_purple(), color);
    let count_str = colorize(&format!("\u{00D7}{n}"), p.agent_purple(), color);
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

    // Body: per-agent `<type>: <first-line desc>` joined by ` + `.
    // We compose the body raw as plain text; truncator shortens the longest
    // sub-item via Sentence first.
    let sub_items: Vec<String> = group
        .iter()
        .map(|a| {
            let t = a.agent_type.as_deref().unwrap_or("agent");
            let d = a.description.lines().next().unwrap_or("");
            format!("{t}: {d}")
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

/// Pack cells into one or more rows, respecting an optional `cap` of cells
/// per row. When `cap == 0`, treat as no cap (all that fit in one row).
fn pack_chunked(
    cells: &[Cell],
    available: usize,
    sep: &str,
    sep_w: usize,
    cap: usize,
    color_enabled: bool,
) -> Vec<String> {
    if cells.is_empty() {
        return Vec::new();
    }
    let chunk = if cap == 0 { cells.len() } else { cap.max(1) };
    cells
        .chunks(chunk)
        .map(|slice| pack_with_separator(slice, available, sep, sep_w, color_enabled))
        .filter(|s| !s.is_empty())
        .collect()
}

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
    fn target_strategy_table_known_tools() {
        assert_eq!(target_strategy_for("Read").0, TruncationStrategy::KeepTail);
        assert_eq!(
            target_strategy_for("Bash").0,
            TruncationStrategy::CommandSmart
        );
        assert_eq!(target_strategy_for("Glob").0, TruncationStrategy::KeepHead);
        assert_eq!(
            target_strategy_for("WebFetch").0,
            TruncationStrategy::KeepMiddle
        );
        assert_eq!(
            target_strategy_for("UnknownTool").0,
            TruncationStrategy::KeepHead
        );
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
            tools_per_line: 0,
            ..cfg()
        };
        let rows = build_activity_rows(&frame, &c, &palette(), 200);
        assert_eq!(rows.len(), 1, "expected 1 packed row, got {rows:?}");
        let row = &rows[0];
        assert!(row.contains("Bash") && row.contains("Edit") && row.contains("Read"));
    }

    #[test]
    fn recent_tool_with_bash_target_uses_command_smart() {
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
        // CommandSmart should preserve the regex payload, not the verb 'sed'
        assert!(
            row.contains("s/") || row.contains("pulseline.toml"),
            "command_smart should surface payload: {row:?}"
        );
        assert!(!row.starts_with("T:Bash: sed -i"));
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
    fn homogeneous_batch_collapses_with_parallel_label() {
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
        assert!(
            rows[0].contains("parallel"),
            "should show parallel label: {:?}",
            rows[0]
        );
        assert!(
            rows[0].contains("+ 2 more"),
            "should show '+ N more': {:?}",
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
