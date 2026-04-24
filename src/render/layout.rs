use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    config::{RenderConfig, WidthDegradeStrategy},
    types::{AgentSummary, Line1Metrics, Line3Metrics, QuotaMetrics, RenderFrame, TodoSummary},
};

use super::color::{colorize, take_visible_chars, visible_width, ThemePalette, RESET};
use super::fmt::{
    format_agent_elapsed, format_duration, format_number, format_reset_duration, format_speed,
};
use super::icons::*;

/// Number of core lines (L1 identity, L2 config, L3 budget) that are always rendered.
/// Used in width degradation to determine what counts as "activity" lines.
const CORE_LINE_COUNT: usize = 3;

pub fn render_frame(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let color = config.color_enabled;
    let p = &config.palette;

    let mut lines = vec![
        format_line1(frame, config, p),
        format_line2(frame, config, " | ", p),
        format_line3(frame, config, p),
    ];

    // Quota line: between L3 and activity lines
    if config.show_quota {
        if let Some(line) = format_quota_line(&frame.quota, config, p) {
            lines.push(line);
        }
    }

    // Tool lines: completed counts (stable, multi-line) then recent tools (volatile)
    if config.show_tools {
        if !frame.completed_tools.is_empty() {
            lines.extend(format_completed_tool_lines(frame, config, p));
        }
        if !frame.tools.is_empty() {
            lines.push(format_recent_tool_line(frame, config, p));
        }
    }

    // Agent lines: one per agent, conditional
    if config.show_agents {
        for agent in frame.agents.iter().take(config.max_agent_lines) {
            lines.push(format_agent_line(agent, config, p));
        }
    }

    // Todo lines: conditional
    if config.show_todo {
        if let Some(todo) = &frame.todo {
            lines.extend(format_todo_lines(todo, config, p));
        }
    }

    if let Some(width) = config.terminal_width {
        let compressed_line2 = format_line2(frame, config, " ", p);
        lines =
            apply_width_degradation(lines, width, &config.degrade_order, compressed_line2, color);
    }

    lines
}

/// Format completed tool counts across multiple lines.
fn format_completed_tool_lines(
    frame: &RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Vec<String> {
    let color = config.color_enabled;
    let sep = colorize(" | ", &p.separator, color);
    let per_line = config.tools_per_line.max(1);

    frame
        .completed_tools
        .chunks(per_line)
        .map(|chunk| {
            let parts: Vec<String> = chunk
                .iter()
                .map(|completed| {
                    let check = colorize("✓", &p.completed_check, color);
                    let name_str = colorize(&completed.name, &p.completed_check, color);
                    let count_str =
                        colorize(&format!(" ×{}", completed.count), &p.secondary, color);
                    format!("{check} {name_str}{count_str}")
                })
                .collect();
            parts.join(&sep)
        })
        .collect()
}

/// Format the recent/running tools line with targets.
fn format_recent_tool_line(frame: &RenderFrame, config: &RenderConfig, p: &ThemePalette) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let sep = colorize(" | ", &p.separator, color);

    let parts: Vec<String> = frame
        .tools
        .iter()
        .take(config.max_tool_lines)
        .map(|tool| {
            let prefix = colorize(&glyph(mode, ICON_TOOL, "T:"), p.tool_blue(), color);
            let name_str = colorize(&tool.name, p.tool_blue(), color);
            if let Some(target) = &tool.target {
                let target_str = colorize(&format!(": {target}"), &p.secondary, color);
                format!("{prefix}{name_str}{target_str}")
            } else {
                format!("{prefix}{name_str}")
            }
        })
        .collect();

    parts.join(&sep)
}

/// Max visible chars for activity line text (agent descriptions, todo task text).
const ACTIVITY_TEXT_MAX_CHARS: usize = 40;

/// Truncate text to `max_chars`, appending ellipsis if needed.
fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() > max_chars {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{truncated}…")
    } else {
        text.to_string()
    }
}

/// Format a parenthesized progress count: ` (N/M)`.
fn format_progress_count(completed: usize, total: usize, p: &ThemePalette, color: bool) -> String {
    let open = colorize(" (", &p.separator, color);
    let counts = colorize(&format!("{completed}/{total}"), &p.secondary, color);
    let close = colorize(")", &p.separator, color);
    format!("{open}{counts}{close}")
}

