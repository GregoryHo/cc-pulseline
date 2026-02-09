use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    config::{RenderConfig, WidthDegradeStrategy},
    types::{AgentSummary, Line1Metrics, Line3Metrics, RenderFrame},
};

use super::color::{
    colorize, emphasis_for_theme, take_visible_chars, visible_width, EmphasisTier, AGENT_PURPLE,
    COMPLETED_CHECK, COST_BASE, COST_HIGH_RATE, COST_LOW_RATE, COST_MED_RATE, CTX_CRITICAL,
    CTX_GOOD, CTX_WARN, GIT_AHEAD, GIT_BEHIND, GIT_GREEN, GIT_MODIFIED, INDICATOR_CLAUDE_MD,
    INDICATOR_DURATION, INDICATOR_HOOKS, INDICATOR_MCP, INDICATOR_RULES, INDICATOR_SKILLS, RESET,
    STABLE_BLUE, TODO_TEAL, TOOL_BLUE,
};
use super::fmt::{format_duration, format_number};
use super::icons::*;

pub fn render_frame(frame: &RenderFrame, config: &RenderConfig) -> Vec<String> {
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let tier = emphasis_for_theme(config.color_theme);

    let mut lines = vec![
        format_line1(frame, config, &tier),
        format_line2(frame, config, " | ", &tier),
        format_line3(frame, config, &tier),
    ];

    // Tool line: running + completed on a single line (claude-hud style)
    if config.show_tools && (!frame.tools.is_empty() || !frame.completed_tools.is_empty()) {
        lines.push(format_tool_line(frame, config, &tier));
    }

    // Agent lines: one per agent, conditional
    // Format: {icon} {agent_type}: {truncated_desc} ({elapsed})
    if config.show_agents {
        for agent in frame.agents.iter().take(config.max_agent_lines) {
            lines.push(format_agent_line(agent, config, &tier));
        }
    }

    // Todo line: conditional
    if config.show_todo {
        if let Some(todo) = &frame.todo {
            let prefix = colorize(&glyph(mode, ICON_TODO, "TODO:"), TODO_TEAL, color);
            let text = colorize(&todo.text, TODO_TEAL, color);
            lines.push(format!("{prefix}{text}"));
        }
    }

    if let Some(width) = config.terminal_width {
        let compressed_line2 = format_line2(frame, config, " ", &tier);
        lines =
            apply_width_degradation(lines, width, &config.degrade_order, compressed_line2, color);
    }

    lines
}

/// Format a single tool line combining running tools and completed counts.
/// Running:   `T:Read: .../main.rs | T:Bash: cargo test`
/// Completed: `✓Read ×5 | ✓Bash ×3`
fn format_tool_line(frame: &RenderFrame, config: &RenderConfig, tier: &EmphasisTier) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let sep = colorize(" | ", tier.separator, color);

    let mut parts: Vec<String> = Vec::new();

    // Running tools (most recent N)
    for tool in frame.tools.iter().take(config.max_tool_lines) {
        let prefix = colorize(&glyph(mode, ICON_TOOL, "T:"), TOOL_BLUE, color);
        let name_str = colorize(&tool.name, TOOL_BLUE, color);
        if let Some(target) = &tool.target {
            let target_str = colorize(&format!(": {target}"), tier.secondary, color);
            parts.push(format!("{prefix}{name_str}{target_str}"));
        } else {
            parts.push(format!("{prefix}{name_str}"));
        }
    }

    // Completed tool counts (top N by frequency)
    for completed in &frame.completed_tools {
        let check = colorize("✓", COMPLETED_CHECK, color);
        let name_str = colorize(&completed.name, COMPLETED_CHECK, color);
        let count_str = colorize(&format!(" ×{}", completed.count), tier.secondary, color);
        parts.push(format!("{check} {name_str}{count_str}"));
    }

    parts.join(&sep)
}

/// Format a single agent line.
/// With agent_type: `A:Explore: Investigate logic (2m)`
/// Without:         `A:Investigate logic (2m)`
///
/// The description field comes from the Task tool's `description` (3-5 word short summary)
/// when available, falling back to `prompt` (full text). We truncate to first line,
/// max 40 chars to keep activity lines compact.
const AGENT_DESC_MAX_CHARS: usize = 40;

