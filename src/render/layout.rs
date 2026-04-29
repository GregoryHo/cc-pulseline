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
    // CC allocates the statusline a sub-region narrower than the raw terminal
    // (see DEFAULT_PANE_CC_MARGIN). Both v1 and v2 paths must size against the
    // sub-region, not the raw terminal — otherwise CC's overflow detection
    // wraps the over-wide line and collapses the whole multi-line render to
    // one visible line.
    let adjusted = config.terminal_width.map(|w| {
        let mut c = config.clone();
        c.terminal_width = Some(w.saturating_sub(config.pane_cc_margin));
        c
    });
    let config: &RenderConfig = adjusted.as_ref().unwrap_or(config);

    // Instrument-cluster layouts own the full pipeline; flat-row layouts fall
    // through to the v1-style assembly below.
    let palette = &config.palette;
    match config.pane_style {
        LayoutStyle::Cockpit => return frames::cockpit::render(frame, config, palette),
        LayoutStyle::Flightstrip => return frames::flightstrip::render(frame, config, palette),
        LayoutStyle::Auto => return frames::auto::render(frame, config, palette),
        LayoutStyle::Ledger => return frames::ledger::render(frame, config, palette),
        LayoutStyle::None
        | LayoutStyle::Zones
        | LayoutStyle::Grid
        | LayoutStyle::Cards
        | LayoutStyle::Sections
        // Console flows through the flat path now; `apply_pane` routes
        // it to `sections::render_with_options(_, _, _, _, Some(title))`
        // which hoists Identity into the top frame border.
        | LayoutStyle::Console => {}
    }

    let color = config.color_enabled;
    let base_palette = &config.palette;
    let tonal = config.pane_tonal_strata;

    // Two strata tiers, not four: state rows (Identity/Config/Budget/Quota)
    // share `p_state`; activity rows (Tools/Agents/Todos) get `p_activity`.
    let p_state = tinted_palette(base_palette, false, tonal);
    let p_activity = tinted_palette(base_palette, true, tonal);

    let mut lines: Vec<String> = Vec::new();
    let mut groups: Vec<(LineKind, Range<usize>)> = Vec::new();

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
    lines.push(identity_str);
    groups.push((LineKind::Identity, start..lines.len()));

    let start = lines.len();
    lines.push(format_line2(frame, config, " | ", &p_state));
    groups.push((LineKind::Config, start..lines.len()));

    let start = lines.len();
    lines.push(format_line3(frame, config, &p_state));
    if config.show_quota {
        if let Some(line) = format_quota_line(&frame.quota, config, &p_state) {
            lines.push(line);
        }
    }
    groups.push((LineKind::Budget, start..lines.len()));

    let activity_start = lines.len();
    let activity_width = config.terminal_width.unwrap_or(usize::MAX);
    lines.extend(crate::render::activity::build_activity_rows(
        frame,
        config,
        &p_activity,
        activity_width,
    ));
    if lines.len() > activity_start {
        groups.push((LineKind::Activity, activity_start..lines.len()));
    }

    // Deduct the active style's horizontal overhead from the degradation
    // budget so content + decoration together fit the terminal.
    let pane_active = !matches!(config.pane_style, LayoutStyle::None);
    let style_overhead = match config.pane_style {
        LayoutStyle::None | LayoutStyle::Zones => 0,
        // Grid consumes `label_width + 2` cols on the left. The default group
        // labels (Identity/Config/Budget/Activity) top out at 8 chars + 2 pad
        // + 2 for " │ " = ~12 cols. Budget value for width degradation.
        LayoutStyle::Grid => 12,
        // Cards / Sections / Console use a wall-on-both-sides layout
        // (`│ ` left + internal ` │ ` divider + ` │` right) — ~4 more cols
        // than Grid.
        LayoutStyle::Cards | LayoutStyle::Sections | LayoutStyle::Console => 16,
        // Instrument-cluster + ledger styles never reach this branch — they
        // returned above.
        LayoutStyle::Cockpit
        | LayoutStyle::Flightstrip
        | LayoutStyle::Auto
        | LayoutStyle::Ledger => 0,
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
        );
        // `apply_width_degradation` may have truncated `lines`; clamp every
        // group range so downstream renderers (cards/sections) don't push
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

fn pane_config_from(config: &RenderConfig) -> PaneConfig {
    let default_groups = vec![
        PaneGroup {
            label: "Identity".to_string(),
            kinds: vec![LineKind::Identity],
        },
        PaneGroup {
            label: "Config".to_string(),
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
        width_mode: config.pane_width_mode,
        min_width: config.pane_min_width,
        max_width: config.pane_max_width,
        groups: default_groups,
        glyph_mode: config.glyph_mode,
        terminal_width: config.terminal_width,
        cc_margin: config.pane_cc_margin,
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
        // Cluster sparkline consumers still take `&[u8]`; ledger and the new
        // sparkline widget will take the timestamped tuple slice directly
        // after the upcoming sparkline-signature change. For now adapt at
        // the call site — the cluster path dies in the next-but-one commit.
        let pcts: Vec<u8> = frame.ctx_history.iter().map(|(p, _)| *p).collect();
        parts.push(format_context_segment(&frame.line3, config, p, &pcts));
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

/// Sizing hint for the gauge widget when a flat layout dispatches a v1 L3
/// row through `render_context_visual`. Roughly matches cockpit's
/// `FULL_GAUGE_WIDTH` so `cards + context_visual = "gauge"` reads at the
/// same density as `cockpit + gauge`.
const V1_GAUGE_WIDTH: usize = 18;

fn format_context_segment(
    line3: &Line3Metrics,
    config: &RenderConfig,
    p: &ThemePalette,
    history: &[u8],
) -> String {
    // Dispatches through the visual hub so flat layouts inherit the same
    // composability as instrument-cluster layouts. Default for flat layouts
    // is `text` — produces the legacy `CTX:43% (86.0k/200.0k)` form
    // unchanged. Users who set `context_visual = "gauge"` on a flat layout
    // get a gauge inside e.g. a Cards frame ("cards + gauge", the headline
    // composability proof).
    frames::shared::render_context_visual(
        config.effective_context_visual(),
        line3,
        history,
        V1_GAUGE_WIDTH,
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
            let pct_color = p.color_for_quota_pct(pct_val);

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
    activity_start: usize,
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
                if let Some(line2) = lines.get_mut(1) {
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