/// Format todo display lines, capped by `config.max_todo_lines`.
fn format_todo_lines(todo: &TodoSummary, config: &RenderConfig, p: &ThemePalette) -> Vec<String> {
    let mode = config.glyph_mode;
    let color = config.color_enabled;

    // All done: celebration line
    if todo.all_done {
        let check = colorize("✓", &p.completed_check, color);
        let text = colorize(" All todos complete", &p.completed_check, color);
        let progress = format_progress_count(todo.completed, todo.total, p, color);
        return vec![format!("{check}{text}{progress}")];
    }

    // Task API path with in-progress items
    if todo.is_task_api && !todo.in_progress_items.is_empty() {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let total_active = todo.in_progress_items.len();
        let mut lines = Vec::new();

        for (idx, item) in todo
            .in_progress_items
            .iter()
            .take(config.max_todo_lines)
            .enumerate()
        {
            let prefix = colorize(&glyph(mode, ICON_TODO, "TODO:"), p.todo_teal(), color);

            let text_str = colorize(
                &truncate_text(&item.text, ACTIVITY_TEXT_MAX_CHARS),
                p.todo_teal(),
                color,
            );

            // Elapsed time
            let elapsed_part = item
                .started_at
                .map(|start_ms| {
                    let secs = now_ms.saturating_sub(start_ms) / 1000;
                    let open = colorize(" (", &p.separator, color);
                    let time = colorize(&format_agent_elapsed(secs), &p.structural, color);
                    let close = colorize(")", &p.separator, color);
                    format!("{open}{time}{close}")
                })
                .unwrap_or_default();

            if idx == 0 {
                // First line: includes progress indicator (completed/total)
                let open = colorize(" (", &p.separator, color);
                let progress = colorize(
                    &format!("{}/{}", todo.completed, todo.total),
                    &p.secondary,
                    color,
                );
                let shown = total_active.min(config.max_todo_lines);
                let overflow_part = if total_active > shown {
                    colorize(&format!(", {} active", total_active), &p.secondary, color)
                } else {
                    String::new()
                };
                let close = colorize(")", &p.separator, color);
                lines.push(format!(
                    "{prefix}{text_str}{open}{progress}{overflow_part}{close}{elapsed_part}"
                ));
            } else {
                // Subsequent lines: just task text + elapsed
                lines.push(format!("{prefix}{text_str}{elapsed_part}"));
            }
        }

        return lines;
    }

    // Task API path with pending only (no in-progress items)
    if todo.is_task_api {
        let prefix = colorize(&glyph(mode, ICON_TODO, "TODO:"), p.todo_teal(), color);
        let label = colorize(&format!("{} tasks", todo.total), p.todo_teal(), color);
        let progress = format_progress_count(todo.completed, todo.total, p, color);
        return vec![format!("{prefix}{label}{progress}")];
    }

    // Legacy fallback (TodoWrite path)
    let prefix = colorize(&glyph(mode, ICON_TODO, "TODO:"), p.todo_teal(), color);
    let text = colorize(&todo.text, p.todo_teal(), color);
    vec![format!("{prefix}{text}")]
}