fn format_agent_line(agent: &AgentSummary, config: &RenderConfig, tier: &EmphasisTier) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;

    let prefix = colorize(&glyph(mode, ICON_AGENT, "A:"), AGENT_PURPLE, color);

    // Truncate description: first line only, max AGENT_DESC_MAX_CHARS visible chars
    let first_line = agent.description.lines().next().unwrap_or("");
    let desc_truncated = if first_line.chars().count() > AGENT_DESC_MAX_CHARS {
        let truncated: String = first_line.chars().take(AGENT_DESC_MAX_CHARS).collect();
        format!("{truncated}…")
    } else {
        first_line.to_string()
    };

    // Elapsed time since agent started (epoch-based, survives across process invocations)
    let elapsed_str = agent
        .started_at
        .map(|start_ms| {
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let secs = now_ms.saturating_sub(start_ms) / 1000;
            if secs < 60 {
                format!("{}s", secs)
            } else {
                format!("{}m", secs / 60)
            }
        })
        .unwrap_or_default();

    let elapsed_part = if elapsed_str.is_empty() {
        String::new()
    } else {
        let open = colorize(" (", tier.separator, color);
        let time = colorize(&elapsed_str, tier.structural, color);
        let close = colorize(")", tier.separator, color);
        format!("{open}{time}{close}")
    };

    if let Some(agent_type) = &agent.agent_type {
        let type_str = colorize(&format!("{agent_type}: "), AGENT_PURPLE, color);
        let desc_str = colorize(&desc_truncated, tier.secondary, color);
        format!("{prefix}{type_str}{desc_str}{elapsed_part}")
    } else {
        let desc_str = colorize(&desc_truncated, AGENT_PURPLE, color);
        format!("{prefix}{desc_str}{elapsed_part}")
    }
}

fn format_line1(frame: &RenderFrame, config: &RenderConfig, tier: &EmphasisTier) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let sep = colorize(" | ", tier.separator, color);

    let mut parts: Vec<String> = Vec::new();

    if config.show_model {
        let model_label = colorize(&glyph(mode, ICON_MODEL, "M:"), STABLE_BLUE, color);
        let model_val = colorize(&frame.line1.model, STABLE_BLUE, color);
        parts.push(format!("{model_label}{model_val}"));
    }

    if config.show_style {
        let style_label = colorize(&glyph(mode, ICON_STYLE, "S:"), tier.secondary, color);
        let style_val = colorize(&frame.line1.output_style, tier.secondary, color);
        parts.push(format!("{style_label}{style_val}"));
    }

    if config.show_version {
        let version_label = colorize(&glyph(mode, ICON_VERSION, "CC:"), tier.secondary, color);
        let version_val = colorize(&frame.line1.claude_code_version, tier.secondary, color);
        parts.push(format!("{version_label}{version_val}"));
    }

    if config.show_project {
        let project_label = colorize(&glyph(mode, ICON_PROJECT, "P:"), tier.secondary, color);
        let project_val = colorize(&frame.line1.project_path, tier.secondary, color);
        parts.push(format!("{project_label}{project_val}"));
    }

    if config.show_git {
        let git_label = colorize(&glyph(mode, ICON_GIT, "G:"), GIT_GREEN, color);
        let git_val = format_git_status(&frame.line1, config);
        parts.push(format!("{git_label}{git_val}"));
    }

    parts.join(&sep)
}

fn format_line2(
    frame: &RenderFrame,
    config: &RenderConfig,
    separator: &str,
    tier: &EmphasisTier,
) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;
    let sep = colorize(separator, tier.separator, color);

    // Helper to format: {icon} {count} {label} or {count} {label}
    // Icon uses per-metric indicator_color; count uses tier.secondary; label uses tier.structural
    let format_item =
        |icon: &str, indicator_color: &str, label: &str, count: u32| -> String {
            let count_str = colorize(&count.to_string(), tier.primary, color);
            let label_str = colorize(label, tier.structural, color);

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
            INDICATOR_CLAUDE_MD,
            "CLAUDE.md",
            frame.line2.claude_md_count,
        ));
    }
    if config.show_rules {
        parts.push(format_item(
            ICON_RULES,
            INDICATOR_RULES,
            "rules",
            frame.line2.rules_count,
        ));
    }
    if config.show_hooks {
        parts.push(format_item(
            ICON_HOOKS,
            INDICATOR_HOOKS,
            "hooks",
            frame.line2.hooks_count,
        ));
    }
    if config.show_mcp {
        parts.push(format_item(
            ICON_MCP,
            INDICATOR_MCP,
            "MCPs",
            frame.line2.mcp_count,
        ));
    }
    if config.show_skills {
        parts.push(format_item(
            ICON_SKILLS,
            INDICATOR_SKILLS,
            "skills",
            frame.line2.skills_count,
        ));
    }
    if config.show_duration {
        let duration_text = format_duration(frame.line2.elapsed_minutes);
        let item = match mode {
            crate::config::GlyphMode::Icon => {
                let icon_str =
                    colorize(&format!("{} ", ICON_ELAPSED), INDICATOR_DURATION, color);
                let time_str = colorize(&duration_text, tier.primary, color);
                format!("{icon_str}{time_str}")
            }
            crate::config::GlyphMode::Ascii => colorize(&duration_text, tier.primary, color),
        };
        parts.push(item);
    }

    parts.join(&sep)
}

fn format_line3(frame: &RenderFrame, config: &RenderConfig, tier: &EmphasisTier) -> String {
    let color = config.color_enabled;
    let sep = colorize(" | ", tier.separator, color);

    let mut parts: Vec<String> = Vec::new();

    if config.show_context {
        parts.push(format_context_segment(&frame.line3, config, tier));
    }
    if config.show_tokens {
        parts.push(format_tokens_segment(&frame.line3, config, tier));
    }
    if config.show_cost {
        parts.push(format_cost_segment(&frame.line3, config, tier));
    }

    parts.join(&sep)
}

