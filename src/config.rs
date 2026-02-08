use serde::Deserialize;
use std::path::PathBuf;

// ── Pulseline Config (TOML file) ─────────────────────────────────────

fn default_true() -> bool {
    true
}
fn default_dark() -> String {
    "dark".to_string()
}
fn default_2() -> usize {
    2
}
fn default_4() -> usize {
    4
}

#[derive(Debug, Clone, Deserialize)]
pub struct PulselineConfig {
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub segments: SegmentsConfig,
}

impl Default for PulselineConfig {
    fn default() -> Self {
        Self {
            display: DisplayConfig::default(),
            segments: SegmentsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_dark")]
    pub theme: String,
    #[serde(default = "default_true")]
    pub icons: bool,
    #[serde(default)]
    pub tokyo_bg: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            theme: default_dark(),
            icons: true,
            tokyo_bg: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SegmentsConfig {
    #[serde(default)]
    pub tools: ToolSegmentConfig,
    #[serde(default)]
    pub agents: SegmentToggle,
    #[serde(default)]
    pub todo: SegmentToggle,
}

impl Default for SegmentsConfig {
    fn default() -> Self {
        Self {
            tools: ToolSegmentConfig::default(),
            agents: SegmentToggle::default(),
            todo: SegmentToggle::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolSegmentConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_2")]
    pub max_lines: usize,
    #[serde(default = "default_4")]
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
    #[serde(default = "default_2")]
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
tokyo_bg = false        # segmented background colors

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
    pub max_tool_lines: usize,
    pub max_completed_tools: usize,
    pub max_agent_lines: usize,
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
            max_tool_lines: 2,
            max_completed_tools: 4,
            max_agent_lines: 2,
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

    let color_theme = match pulseline.display.theme.as_str() {
        "light" => ColorTheme::Light,
        _ => ColorTheme::Dark,
    };

    let terminal_width = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse().ok());

    RenderConfig {
        color_enabled,
        color_theme,
        glyph_mode,
        terminal_width,
        max_tool_lines: pulseline.segments.tools.max_lines,
        max_completed_tools: pulseline.segments.tools.max_completed,
        max_agent_lines: pulseline.segments.agents.max_lines,
        show_tools: pulseline.segments.tools.enabled,
        show_agents: pulseline.segments.agents.enabled,
        show_todo: pulseline.segments.todo.enabled,
        ..RenderConfig::default()
    }
}