/// Format a single agent line.
fn format_agent_line(agent: &AgentSummary, config: &RenderConfig, p: &ThemePalette) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let completed = agent.is_completed();

    // Prefix: running vs completed
    let prefix = if completed {
        match mode {
            crate::config::GlyphMode::Icon => {
                colorize(&format!("{} ", ICON_AGENT_DONE), &p.completed_check, color)
            }
            crate::config::GlyphMode::Ascii => colorize("A:", &p.completed_check, color),
        }
    } else {
        colorize(&glyph(mode, ICON_AGENT, "A:"), p.agent_purple(), color)
    };

    // Truncate description: first line only, max ACTIVITY_TEXT_MAX_CHARS visible chars
    let first_line = agent.description.lines().next().unwrap_or("");
    let desc_truncated = truncate_text(first_line, ACTIVITY_TEXT_MAX_CHARS);

    // Elapsed time
    let elapsed_str = if completed {
        match (agent.started_at, agent.completed_at) {
            (Some(start), Some(end)) => {
                let secs = end.saturating_sub(start) / 1000;
                format_agent_elapsed(secs)
            }
            _ => String::new(),
        }
    } else {
        agent
            .started_at
            .map(|start_ms| {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let secs = now_ms.saturating_sub(start_ms) / 1000;
                format_agent_elapsed(secs)
            })
            .unwrap_or_default()
    };

    // Model tag: [haiku] in structural color
    let model_part = agent
        .model
        .as_ref()
        .map(|m| colorize(&format!(" [{m}]"), &p.structural, color))
        .unwrap_or_default();

    // Done tag for ASCII completed agents
    let done_tag = if completed && mode == crate::config::GlyphMode::Ascii {
        colorize(" [done]", &p.structural, color)
    } else {
        String::new()
    };

    let elapsed_part = if elapsed_str.is_empty() {
        String::new()
    } else {
        let open = colorize(" (", &p.separator, color);
        let time = colorize(&elapsed_str, &p.structural, color);
        let close = colorize(")", &p.separator, color);
        format!("{open}{time}{close}")
    };

    let accent_color = if completed {
        &p.completed_check
    } else {
        p.agent_purple()
    };

    if let Some(agent_type) = &agent.agent_type {
        let type_str = colorize(&agent_type.to_string(), accent_color, color);
        let colon = colorize(": ", accent_color, color);
        let desc_str = colorize(&desc_truncated, &p.secondary, color);
        format!("{prefix}{type_str}{model_part}{colon}{desc_str}{done_tag}{elapsed_part}")
    } else {
        let desc_str = colorize(&desc_truncated, accent_color, color);
        format!("{prefix}{desc_str}{model_part}{done_tag}{elapsed_part}")
    }
}

fn format_line1(frame: &RenderFrame, config: &RenderConfig, p: &ThemePalette) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let sep = colorize(" | ", &p.separator, color);

    let mut parts: Vec<String> = Vec::new();

    if config.show_model {
        let model_label = colorize(&glyph(mode, ICON_MODEL, "M:"), &p.stable_blue, color);
        let model_val = colorize(&frame.line1.model, &p.stable_blue, color);
        parts.push(format!("{model_label}{model_val}"));
    }

    if config.show_effort {
        if let Some(level) = &frame.line1.effort_level {
            let effort_color = p.color_for_effort_level(level);
            let label = colorize(&glyph(mode, ICON_EFFORT, "E:"), effort_color, color);
            let val = colorize(level, effort_color, color);
            parts.push(format!("{label}{val}"));
        }
    }

    if config.show_thinking && frame.line1.thinking_enabled == Some(true) {
        // Label-only pill — no value; `enabled: false` or missing → omitted entirely.
        // Trim before colorizing so the ANSI reset stays tight against the glyph.
        let raw = glyph(mode, ICON_THINKING, "[T]");
        let label = colorize(raw.trim_end(), &p.active_purple, color);
        parts.push(label);
    }

    if config.show_agent {
        if let Some(agent_name) = &frame.line1.agent_name {
            let label = colorize(&glyph(mode, ICON_AGENT, "AG:"), &p.stable_blue, color);
            let val = colorize(agent_name, &p.stable_blue, color);
            parts.push(format!("{label}{val}"));
        }
    }

    if config.show_style {
        let style_label = colorize(&glyph(mode, ICON_STYLE, "S:"), &p.secondary, color);
        let style_val = colorize(&frame.line1.output_style, &p.secondary, color);
        parts.push(format!("{style_label}{style_val}"));
    }

    if config.show_version {
        let version_label = colorize(&glyph(mode, ICON_VERSION, "CC:"), &p.secondary, color);
        let version_val = colorize(&frame.line1.claude_code_version, &p.secondary, color);
        parts.push(format!("{version_label}{version_val}"));
    }

    if config.show_project {
        let project_label = colorize(&glyph(mode, ICON_PROJECT, "P:"), &p.secondary, color);
        let project_val = colorize(&frame.line1.project_path, &p.secondary, color);
        parts.push(format!("{project_label}{project_val}"));
    }

    if config.show_git {
        let git_label = colorize(&glyph(mode, ICON_GIT, "G:"), p.git_green(), color);
        let git_val = format_git_status(&frame.line1, config, p);
        parts.push(format!("{git_label}{git_val}"));
    }

    parts.join(&sep)
}

