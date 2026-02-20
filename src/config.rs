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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PulselineConfig {
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub segments: SegmentsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_dark")]
    pub theme: String,
    #[serde(default = "default_true")]
    pub icons: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            theme: default_dark(),
            icons: true,
        }
    }
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
}

impl Default for ToolSegmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 2,
            max_completed: 4,
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

/// Returns `~/.claude/pulseline/config.toml`
pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
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
theme = "dark"          # dark | light
icons = true            # nerd font icons vs ascii

[segments.identity]     # Line 1 — model, style, version, project, git
show_model = true
show_style = true
show_version = true
show_project = true
show_git = true
show_git_stats = false  # !3 +1 ✘2 ?4 file stats after branch

[segments.config]       # Line 2 — CLAUDE.md, rules, memories, hooks, MCPs, skills, duration
show_claude_md = true
show_rules = true
show_memory = true
show_hooks = true
show_mcp = true
show_skills = true
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

[segments.agents]
enabled = true
max_lines = 2

[segments.todo]
enabled = true
max_lines = 2
"#
}

// ── Project Override Config (all-Optional for deep merge) ────────────

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectOverrideConfig {
    pub display: Option<ProjectDisplayOverride>,
    pub segments: Option<ProjectSegmentsOverride>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectDisplayOverride {
    pub theme: Option<String>,
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
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectConfigOverride {
    pub show_claude_md: Option<bool>,
    pub show_rules: Option<bool>,
    pub show_memory: Option<bool>,
    pub show_hooks: Option<bool>,
    pub show_mcp: Option<bool>,
    pub show_skills: Option<bool>,
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
        if let Some(icons) = display.icons {
            user.display.icons = icons;
        }
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

# [segments.config]
# show_memory = false
# show_skills = false

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

# [segments.agents]
# enabled = true
# max_lines = 2

# [segments.todo]
# enabled = true
# max_lines = 2
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderConfig {
    pub glyph_mode: GlyphMode,
    pub color_enabled: bool,
    pub color_theme: ColorTheme,
    // L1 segment toggles
    pub show_model: bool,
    pub show_style: bool,
    pub show_version: bool,
    pub show_project: bool,
    pub show_git: bool,
    pub show_git_stats: bool,
    // L2 segment toggles
    pub show_claude_md: bool,
    pub show_rules: bool,
    pub show_memory: bool,
    pub show_hooks: bool,
    pub show_mcp: bool,
    pub show_skills: bool,
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
    pub max_agent_lines: usize,
    pub max_todo_lines: usize,
    pub show_tools: bool,
    pub show_agents: bool,
    pub show_todo: bool,
    pub transcript_window_events: usize,
    pub transcript_poll_throttle_ms: u64,
    pub terminal_width: Option<usize>,
    pub degrade_order: Vec<WidthDegradeStrategy>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            glyph_mode: GlyphMode::Ascii,
            color_enabled: false,
            color_theme: ColorTheme::Dark,
            show_model: true,
            show_style: true,
            show_version: true,
            show_project: true,
            show_git: true,
            show_git_stats: false,
            show_claude_md: true,
            show_rules: true,
            show_memory: true,
            show_hooks: true,
            show_mcp: true,
            show_skills: true,
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
        }
    }
}

/// Build a RenderConfig from PulselineConfig + environment overrides.
pub fn build_render_config(pulseline: &PulselineConfig) -> RenderConfig {
    let color_enabled = std::env::var("NO_COLOR").is_err();

    let glyph_mode = if pulseline.display.icons {
        GlyphMode::Icon
    } else {
        GlyphMode::Ascii
    };

    let color_theme = match pulseline.display.theme.to_lowercase().as_str() {
        "light" => ColorTheme::Light,
        _ => ColorTheme::Dark,
    };

    let terminal_width = std::env::var("COLUMNS").ok().and_then(|v| v.parse().ok());

    RenderConfig {
        color_enabled,
        color_theme,
        glyph_mode,
        terminal_width,
        // L1 identity toggles
        show_model: pulseline.segments.identity.show_model,
        show_style: pulseline.segments.identity.show_style,
        show_version: pulseline.segments.identity.show_version,
        show_project: pulseline.segments.identity.show_project,
        show_git: pulseline.segments.identity.show_git,
        show_git_stats: pulseline.segments.identity.show_git_stats,
        // L2 config toggles
        show_claude_md: pulseline.segments.config.show_claude_md,
        show_rules: pulseline.segments.config.show_rules,
        show_memory: pulseline.segments.config.show_memory,
        show_hooks: pulseline.segments.config.show_hooks,
        show_mcp: pulseline.segments.config.show_mcp,
        show_skills: pulseline.segments.config.show_skills,
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
        max_agent_lines: pulseline.segments.agents.max_lines,
        max_todo_lines: pulseline.segments.todo.max_lines,
        show_tools: pulseline.segments.tools.enabled,
        show_agents: pulseline.segments.agents.enabled,
        show_todo: pulseline.segments.todo.enabled,
        ..RenderConfig::default()
    }
}