fn format_git_status(line1: &Line1Metrics, config: &RenderConfig) -> String {
    let color = config.color_enabled;

    if line1.git_branch.is_empty() || line1.git_branch == "unknown" {
        return "unknown".to_string();
    }

    let mut status = colorize(&line1.git_branch, GIT_GREEN, color);
    if line1.git_dirty {
        status.push_str(&colorize("*", GIT_MODIFIED, color));
    }
    if line1.git_ahead > 0 {
        status.push_str(&colorize(
            &format!(" ↑{}", line1.git_ahead),
            GIT_AHEAD,
            color,
        ));
    }
    if line1.git_behind > 0 {
        status.push_str(&colorize(
            &format!(" ↓{}", line1.git_behind),
            GIT_BEHIND,
            color,
        ));
    }

    status
}

fn format_context_segment(
    line3: &Line3Metrics,
    config: &RenderConfig,
    tier: &EmphasisTier,
) -> String {
    let color = config.color_enabled;
    let mode = config.glyph_mode;

    match (line3.context_used_percentage, line3.context_window_size) {
        (Some(used_pct), Some(size)) => {
            let pct_color = if used_pct >= 85 {
                CTX_CRITICAL
            } else if used_pct >= 70 {
                CTX_WARN
            } else {
                CTX_GOOD
            };

            let used_tokens = (size as f64 * used_pct as f64 / 100.0) as u64;

            let label = colorize(&glyph(mode, ICON_CONTEXT, "CTX:"), pct_color, color);
            let pct = colorize(&format!("{}%", used_pct), pct_color, color);
            let open_paren = colorize(" (", tier.separator, color);
            let usage = colorize(&format_number(used_tokens), tier.primary, color);
            let sep = colorize("/", tier.separator, color);
            let total = colorize(&format_number(size), tier.primary, color);
            let close_paren = colorize(")", tier.separator, color);

            format!("{label}{pct}{open_paren}{usage}{sep}{total}{close_paren}")
        }
        _ => {
            let label = colorize(&glyph(mode, ICON_CONTEXT, "CTX:"), tier.secondary, color);
            format!("{label}NA")
        }
    }
}

fn format_tokens_segment(
    line3: &Line3Metrics,
    config: &RenderConfig,
    tier: &EmphasisTier,
) -> String {
    let mode = config.glyph_mode;
    let color = config.color_enabled;

    if line3.input_tokens.is_none()
        && line3.output_tokens.is_none()
        && line3.cache_creation_tokens.is_none()
        && line3.cache_read_tokens.is_none()
    {
        let label = colorize(&glyph(mode, ICON_TOKENS, "TOK "), tier.structural, color);
        return format!("{label}NA");
    }

    let label = colorize("TOK ", tier.structural, color);
    let cache_str = format!(
        "{}/{}",
        format_number(line3.cache_creation_tokens.unwrap_or(0)),
        format_number(line3.cache_read_tokens.unwrap_or(0)),
    );
    let parts = [
        format!(
            "{}{}",
            colorize(&glyph(mode, ICON_TOKEN_INPUT, "I: "), tier.structural, color),
            colorize(
                &format_number(line3.input_tokens.unwrap_or(0)),
                tier.primary,
                color,
            ),
        ),
        format!(
            "{}{}",
            colorize(&glyph(mode, ICON_TOKEN_OUTPUT, "O: "), tier.structural, color),
            colorize(
                &format_number(line3.output_tokens.unwrap_or(0)),
                tier.primary,
                color,
            ),
        ),
        format!(
            "{}{}",
            colorize(&glyph(mode, ICON_TOKEN_CACHE_CREATE, "C:"), tier.structural, color),
            colorize(&cache_str, tier.primary, color),
        ),
    ];
    format!("{label}{}", parts.join(" "))
}

fn format_cost_segment(
    line3: &Line3Metrics,
    config: &RenderConfig,
    tier: &EmphasisTier,
) -> String {
    let color = config.color_enabled;

    let total_cost = line3.total_cost_usd.unwrap_or(0.0);
    let per_hour = line3
        .total_duration_ms
        .filter(|duration| *duration > 0)
        .map(|duration| total_cost / ((duration as f64) / 3_600_000.0))
        .unwrap_or(0.0);

    let rate_color = if per_hour > 50.0 {
        COST_HIGH_RATE
    } else if per_hour > 10.0 {
        COST_MED_RATE
    } else {
        COST_LOW_RATE
    };

    let total_str = colorize(&format!("${total_cost:.2}"), COST_BASE, color);
    let open_paren = colorize("(", tier.separator, color);
    let rate_str = colorize(&format!("${per_hour:.2}/h"), rate_color, color);
    let close_paren = colorize(")", tier.separator, color);
    format!("{total_str} {open_paren}{rate_str}{close_paren}")
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
                if lines.len() > 3 {
                    lines.truncate(3);
                }
            }
            WidthDegradeStrategy::CompressLine2 => {
                if let Some(line2) = lines.get_mut(1) {
                    *line2 = compressed_line2.clone();
                }
            }
            WidthDegradeStrategy::CompressCoreLines => {
                for index in 0..lines.len().min(3) {
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