fn format_line2(
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

    let mut parts: Vec<String> = Vec::new();

    if config.show_context {
        parts.push(format_context_segment(&frame.line3, config, p));
    }
    if config.show_tokens {
        let speed = if config.show_speed {
            frame.line3.output_speed_toks_per_sec
        } else {
            None
        };
        parts.push(format_tokens_segment(&frame.line3, speed, config, p));
    }
    if config.show_cost {
        parts.push(format_cost_segment(&frame.line3, config, p));
    }

    parts.join(&sep)
}

fn format_git_status(line1: &Line1Metrics, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;

    if line1.git_branch.is_empty() || line1.git_branch == "unknown" {
        let mut s = colorize("unknown", &p.structural, color);
        if config.show_worktree && line1.in_worktree {
            s.push_str(&colorize(" (WT)", &p.structural, color));
        }
        return s;
    }

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

fn format_context_segment(line3: &Line3Metrics, config: &RenderConfig, p: &ThemePalette) -> String {
    let color = config.color_enabled;
    let mode = config.glyph_mode;

    match (line3.context_used_percentage, line3.context_window_size) {
        (Some(used_pct), Some(size)) => {
            let pct_color = p.color_for_ctx_pct(used_pct, Some(size));

            let used_tokens = (size as f64 * used_pct as f64 / 100.0) as u64;

            let label = colorize(&glyph(mode, ICON_CONTEXT, "CTX:"), pct_color, color);
            let pct = colorize(&format!("{}%", used_pct), pct_color, color);
            let open_paren = colorize(" (", &p.separator, color);
            let usage = colorize(&format_number(used_tokens), &p.primary, color);
            let sep = colorize("/", &p.separator, color);
            let total = colorize(&format_number(size), &p.primary, color);
            let close_paren = colorize(")", &p.separator, color);

            format!("{label}{pct}{open_paren}{usage}{sep}{total}{close_paren}")
        }
        _ => {
            let label = colorize(&glyph(mode, ICON_CONTEXT, "CTX:"), &p.structural, color);
            let dash = colorize("--", &p.structural, color);
            let pct_sign = colorize("%", &p.structural, color);
            let open_paren = colorize(" (", &p.separator, color);
            let sep = colorize("/", &p.separator, color);
            let close_paren = colorize(")", &p.separator, color);
            format!("{label}{dash}{pct_sign}{open_paren}{dash}{sep}{dash}{close_paren}")
        }
    }
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
    let cache_str = if has_data {
        format!(
            "{}/{}",
            format_number(line3.cache_creation_tokens.unwrap_or(0)),
            format_number(line3.cache_read_tokens.unwrap_or(0)),
        )
    } else {
        "--/--".to_string()
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
            colorize(&cache_str, val_color, color),
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
            let pct_color = if pct_val >= 85.0 {
                p.ctx_critical()
            } else if pct_val >= 50.0 {
                p.ctx_warn()
            } else {
                p.ctx_good()
            };

            let pct_str = colorize(&format!("{pct_val:.0}%"), pct_color, color);
            let label_str = colorize(&format!("{label}:"), &p.secondary, color);

            let reset_part = reset_minutes
                .map(|m| {
                    let duration = format_reset_duration(m);
                    let open = colorize(" (", &p.separator, color);
                    let txt = colorize(&format!("resets {duration}"), &p.structural, color);
                    let close = colorize(")", &p.separator, color);
                    format!("{open}{txt}{close}")
                })
                .unwrap_or_default();

            if pct_val >= 100.0 {
                let limit_text = colorize("Limit reached", p.ctx_critical(), color);
                format!("{label_str} {limit_text}{reset_part}")
            } else {
                format!("{label_str} {pct_str}{reset_part}")
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
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    if lines_fit_width(&lines, width) {
        return lines;
    }

    for strategy in strategies {
        if lines_fit_width(&lines, width) {
            break;
        }

        match strategy {
            WidthDegradeStrategy::DropActivityLinesFirst => {
                if lines.len() > CORE_LINE_COUNT {
                    lines.truncate(CORE_LINE_COUNT);
                }
            }
            WidthDegradeStrategy::CompressLine2 => {
                if let Some(line2) = lines.get_mut(1) {
                    *line2 = compressed_line2.clone();
                }
            }
            WidthDegradeStrategy::CompressCoreLines => {
                for index in 0..lines.len().min(CORE_LINE_COUNT) {
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

fn truncate_to_width(line: &str, width: usize, color_enabled: bool) -> String {
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
