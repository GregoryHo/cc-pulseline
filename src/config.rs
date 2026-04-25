use crate::render::color::{resolve_palette, ThemePalette};
use crate::render::pane::{PaneStyle, PaneWidth};
use serde::Deserialize;
use std::path::PathBuf;

// ── Pulseline Config (TOML file) ─────────────────────────────────────

fn default_true() -> bool {
    true
}
fn default_dark() -> String {
    "dark".to_string()
}
fn default_max_lines() -> usize {
    2
}
fn default_max_completed() -> usize {
    4
}
fn default_tools_per_line() -> usize {
    6
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PulselineConfig {
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub colors: ColorsConfig,
    #[serde(default)]
    pub segments: SegmentsConfig,
    #[serde(default)]
    pub pane: PaneSection,
}

fn default_pane_style() -> String {
    "none".to_string()
}
fn default_pane_width_mode() -> String {
    "auto".to_string()
}
fn default_pane_min_width() -> usize {
    60
}
fn default_pane_max_width() -> usize {
    140
}
fn default_pane_cc_margin() -> usize {
    crate::render::pane::DEFAULT_PANE_CC_MARGIN
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaneSection {
    #[serde(default = "default_pane_style")]
    pub style: String,
    #[serde(default = "default_pane_width_mode")]
    pub width_mode: String,
    #[serde(default)]
    pub fixed_width: Option<usize>,
    #[serde(default = "default_pane_min_width")]
    pub min_width: usize,
    #[serde(default = "default_pane_max_width")]
    pub max_width: usize,
    #[serde(default = "default_pane_cc_margin")]
    pub cc_margin: usize,
}

impl Default for PaneSection {
    fn default() -> Self {
        Self {
            style: default_pane_style(),
            width_mode: default_pane_width_mode(),
            fixed_width: None,
            min_width: default_pane_min_width(),
            max_width: default_pane_max_width(),
            cc_margin: default_pane_cc_margin(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_dark")]
    pub theme: String,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default = "default_true")]
    pub icons: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            theme: default_dark(),
            variant: None,
            icons: true,
        }
    }
}

/// Optional per-color overrides (ANSI 256-color codes, 0-255).
/// Applied on top of the selected theme preset.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ColorsConfig {
    // Emphasis tiers
    #[serde(default)]
    pub primary: Option<u8>,
    #[serde(default)]
    pub secondary: Option<u8>,
    #[serde(default)]
    pub structural: Option<u8>,
    #[serde(default)]
    pub separator: Option<u8>,
    // Alert tier
    #[serde(default)]
    pub alert_red: Option<u8>,
    #[serde(default)]
    pub alert_orange: Option<u8>,
    #[serde(default)]
    pub alert_magenta: Option<u8>,
    // Active tier
    #[serde(default)]
    pub active_cyan: Option<u8>,
    #[serde(default)]
    pub active_purple: Option<u8>,
    #[serde(default)]
    pub active_teal: Option<u8>,
    #[serde(default)]
    pub active_amber: Option<u8>,
    #[serde(default)]
    pub active_coral: Option<u8>,
    // Stable tier
    #[serde(default)]
    pub stable_blue: Option<u8>,
    #[serde(default)]
    pub stable_green: Option<u8>,
    // Indicator tier (L2 icons)
    #[serde(default)]
    pub indicator_claude_md: Option<u8>,
    #[serde(default)]
    pub indicator_rules: Option<u8>,
    #[serde(default)]
    pub indicator_memory: Option<u8>,
    #[serde(default)]
    pub indicator_hooks: Option<u8>,
    #[serde(default)]
    pub indicator_mcp: Option<u8>,
    #[serde(default)]
    pub indicator_skills: Option<u8>,
    #[serde(default)]
    pub indicator_duration: Option<u8>,
    // Completed accent
    #[serde(default)]
    pub completed_check: Option<u8>,
    // Cost tier
    #[serde(default)]
    pub cost_base: Option<u8>,
    #[serde(default)]
    pub cost_low_rate: Option<u8>,
    #[serde(default)]
    pub cost_med_rate: Option<u8>,
    #[serde(default)]
    pub cost_high_rate: Option<u8>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SegmentsConfig {
    #[serde(default)]
    pub identity: IdentitySegmentConfig,
    #[serde(default)]
    pub config: ConfigSegmentConfig,
    #[serde(default)]
    pub budget: BudgetSegmentConfig,
    #[serde(default)]
    pub quota: QuotaSegmentConfig,
    #[serde(default)]
    pub tools: ToolSegmentConfig,
    #[serde(default)]
    pub agents: SegmentToggle,
    #[serde(default)]
    pub todo: SegmentToggle,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IdentitySegmentConfig {
    #[serde(default = "default_true")]
    pub show_model: bool,
    #[serde(default = "default_true")]
    pub show_style: bool,
    #[serde(default = "default_true")]
    pub show_version: bool,
    #[serde(default = "default_true")]
    pub show_project: bool,
    #[serde(default = "default_true")]
    pub show_git: bool,
    #[serde(default)]
    pub show_git_stats: bool,
    #[serde(default = "default_true")]
    pub show_agent: bool,
    #[serde(default = "default_true")]
    pub show_worktree: bool,
    #[serde(default = "default_true")]
    pub show_effort: bool,
    #[serde(default = "default_true")]
    pub show_thinking: bool,
}

impl Default for IdentitySegmentConfig {
    fn default() -> Self {
        Self {
            show_model: true,
            show_style: true,
            show_version: true,
            show_project: true,
            show_git: true,
            show_git_stats: false,
            show_agent: true,
            show_worktree: true,
            show_effort: true,
            show_thinking: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigSegmentConfig {
    #[serde(default = "default_true")]
    pub show_claude_md: bool,
    #[serde(default = "default_true")]
    pub show_rules: bool,
    #[serde(default = "default_true")]
    pub show_memory: bool,
    #[serde(default = "default_true")]
    pub show_hooks: bool,
    #[serde(default = "default_true")]
    pub show_mcp: bool,
    #[serde(default = "default_true")]
    pub show_skills: bool,
    #[serde(default = "default_true")]
    pub show_plugins: bool,
    #[serde(default = "default_true")]
    pub show_duration: bool,
}

impl Default for ConfigSegmentConfig {
    fn default() -> Self {
        Self {
            show_claude_md: true,
            show_rules: true,
            show_memory: true,
            show_hooks: true,
            show_mcp: true,
            show_skills: true,
            show_plugins: true,
            show_duration: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BudgetSegmentConfig {
    #[serde(default = "default_true")]
    pub show_context: bool,
    #[serde(default = "default_true")]
    pub show_tokens: bool,
    #[serde(default = "default_true")]
    pub show_cost: bool,
    #[serde(default)]
    pub show_speed: bool,
}

impl Default for BudgetSegmentConfig {
    fn default() -> Self {
        Self {
            show_context: true,
            show_tokens: true,
            show_cost: true,
            show_speed: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuotaSegmentConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub show_five_hour: bool,
    #[serde(default)]
    pub show_seven_day: bool,
}

impl Default for QuotaSegmentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_five_hour: true,
            show_seven_day: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolSegmentConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
    #[serde(default = "default_max_completed")]
    pub max_completed: usize,
    #[serde(default = "default_tools_per_line")]
    pub tools_per_line: usize,
}

impl Default for ToolSegmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 2,
            max_completed: 4,
            tools_per_line: 6,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SegmentToggle {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
}

impl Default for SegmentToggle {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 2,
        }
    }
}

/// Resolve the user's home directory from environment.
pub fn user_home() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

/// Returns `~/.claude/pulseline/config.toml`
pub fn config_path() -> PathBuf {
    let home = user_home().unwrap_or_else(|| ".".to_string());
    PathBuf::from(home)
        .join(".claude")
        .join("pulseline")
        .join("config.toml")
}

/// Load config from disk, falling back to defaults if file is missing or invalid.
pub fn load_config() -> PulselineConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_else(|err| {
            eprintln!("warning: invalid config {}: {err}", path.display());
            PulselineConfig::default()
        }),
        Err(_) => PulselineConfig::default(),
    }
}

/// Generate the default config file content.
pub fn default_config_toml() -> &'static str {
    r#"[display]
theme = "dark"          # tokyo-night | echo-sub-zero | dark | light
# variant = "dark"      # dark | light (overrides theme-implied variant)
icons = true            # nerd font icons vs ascii

# [colors]              # Override individual ANSI 256-color codes (0-255)
# primary = 251         # emphasis tiers
# alert_red = 196       # alert/active/stable/indicator/cost tiers
# See docs/theme-palette.md for all 26 field names

[segments.identity]     # Line 1 — model, style, version, project, git
show_model = true
show_style = true
show_version = true
show_project = true
show_git = true
show_git_stats = false  # !3 +1 ✘2 ?4 file stats after branch
show_agent = true       # AG:agent-name when --agent is active
show_worktree = true    # (WT) indicator when in a worktree session
show_effort = true      # effort level pill (low/medium/high/xhigh/max, CC 2.1.119+)
show_thinking = true    # thinking mode indicator (CC 2.1.119+)

[segments.config]       # Line 2 — CLAUDE.md, rules, memories, hooks, MCPs, skills, duration
show_claude_md = true
show_rules = true
show_memory = true
show_hooks = true
show_mcp = true
show_skills = true
show_plugins = true      # N plugins (enabled Claude Code plugins, CC 2.0.12+)
show_duration = true

[segments.budget]       # Line 3 — context, tokens, cost
show_context = true
show_tokens = true
show_cost = true
show_speed = false          # output tok/s rate

[segments.quota]            # Usage/quota tracking (subscription plans)
enabled = false             # opt-in: requires OAuth credentials
show_five_hour = true
show_seven_day = false

[segments.tools]
enabled = true
max_lines = 2           # max running tools shown
max_completed = 4       # max completed tool counts
tools_per_line = 6      # completed tools per line

[segments.agents]
enabled = true
max_lines = 2

[segments.todo]
enabled = true
max_lines = 2

[pane]
# Group marker style:
#   "none"  — flat output, no grouping markers
#   "zones" — one `─── activity ───` rule between state and live activity (+1 row)
#   "grid"  — fixed label column + │ + right-padded content (table layout, 0 rows)
#   "cards" — each group is its own ╭─┬─╮ card, stacked vertically (+2 rows
#             per non-empty group, strong separation between groups)
#   "sections" — single outer ╭─┬─╮ frame with ├─┼─┤ between every group
#             (+2 rows + 1 per gap; cheaper than cards, same per-group separation)
style = "none"
#
# Width only applies to "zones" (which draws a horizontal rule).
width_mode = "auto"     # "auto" | "terminal" | "fixed"
# fixed_width = 100     # only used when width_mode = "fixed"
min_width = 60          # skip framing when terminal can't fit this many cols
max_width = 160         # clamp auto-sized frames to this many cols
# cc_margin = 4         # cols subtracted from detected width in "terminal" mode
"#
}

// ── Project Override Config (all-Optional for deep merge) ────────────

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectOverrideConfig {
    pub display: Option<ProjectDisplayOverride>,
    pub colors: Option<ColorsConfig>,
    pub segments: Option<ProjectSegmentsOverride>,
    pub pane: Option<ProjectPaneOverride>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectPaneOverride {
    pub style: Option<String>,
    pub width_mode: Option<String>,
    pub fixed_width: Option<usize>,
    pub min_width: Option<usize>,
    pub max_width: Option<usize>,
    pub cc_margin: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectDisplayOverride {
    pub theme: Option<String>,
    pub variant: Option<String>,
    pub icons: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectSegmentsOverride {
    pub identity: Option<ProjectIdentityOverride>,
    pub config: Option<ProjectConfigOverride>,
    pub budget: Option<ProjectBudgetOverride>,
    pub quota: Option<ProjectQuotaOverride>,
    pub tools: Option<ProjectToolOverride>,
    pub agents: Option<ProjectSegmentToggleOverride>,
    pub todo: Option<ProjectSegmentToggleOverride>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectIdentityOverride {
    pub show_model: Option<bool>,
    pub show_style: Option<bool>,
    pub show_version: Option<bool>,
    pub show_project: Option<bool>,
    pub show_git: Option<bool>,
    pub show_git_stats: Option<bool>,
    pub show_agent: Option<bool>,
    pub show_worktree: Option<bool>,
    pub show_effort: Option<bool>,
    pub show_thinking: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectConfigOverride {
    pub show_claude_md: Option<bool>,
    pub show_rules: Option<bool>,
    pub show_memory: Option<bool>,
    pub show_hooks: Option<bool>,
    pub show_mcp: Option<bool>,
    pub show_skills: Option<bool>,
    pub show_plugins: Option<bool>,
    pub show_duration: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectBudgetOverride {
    pub show_context: Option<bool>,
    pub show_tokens: Option<bool>,
    pub show_cost: Option<bool>,
    pub show_speed: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectQuotaOverride {
    pub enabled: Option<bool>,
    pub show_five_hour: Option<bool>,
    pub show_seven_day: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectToolOverride {
    pub enabled: Option<bool>,
    pub max_lines: Option<usize>,
    pub max_completed: Option<usize>,
    pub tools_per_line: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectSegmentToggleOverride {
    pub enabled: Option<bool>,
    pub max_lines: Option<usize>,
}

/// Returns `{project_root}/.claude/pulseline.toml`
pub fn project_config_path(project_root: &str) -> PathBuf {
    PathBuf::from(project_root)
        .join(".claude")
        .join("pulseline.toml")
}

/// Load project-level override config, returning None if file doesn't exist.
pub fn load_project_config(project_root: &str) -> Option<ProjectOverrideConfig> {
    let path = project_config_path(project_root);
    match std::fs::read_to_string(&path) {
        Ok(contents) => match toml::from_str(&contents) {
            Ok(config) => Some(config),
            Err(err) => {
                eprintln!("warning: invalid project config {}: {err}", path.display());
                None
            }
        },
        Err(_) => None,
    }
}

/// Deep-merge project overrides onto user config. `Some(value)` wins; `None` inherits.
pub fn merge_configs(
    mut user: PulselineConfig,
    project: &ProjectOverrideConfig,
) -> PulselineConfig {
    // Display overrides
    if let Some(display) = &project.display {
        if let Some(theme) = &display.theme {
            user.display.theme = theme.clone();
        }
        if let Some(variant) = &display.variant {
            user.display.variant = Some(variant.clone());
        }
        if let Some(icons) = display.icons {
            user.display.icons = icons;
        }
    }

    // Color overrides (field-by-field Some wins)
    if let Some(colors) = &project.colors {
        macro_rules! merge_color {
            ($field:ident) => {
                if let Some(v) = colors.$field {
                    user.colors.$field = Some(v);
                }
            };
        }
        merge_color!(primary);
        merge_color!(secondary);
        merge_color!(structural);
        merge_color!(separator);
        merge_color!(alert_red);
        merge_color!(alert_orange);
        merge_color!(alert_magenta);
        merge_color!(active_cyan);
        merge_color!(active_purple);
        merge_color!(active_teal);
        merge_color!(active_amber);
        merge_color!(active_coral);
        merge_color!(stable_blue);
        merge_color!(stable_green);
        merge_color!(indicator_claude_md);
        merge_color!(indicator_rules);
        merge_color!(indicator_memory);
        merge_color!(indicator_hooks);
        merge_color!(indicator_mcp);
        merge_color!(indicator_skills);
        merge_color!(indicator_duration);
        merge_color!(completed_check);
        merge_color!(cost_base);
        merge_color!(cost_low_rate);
        merge_color!(cost_med_rate);
        merge_color!(cost_high_rate);
    }

    // Segment overrides
    if let Some(segments) = &project.segments {
        if let Some(identity) = &segments.identity {
            if let Some(v) = identity.show_model {
                user.segments.identity.show_model = v;
            }
            if let Some(v) = identity.show_style {
                user.segments.identity.show_style = v;
            }
            if let Some(v) = identity.show_version {
                user.segments.identity.show_version = v;
            }
            if let Some(v) = identity.show_project {
                user.segments.identity.show_project = v;
            }
            if let Some(v) = identity.show_git {
                user.segments.identity.show_git = v;
            }
            if let Some(v) = identity.show_git_stats {
                user.segments.identity.show_git_stats = v;
            }
            if let Some(v) = identity.show_agent {
                user.segments.identity.show_agent = v;
            }
            if let Some(v) = identity.show_worktree {
                user.segments.identity.show_worktree = v;
            }
            if let Some(v) = identity.show_effort {
                user.segments.identity.show_effort = v;
            }
            if let Some(v) = identity.show_thinking {
                user.segments.identity.show_thinking = v;
            }
        }
        if let Some(config) = &segments.config {
            if let Some(v) = config.show_claude_md {
                user.segments.config.show_claude_md = v;
            }
            if let Some(v) = config.show_rules {
                user.segments.config.show_rules = v;
            }
            if let Some(v) = config.show_memory {
                user.segments.config.show_memory = v;
            }
            if let Some(v) = config.show_hooks {
                user.segments.config.show_hooks = v;
            }
            if let Some(v) = config.show_mcp {
                user.segments.config.show_mcp = v;
            }
            if let Some(v) = config.show_skills {
                user.segments.config.show_skills = v;
            }
            if let Some(v) = config.show_plugins {
                user.segments.config.show_plugins = v;
            }
            if let Some(v) = config.show_duration {
                user.segments.config.show_duration = v;
            }
        }
        if let Some(budget) = &segments.budget {
            if let Some(v) = budget.show_context {
                user.segments.budget.show_context = v;
            }
            if let Some(v) = budget.show_tokens {
                user.segments.budget.show_tokens = v;
            }
            if let Some(v) = budget.show_cost {
                user.segments.budget.show_cost = v;
            }
            if let Some(v) = budget.show_speed {
                user.segments.budget.show_speed = v;
            }
        }
        if let Some(quota) = &segments.quota {
            if let Some(v) = quota.enabled {
                user.segments.quota.enabled = v;
            }
            if let Some(v) = quota.show_five_hour {
                user.segments.quota.show_five_hour = v;
            }
            if let Some(v) = quota.show_seven_day {
                user.segments.quota.show_seven_day = v;
            }
        }
        if let Some(tools) = &segments.tools {
            if let Some(v) = tools.enabled {
                user.segments.tools.enabled = v;
            }
            if let Some(v) = tools.max_lines {
                user.segments.tools.max_lines = v;
            }
            if let Some(v) = tools.max_completed {
                user.segments.tools.max_completed = v;
            }
            if let Some(v) = tools.tools_per_line {
                user.segments.tools.tools_per_line = v;
            }
        }
        if let Some(agents) = &segments.agents {
            if let Some(v) = agents.enabled {
                user.segments.agents.enabled = v;
            }
            if let Some(v) = agents.max_lines {
                user.segments.agents.max_lines = v;
            }
        }
        if let Some(todo) = &segments.todo {
            if let Some(v) = todo.enabled {
                user.segments.todo.enabled = v;
            }
            if let Some(v) = todo.max_lines {
                user.segments.todo.max_lines = v;
            }
        }
    }

    if let Some(pane) = &project.pane {
        if let Some(v) = &pane.style {
            user.pane.style = v.clone();
        }
        if let Some(v) = &pane.width_mode {
            user.pane.width_mode = v.clone();
        }
        if pane.fixed_width.is_some() {
            user.pane.fixed_width = pane.fixed_width;
        }
        if let Some(v) = pane.min_width {
            user.pane.min_width = v;
        }
        if let Some(v) = pane.max_width {
            user.pane.max_width = v;
        }
        if let Some(v) = pane.cc_margin {
            user.pane.cc_margin = v;
        }
    }

    user
}

/// Load user config, then merge project overrides if available.
pub fn load_merged_config(project_root: Option<&str>) -> PulselineConfig {
    let user_config = load_config();
    match project_root {
        Some(root) => match load_project_config(root) {
            Some(project_config) => merge_configs(user_config, &project_config),
            None => user_config,
        },
        None => user_config,
    }
}

/// Validate that config files parse correctly. Returns a list of (path, error) pairs.
pub fn check_configs(project_root: Option<&str>) -> Vec<(PathBuf, String)> {
    let mut errors = Vec::new();

    let user_path = config_path();
    if user_path.exists() {
        if let Ok(contents) = std::fs::read_to_string(&user_path) {
            if let Err(err) = toml::from_str::<PulselineConfig>(&contents) {
                errors.push((user_path, err.to_string()));
            }
        }
    }

    if let Some(root) = project_root {
        let project_path = project_config_path(root);
        if project_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&project_path) {
                if let Err(err) = toml::from_str::<ProjectOverrideConfig>(&contents) {
                    errors.push((project_path, err.to_string()));
                }
            }
        }
    }

    errors
}

/// Update the `theme = "..."` line in a config file, preserving comments and formatting.
/// If the file does not exist, creates it from `template` with the theme pre-set.
/// If the file exists but has no `theme =` line, prepends a `[display]` section.
pub fn update_theme_in_config(
    path: &std::path::Path,
    template: &str,
    theme_name: &str,
) -> Result<(), String> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create directory: {e}"))?;
        }
        let content = template.replace("theme = \"dark\"", &format!("theme = \"{theme_name}\""));
        std::fs::write(path, content)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
        return Ok(());
    }

    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    let mut found = false;
    let updated: String = contents
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if !found
                && !trimmed.starts_with('#')
                && (trimmed.starts_with("theme =") || trimmed.starts_with("theme="))
            {
                found = true;
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                format!("{indent}theme = \"{theme_name}\"")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let new_contents = if found {
        updated
    } else {
        format!("[display]\ntheme = \"{theme_name}\"\n\n{contents}")
    };

    let new_contents = if new_contents.ends_with('\n') {
        new_contents
    } else {
        format!("{new_contents}\n")
    };

    std::fs::write(path, new_contents)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;

    Ok(())
}

/// Generate the default project config file content.
pub fn default_project_config_toml() -> &'static str {
    r#"# Project-level pulseline overrides
# Only set fields you want to override from the user config.
# Absent fields inherit from ~/.claude/pulseline/config.toml

# [display]
# theme = "light"

# [segments.identity]
# show_version = false
# show_git_stats = true
# show_agent = true
# show_worktree = true
# show_effort = false
# show_thinking = false

# [segments.config]
# show_memory = false
# show_skills = false
# show_plugins = false

# [segments.budget]
# show_tokens = false
# show_speed = true

# [segments.quota]
# enabled = true
# show_five_hour = true
# show_seven_day = false

# [segments.tools]
# enabled = true
# max_lines = 2
# max_completed = 4
# tools_per_line = 6

# [segments.agents]
# enabled = true
# max_lines = 2

# [segments.todo]
# enabled = true
# max_lines = 2

# [pane]
# style = "grid"            # "none" | "zones" | "grid" | "cards" | "sections"
# width_mode = "auto"       # "auto" | "terminal" | "fixed"
# fixed_width = 100
# min_width = 60
# max_width = 140
# cc_margin = 4             # "terminal" mode: cols subtracted for CC's slot padding
"#
}

// ── Render Config (runtime, built from PulselineConfig + env) ────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphMode {
    Ascii,
    Icon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorTheme {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidthDegradeStrategy {
    DropActivityLinesFirst,
    CompressLine2,
    CompressCoreLines,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderConfig {
    pub glyph_mode: GlyphMode,
    pub color_enabled: bool,
    pub palette: ThemePalette,
    // L1 segment toggles
    pub show_model: bool,
    pub show_style: bool,
    pub show_version: bool,
    pub show_project: bool,
    pub show_git: bool,
    pub show_git_stats: bool,
    pub show_agent: bool,
    pub show_worktree: bool,
    pub show_effort: bool,
    pub show_thinking: bool,
    // L2 segment toggles
    pub show_claude_md: bool,
    pub show_rules: bool,
    pub show_memory: bool,
    pub show_hooks: bool,
    pub show_mcp: bool,
    pub show_skills: bool,
    pub show_plugins: bool,
    pub show_duration: bool,
    // L3 segment toggles
    pub show_context: bool,
    pub show_tokens: bool,
    pub show_cost: bool,
    pub show_speed: bool,
    // Quota segment toggles
    pub show_quota: bool,
    pub show_quota_five_hour: bool,
    pub show_quota_seven_day: bool,
    // Activity segment toggles + limits
    pub max_tool_lines: usize,
    pub max_completed_tools: usize,
    pub tools_per_line: usize,
    pub max_agent_lines: usize,
    pub max_todo_lines: usize,
    pub show_tools: bool,
    pub show_agents: bool,
    pub show_todo: bool,
    pub transcript_window_events: usize,
    pub transcript_poll_throttle_ms: u64,
    pub terminal_width: Option<usize>,
    pub degrade_order: Vec<WidthDegradeStrategy>,
    // Pane framing
    pub pane_style: PaneStyle,
    pub pane_width_mode: PaneWidth,
    pub pane_min_width: usize,
    pub pane_max_width: usize,
    pub pane_cc_margin: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            glyph_mode: GlyphMode::Ascii,
            color_enabled: false,
            palette: ThemePalette::default(),
            show_model: true,
            show_style: true,
            show_version: true,
            show_project: true,
            show_git: true,
            show_git_stats: false,
            show_agent: true,
            show_worktree: true,
            show_effort: true,
            show_thinking: true,
            show_claude_md: true,
            show_rules: true,
            show_memory: true,
            show_hooks: true,
            show_mcp: true,
            show_skills: true,
            show_plugins: true,
            show_duration: true,
            show_context: true,
            show_tokens: true,
            show_cost: true,
            show_speed: false,
            show_quota: false,
            show_quota_five_hour: true,
            show_quota_seven_day: false,
            max_tool_lines: 2,
            max_completed_tools: 4,
            tools_per_line: 6,
            max_agent_lines: 2,
            max_todo_lines: 2,
            show_tools: true,
            show_agents: true,
            show_todo: true,
            transcript_window_events: 400,
            transcript_poll_throttle_ms: 250,
            terminal_width: None,
            degrade_order: vec![
                WidthDegradeStrategy::DropActivityLinesFirst,
                WidthDegradeStrategy::CompressLine2,
                WidthDegradeStrategy::CompressCoreLines,
            ],
            pane_style: PaneStyle::None,
            pane_width_mode: PaneWidth::Auto,
            pane_min_width: 60,
            pane_max_width: 140,
            pane_cc_margin: crate::render::pane::DEFAULT_PANE_CC_MARGIN,
        }
    }
}

fn parse_pane_style(value: &str) -> PaneStyle {
    match value.to_lowercase().as_str() {
        "none" => PaneStyle::None,
        "zones" => PaneStyle::Zones,
        "grid" => PaneStyle::Grid,
        "cards" => PaneStyle::Cards,
        "sections" => PaneStyle::Sections,
        unknown => {
            eprintln!(
                "warning: unknown pane.style {unknown:?}; falling back to \"none\" \
                 (valid: none | zones | grid | cards | sections)"
            );
            PaneStyle::None
        }
    }
}

fn parse_pane_width_mode(value: &str, fixed_width: Option<usize>) -> PaneWidth {
    match value.to_lowercase().as_str() {
        "terminal" => PaneWidth::Terminal,
        "fixed" => PaneWidth::Fixed(fixed_width.unwrap_or(100)),
        _ => PaneWidth::Auto,
    }
}

/// Resolve the terminal width from (in priority order): the `COLUMNS` env var,
/// then an `ioctl(TIOCGWINSZ)` probe. Returns `None` only when the process
/// has no controlling terminal at all — e.g. a daemon or systemd unit.
fn resolve_terminal_width(columns_env: Option<&str>, ioctl_probe: Option<u16>) -> Option<usize> {
    if let Some(raw) = columns_env {
        if let Ok(w) = raw.parse::<usize>() {
            return Some(w);
        }
    }
    ioctl_probe.map(|w| w as usize)
}

/// Two-stage ioctl probe: first try the inherited stdio fds via
/// `terminal_size::terminal_size()` (checks stdout → stderr → stdin), then on
/// Unix fall back to opening `/dev/tty` directly. The fallback is critical for
/// the Claude Code statusline hook context, where stdin, stdout, and stderr
/// are all pipes — in that case the inherited-fd probe returns `None`, but the
/// process is still attached to a controlling terminal reachable via
/// `/dev/tty`.
fn probe_ioctl_width() -> Option<u16> {
    if let Some((terminal_size::Width(w), _)) = terminal_size::terminal_size() {
        return Some(w);
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;
        if let Ok(f) = std::fs::File::open("/dev/tty") {
            if let Some((terminal_size::Width(w), _)) = terminal_size::terminal_size_of(f.as_fd()) {
                return Some(w);
            }
        }
    }
    None
}

fn detect_terminal_width() -> Option<usize> {
    let env_columns = std::env::var("COLUMNS").ok();
    resolve_terminal_width(env_columns.as_deref(), probe_ioctl_width())
}

/// Build a RenderConfig from PulselineConfig + environment overrides.
pub fn build_render_config(pulseline: &PulselineConfig) -> RenderConfig {
    let color_enabled = std::env::var("NO_COLOR").is_err();

    let glyph_mode = if pulseline.display.icons {
        GlyphMode::Icon
    } else {
        GlyphMode::Ascii
    };

    let palette = resolve_palette(
        &pulseline.display.theme,
        pulseline.display.variant.as_deref(),
        &pulseline.colors,
    );

    let terminal_width = detect_terminal_width();

    RenderConfig {
        color_enabled,
        palette,
        glyph_mode,
        terminal_width,
        // L1 identity toggles
        show_model: pulseline.segments.identity.show_model,
        show_style: pulseline.segments.identity.show_style,
        show_version: pulseline.segments.identity.show_version,
        show_project: pulseline.segments.identity.show_project,
        show_git: pulseline.segments.identity.show_git,
        show_git_stats: pulseline.segments.identity.show_git_stats,
        show_agent: pulseline.segments.identity.show_agent,
        show_worktree: pulseline.segments.identity.show_worktree,
        show_effort: pulseline.segments.identity.show_effort,
        show_thinking: pulseline.segments.identity.show_thinking,
        // L2 config toggles
        show_claude_md: pulseline.segments.config.show_claude_md,
        show_rules: pulseline.segments.config.show_rules,
        show_memory: pulseline.segments.config.show_memory,
        show_hooks: pulseline.segments.config.show_hooks,
        show_mcp: pulseline.segments.config.show_mcp,
        show_skills: pulseline.segments.config.show_skills,
        show_plugins: pulseline.segments.config.show_plugins,
        show_duration: pulseline.segments.config.show_duration,
        // L3 budget toggles
        show_context: pulseline.segments.budget.show_context,
        show_tokens: pulseline.segments.budget.show_tokens,
        show_cost: pulseline.segments.budget.show_cost,
        show_speed: pulseline.segments.budget.show_speed,
        // Quota
        show_quota: pulseline.segments.quota.enabled,
        show_quota_five_hour: pulseline.segments.quota.show_five_hour,
        show_quota_seven_day: pulseline.segments.quota.show_seven_day,
        // Activity
        max_tool_lines: pulseline.segments.tools.max_lines,
        max_completed_tools: pulseline.segments.tools.max_completed,
        tools_per_line: pulseline.segments.tools.tools_per_line,
        max_agent_lines: pulseline.segments.agents.max_lines,
        max_todo_lines: pulseline.segments.todo.max_lines,
        show_tools: pulseline.segments.tools.enabled,
        show_agents: pulseline.segments.agents.enabled,
        show_todo: pulseline.segments.todo.enabled,
        pane_style: parse_pane_style(&pulseline.pane.style),
        pane_width_mode: parse_pane_width_mode(
            &pulseline.pane.width_mode,
            pulseline.pane.fixed_width,
        ),
        pane_min_width: pulseline.pane.min_width,
        pane_max_width: pulseline.pane.max_width,
        pane_cc_margin: pulseline.pane.cc_margin,
        ..RenderConfig::default()
    }
}

#[cfg(test)]
mod terminal_width_tests {
    use super::resolve_terminal_width;

    #[test]
    fn columns_env_wins_over_ioctl() {
        assert_eq!(resolve_terminal_width(Some("120"), Some(80)), Some(120));
    }

    #[test]
    fn falls_back_to_ioctl_when_env_absent() {
        assert_eq!(resolve_terminal_width(None, Some(180)), Some(180));
    }

    #[test]
    fn falls_back_to_ioctl_when_env_unparseable() {
        assert_eq!(
            resolve_terminal_width(Some("not-a-number"), Some(160)),
            Some(160)
        );
    }

    #[test]
    fn returns_none_when_both_sources_fail() {
        assert_eq!(resolve_terminal_width(None, None), None);
        assert_eq!(resolve_terminal_width(Some("bogus"), None), None);
    }
}
