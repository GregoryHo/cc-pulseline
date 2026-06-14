use crate::render::color::{resolve_palette, ThemePalette};
use crate::render::pane::LayoutStyle;
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
/// Agent / todo rows default to 1 — vertical footprint discipline.
/// Overflow folds into the last visible row as a ` +N` tail.
fn default_max_activity_lines() -> usize {
    1
}
fn default_max_completed() -> usize {
    4
}
/// Hard cap on completed-tool rows. Width-aware packing fills rows
/// greedily; once this many rows are filled, remaining items fold into
/// a ` +N` tail on the last visible row.
fn default_max_completed_lines() -> usize {
    1
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
    pub layout: LayoutSection,
    #[serde(default)]
    pub performance: PerformanceConfig,
}

fn default_transcript_window_events() -> usize {
    400
}
fn default_transcript_poll_throttle_ms() -> u64 {
    250
}

/// Transcript-parsing performance knobs. Defaults are tuned for the
/// p95 < 50ms render budget; raise the window only if activity state
/// looks stale after huge bursts, lower the throttle only for tests.
#[derive(Debug, Clone, Deserialize)]
pub struct PerformanceConfig {
    /// Max transcript events processed per invocation (0 = unlimited).
    #[serde(default = "default_transcript_window_events")]
    pub transcript_window_events: usize,
    /// Min interval between transcript re-reads (0 = no throttle).
    #[serde(default = "default_transcript_poll_throttle_ms")]
    pub transcript_poll_throttle_ms: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            transcript_window_events: default_transcript_window_events(),
            transcript_poll_throttle_ms: default_transcript_poll_throttle_ms(),
        }
    }
}

fn default_layout_name() -> String {
    "none".to_string()
}
fn default_layout_min_width() -> usize {
    60
}
fn default_layout_max_width() -> usize {
    140
}
fn default_layout_cc_margin() -> usize {
    crate::render::pane::DEFAULT_PANE_CC_MARGIN
}
fn default_layout_tonal_strata() -> bool {
    true
}
fn default_layout_ledger_dense() -> bool {
    false
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayoutSection {
    #[serde(default = "default_layout_name")]
    pub name: String,
    #[serde(default = "default_layout_min_width")]
    pub min_width: usize,
    #[serde(default = "default_layout_max_width")]
    pub max_width: usize,
    #[serde(default = "default_layout_cc_margin")]
    pub cc_margin: usize,
    /// Two-tier separator tint: state rows use `strata_state`, activity rows
    /// use `strata_activity`. See `designs/tonal-strata-redesign.md`.
    #[serde(default = "default_layout_tonal_strata")]
    pub tonal_strata: bool,
    /// Ledger-only: drop all inter-group blank rows for a compact rhythm.
    #[serde(default = "default_layout_ledger_dense")]
    pub ledger_dense: bool,
    /// Hard cap on TOTAL output rows (chrome included). When exceeded the
    /// height-degradation ladder collapses groups in order until the render
    /// fits. `None` = unlimited (current behavior). Ledger is exempt (it
    /// owns its pipeline) — use `ledger_dense` there.
    #[serde(default)]
    pub max_total_lines: Option<MaxTotalLines>,
}

/// TOML forms of `layout.max_total_lines`: an integer cap, or `"auto"`
/// (cap derived from the detected terminal height so the statusline never
/// eats more than ~25% of the terminal — see `auto_height_cap`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum MaxTotalLines {
    Cap(usize),
    Mode(String),
}

impl Default for LayoutSection {
    fn default() -> Self {
        Self {
            name: default_layout_name(),
            min_width: default_layout_min_width(),
            max_width: default_layout_max_width(),
            cc_margin: default_layout_cc_margin(),
            tonal_strata: default_layout_tonal_strata(),
            ledger_dense: default_layout_ledger_dense(),
            max_total_lines: None,
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
    // Tonal strata (chrome tier)
    #[serde(default)]
    pub strata_state: Option<u8>,
    #[serde(default)]
    pub strata_activity: Option<u8>,
    // Aurora pulse (3-stop gradient — sparkline velocity color)
    #[serde(default)]
    pub aurora_low: Option<u8>,
    #[serde(default)]
    pub aurora_mid: Option<u8>,
    #[serde(default)]
    pub aurora_high: Option<u8>,
    #[serde(default)]
    pub tag_label: Option<u8>,
    #[serde(default)]
    pub head_agent: Option<u8>,
    #[serde(default)]
    pub head_thinking: Option<u8>,
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
    pub agents: AgentSegmentConfig,
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
    /// Master toggle for the whole L2 config-counts row. Defaults to
    /// `false`: the counts (CLAUDE.md / rules / memories / hooks / MCPs /
    /// skills / plugins / duration) rarely change within a session, and
    /// the row costs a full terminal line everywhere (plus an ENV group
    /// in framed layouts). Set `enabled = true` to bring it back; the
    /// individual `show_*` toggles below then apply as before.
    #[serde(default)]
    pub enabled: bool,
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
            enabled: false,
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
    /// Opt-in cache trend — compact C-cell mini sparkline + ledger CACHE
    /// row. The creation-aware C coloring is always on regardless.
    #[serde(default)]
    pub show_cache_trend: bool,
    /// CTX visual spec — `"+"` separated widget names. Recognized:
    /// `gauge`, `sparkline`, `text`. Empty string defers to layout default
    /// (see `frames::default_visuals_for`).
    /// Examples: `"text+sparkline"` (ledger default), `"text"` (flat
    /// layouts default), `"gauge"` (text-free dashboard).
    #[serde(default)]
    pub context_visual: String,
}

impl Default for BudgetSegmentConfig {
    fn default() -> Self {
        Self {
            show_context: true,
            show_tokens: true,
            show_cost: true,
            show_speed: false,
            show_cache_trend: false,
            context_visual: String::new(),
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
    /// Quota visual spec. Recognized: `text`, `gauge`. Defers to layout
    /// default when empty (see `frames::default_visuals_for`).
    #[serde(default)]
    pub visual: String,
}

impl Default for QuotaSegmentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_five_hour: true,
            show_seven_day: false,
            visual: String::new(),
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
    #[serde(default = "default_max_completed_lines")]
    pub max_completed_lines: usize,
    /// Tools visual spec. Recognized atoms (`+`-joined): `counts`
    /// (completed-tool count rows), `targets` (running/recent tools row),
    /// `ticker` (counts + running fused into ONE row — subsumes both).
    /// Defers to the layout default when empty.
    #[serde(default)]
    pub visual: String,
}

impl Default for ToolSegmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 2,
            max_completed: 4,
            max_completed_lines: 1,
            visual: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentSegmentConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_activity_lines")]
    pub max_lines: usize,
    /// Agent visual spec. Recognized atoms (`+`-joined): `description`,
    /// `model`. The agent's name (type or first description line) is
    /// always rendered. Defers to the layout default when empty (see
    /// `frames::default_visuals_for`).
    #[serde(default)]
    pub visual: String,
}

impl Default for AgentSegmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 1,
            visual: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SegmentToggle {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_activity_lines")]
    pub max_lines: usize,
    /// Todo visual spec. Recognized atoms (`+`-joined): `text` (item
    /// text / task summary), `bar` (5-cell progress gauge of
    /// completed/total). Defers to the layout default when empty.
    #[serde(default)]
    pub visual: String,
}

impl Default for SegmentToggle {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 1,
            visual: String::new(),
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
theme = "dark"
# Built-ins: tokyo-night | echo-sub-zero | titanium-precision | cnc-telemetry
#            cyberdeck-hud | stark-hud | mako-reactor | aburaya-twilight
#            matte-carbon-neon | pulseline-aurora
# Legacy aliases: dark | light (tokyo-night with that variant)
# variant = "dark"      # dark | light (overrides theme-implied variant)
icons = true            # nerd font icons vs ascii

# [colors]              # Override individual ANSI 256-color codes (0-255)
# primary = 251         # emphasis tiers
# alert_red = 196       # alert/active/stable/indicator/cost tiers
# strata_state = 238    # chrome on state rows (L1/L2/L3/Quota)
# strata_activity = 103 # chrome on activity rows (Tools/Agents/Todos)
# See docs/theme-palette.md for all 34 field names

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
# Master toggle. Default OFF: the counts rarely change within a session
# and the row costs a full line (plus an ENV group in framed layouts).
enabled = false
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
show_cache_trend = false      # cache trend: compact C-cell sparkline + ledger CACHE row (opt-in)
# context_visual: per-segment widget composition. `+`-joined widget names.
# Empty (the default) defers to the layout's choice — most layouts use
# "text", ledger uses "text+sparkline". Examples:
#   context_visual = "gauge"            # gauge bar replaces percentage text
#   context_visual = "text"             # plain percentage + token counts
#   context_visual = "text+sparkline"   # numbers + braille trend
context_visual = ""

[segments.quota]            # Usage/quota tracking (subscription plans)
enabled = false             # opt-in: driven by CC's native rate_limits stdin field
show_five_hour = true
show_seven_day = false
# visual: bar gauge or text. Empty (default) defers to the layout —
# console / ledger default to "gauge" (F-style bar with threshold
# marks at 50% / 85% positions); flat layouts (none / compact)
# default to "text". Examples:
#   visual = "gauge"            # ▰▰▰▰▰▰▰▰▰───·─ 62% (resets ...)
#   visual = "text"             # 62% (resets ...) — no bar
visual = ""

[segments.tools]
enabled = true
max_lines = 2           # max running tools shown (0 = hide the running-tools row)
max_completed = 4       # max completed tool counts
max_completed_lines = 1 # max rows of completed tools (overflow folds into ` +N` tail)
# visual: `+`-joined atoms — `counts` (completed rows), `targets`
# (running tools row), `ticker` (both fused into ONE row).
#   visual = "counts+targets"   # layout default
#   visual = "ticker"           # ✓ 25 tools | T:Bash: cargo test
visual = ""

[segments.agents]
enabled = true
max_lines = 1
# visual: `+`-joined atoms — `description`, `model`. The agent name
# (type or first-line description) is always rendered.
#   visual = "name"               # name + ×N (parallel grouping kept)
#   visual = "name+description"   # name + body description
#   visual = "name+model"         # name + [haiku] tag
#   visual = "name+description+model"  # everything
visual = ""

[segments.todo]
enabled = true
max_lines = 1
# visual: `+`-joined atoms — `text` (item text), `bar` (5-cell progress
# gauge of completed/total).
#   visual = "text"        # layout default
#   visual = "bar+text"    # TODO:▰▰─── fix the parser (1/3)
visual = ""

[layout]
# Which renderer arranges the lines.
#
# Plain / decorated:
#   "none"     — flat output, no grouping markers
#   "compact"  — 2–3 rows total: packed identity row, packed budget+quota
#                row, activity ticker on row 3 (only when active).
#                Small but readable — never squeezes CC's footer.
#                (Absolute minimum: "none" + max_total_lines = 1–2.)
#   "console"  — single outer ╭─...─╮ frame with ├─┼─┤ between groups and
#                the identity row hoisted into the top frame title (best
#                ≥110 cols)
#
# Typographic-rhythm:
#   "ledger"   — TAG-column rows with blank-row group separation. Tallest
#                layout — favours rhythm over density. Ships sparkline +
#                delta-time on the CTX row by default.
name = "none"
min_width = 60          # skip framing when terminal can't fit this many cols
max_width = 160         # clamp auto-sized frames to this many cols
# cc_margin = 4         # cols subtracted from the detected terminal width

# Per-line separator tint. When true, the `|` between segments uses a
# theme-authored chrome pair: state rows (Identity/Config/Budget/Quota) get
# `strata_state`, activity rows (Tools/Agents/Todos) get `strata_activity`.
# Set to false for a fully flat baseline.
tonal_strata = true

# Ledger only: drop the blank rows that separate groups (ENV / Budget+Quota
# / TOOL / AGENT+TODO). The bottom frame always closes flush against the
# last content row regardless of this setting.
ledger_dense = false

# Hard cap on TOTAL statusline rows, frame chrome included. When the render
# exceeds it, groups collapse in order (running tools → completed tools →
# agents → todo → merged activity row → quota into L3 → drop config row →
# fuse-core: identity+budget+quota fused into one compact head row)
# until it fits. Unset = unlimited. Ledger ignores this (use ledger_dense).
# "auto" derives the cap from the terminal height (~25%, clamped to 4–10).
# max_total_lines = 6
# max_total_lines = "auto"

# ── Performance ──────────────────────────────────────────────────────
# [performance]
# transcript_window_events = 400    # max events parsed per tick (0 = unlimited)
# transcript_poll_throttle_ms = 250 # min ms between transcript re-reads
"#
}

// ── Project Override Config (all-Optional for deep merge) ────────────

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectOverrideConfig {
    pub display: Option<ProjectDisplayOverride>,
    pub colors: Option<ColorsConfig>,
    pub segments: Option<ProjectSegmentsOverride>,
    pub performance: Option<ProjectPerformanceOverride>,
    pub layout: Option<ProjectLayoutOverride>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectLayoutOverride {
    pub name: Option<String>,
    pub min_width: Option<usize>,
    pub max_width: Option<usize>,
    pub cc_margin: Option<usize>,
    pub tonal_strata: Option<bool>,
    pub ledger_dense: Option<bool>,
    pub max_total_lines: Option<MaxTotalLines>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectPerformanceOverride {
    pub transcript_window_events: Option<usize>,
    pub transcript_poll_throttle_ms: Option<u64>,
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
    pub agents: Option<ProjectAgentOverride>,
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
    pub enabled: Option<bool>,
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
    pub show_cache_trend: Option<bool>,
    /// CTX visual spec — see `BudgetSegmentConfig::context_visual`.
    pub context_visual: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectQuotaOverride {
    pub enabled: Option<bool>,
    pub show_five_hour: Option<bool>,
    pub show_seven_day: Option<bool>,
    /// Quota visual spec — see `QuotaSegmentConfig::visual`.
    pub visual: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectToolOverride {
    pub enabled: Option<bool>,
    pub max_lines: Option<usize>,
    pub max_completed: Option<usize>,
    pub max_completed_lines: Option<usize>,
    /// Tools visual spec — see `ToolSegmentConfig::visual`.
    pub visual: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectAgentOverride {
    pub enabled: Option<bool>,
    pub max_lines: Option<usize>,
    /// Agent visual spec — see `AgentSegmentConfig::visual`.
    pub visual: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectSegmentToggleOverride {
    pub enabled: Option<bool>,
    pub max_lines: Option<usize>,
    /// Todo visual spec — see `SegmentToggle::visual`.
    pub visual: Option<String>,
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
        merge_color!(strata_state);
        merge_color!(strata_activity);
        merge_color!(aurora_low);
        merge_color!(aurora_mid);
        merge_color!(aurora_high);
        merge_color!(tag_label);
        merge_color!(head_agent);
        merge_color!(head_thinking);
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
            if let Some(v) = config.enabled {
                user.segments.config.enabled = v;
            }
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
            if let Some(v) = budget.show_cache_trend {
                user.segments.budget.show_cache_trend = v;
            }
            if let Some(v) = &budget.context_visual {
                user.segments.budget.context_visual = v.clone();
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
            if let Some(v) = &quota.visual {
                user.segments.quota.visual = v.clone();
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
            if let Some(v) = tools.max_completed_lines {
                user.segments.tools.max_completed_lines = v;
            }
            if let Some(v) = &tools.visual {
                user.segments.tools.visual = v.clone();
            }
        }
        if let Some(agents) = &segments.agents {
            if let Some(v) = agents.enabled {
                user.segments.agents.enabled = v;
            }
            if let Some(v) = agents.max_lines {
                user.segments.agents.max_lines = v;
            }
            if let Some(v) = &agents.visual {
                user.segments.agents.visual = v.clone();
            }
        }
        if let Some(todo) = &segments.todo {
            if let Some(v) = todo.enabled {
                user.segments.todo.enabled = v;
            }
            if let Some(v) = todo.max_lines {
                user.segments.todo.max_lines = v;
            }
            if let Some(v) = &todo.visual {
                user.segments.todo.visual = v.clone();
            }
        }
    }

    if let Some(layout) = &project.layout {
        if let Some(v) = &layout.name {
            user.layout.name = v.clone();
        }
        if let Some(v) = layout.min_width {
            user.layout.min_width = v;
        }
        if let Some(v) = layout.max_width {
            user.layout.max_width = v;
        }
        if let Some(v) = layout.cc_margin {
            user.layout.cc_margin = v;
        }
        if let Some(v) = layout.tonal_strata {
            user.layout.tonal_strata = v;
        }
        if let Some(v) = layout.ledger_dense {
            user.layout.ledger_dense = v;
        }
        if let Some(v) = &layout.max_total_lines {
            user.layout.max_total_lines = Some(v.clone());
        }
    }

    if let Some(performance) = &project.performance {
        if let Some(v) = performance.transcript_window_events {
            user.performance.transcript_window_events = v;
        }
        if let Some(v) = performance.transcript_poll_throttle_ms {
            user.performance.transcript_poll_throttle_ms = v;
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
# enabled = true            # L2 row is opt-in (off by default)
# show_memory = false
# show_skills = false
# show_plugins = false

# [segments.budget]
# show_tokens = false
# show_speed = true
# show_cache_trend = true

# [segments.quota]
# enabled = true
# show_five_hour = true
# show_seven_day = false

# [segments.tools]
# enabled = true
# max_lines = 2            # 0 = hide the running-tools row
# max_completed = 4
# max_completed_lines = 1
# visual = ""               # "counts+targets" | "ticker"

# [segments.agents]
# enabled = true
# max_lines = 1
# visual = ""               # "name" | "name+description" | "name+model" | "name+description+model"

# [segments.todo]
# enabled = true
# max_lines = 1
# visual = ""               # "text" | "bar+text" | "bar"

# [layout]
# # Layouts: "none" | "compact" | "console" | "ledger"
# name = "console"
# min_width = 60
# max_width = 140
# cc_margin = 4             # cols subtracted for CC's slot padding
# tonal_strata = true       # 2-tier separator tint: state rows vs activity rows
# ledger_dense = false      # ledger-only: drop inter-group blank rows
# max_total_lines = 6       # hard cap on total rows (or "auto" = ~25% of terminal);
#                           # ladder ends at fuse-core (identity+budget on one row)
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

/// One rung of the height-degradation ladder. When the rendered output
/// exceeds `max_total_lines`, rungs are enabled cumulatively in
/// `height_degrade_order` until the render fits — volatile/low-info rows
/// collapse first, core metrics last. Mirrors `WidthDegradeStrategy`
/// (and like it, the order is a `RenderConfig` field, not a TOML knob).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeightDegradeStrategy {
    /// Drop the running/recent-tools row (volatile; tools resurface in
    /// the completed counts once they finish).
    DropRunningTools,
    /// Force completed-tool counts onto one row (` +N` fold).
    CollapseCompletedTools,
    /// Force agent groups onto one row (` +N` fold).
    CollapseAgents,
    /// Force todo onto one row (counts already carry the hidden items).
    CollapseTodo,
    /// Fuse ALL activity into a single packed row.
    MergeActivity,
    /// Append compact quota (`5h:62%`) to L3 instead of its own row.
    MergeQuotaIntoL3,
    /// Drop the L2 config-counts row (static, lowest urgency).
    DropLine2,
    /// Fuse identity + budget + compact quota into the compact layout's
    /// single priority-packed head row (the ladder's floor).
    FuseCore,
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
    /// Opt-in cache trend — compact C-cell mini sparkline + ledger CACHE
    /// row. The creation-aware C coloring is always on regardless.
    pub show_cache_trend: bool,
    /// Effective CTX visual spec — already resolved against the layout
    /// default if the user TOML left it empty. See
    /// `frames::default_visuals_for` for the per-layout defaults and
    /// `widgets::visual::render_context` for the dispatcher.
    pub context_visual: String,
    // Quota segment toggles
    pub show_quota: bool,
    pub show_quota_five_hour: bool,
    pub show_quota_seven_day: bool,
    /// Effective quota visual spec.
    pub quota_visual: String,
    /// Effective agents visual spec — atoms `description`, `model`.
    pub agents_visual: String,
    /// Effective tools visual spec — atoms `counts`, `targets`, `ticker`.
    pub tools_visual: String,
    /// Effective todo visual spec — atoms `text`, `bar`.
    pub todo_visual: String,
    // Activity segment toggles + limits
    pub max_tool_lines: usize,
    pub max_completed_tools: usize,
    /// Hard cap on completed-tool rows. See `default_max_completed_lines`.
    pub max_completed_lines: usize,
    pub max_agent_lines: usize,
    pub max_todo_lines: usize,
    pub show_tools: bool,
    pub show_agents: bool,
    pub show_todo: bool,
    pub transcript_window_events: usize,
    pub transcript_poll_throttle_ms: u64,
    pub terminal_width: Option<usize>,
    pub degrade_order: Vec<WidthDegradeStrategy>,
    /// Hard cap on total output rows (chrome included); `None` = off.
    /// Ledger is exempt — it owns its pipeline.
    pub max_total_lines: Option<usize>,
    pub height_degrade_order: Vec<HeightDegradeStrategy>,
    // Pane framing
    pub pane_style: LayoutStyle,
    pub pane_min_width: usize,
    pub pane_max_width: usize,
    pub pane_cc_margin: usize,
    pub pane_tonal_strata: bool,
    /// Ledger-only: drop all inter-group blank rows for a compact rhythm.
    pub pane_ledger_dense: bool,
}

impl RenderConfig {
    /// Effective CTX visual spec — falls back to the layout's default when
    /// the field is empty. `build_render_config` resolves this up-front so
    /// production renders never see an empty string; this helper exists for
    /// tests that construct `RenderConfig` directly via `Default` + field
    /// overrides.
    pub fn effective_context_visual(&self) -> &str {
        if self.context_visual.is_empty() {
            crate::render::frames::default_visuals_for(self.pane_style).context_visual
        } else {
            &self.context_visual
        }
    }

    /// See `effective_context_visual` — same fallback rule for quota.
    pub fn effective_quota_visual(&self) -> &str {
        if self.quota_visual.is_empty() {
            crate::render::frames::default_visuals_for(self.pane_style).quota_visual
        } else {
            &self.quota_visual
        }
    }

    /// See `effective_context_visual` — same fallback rule for agents.
    pub fn effective_agents_visual(&self) -> &str {
        if self.agents_visual.is_empty() {
            crate::render::frames::default_visuals_for(self.pane_style).agents_visual
        } else {
            &self.agents_visual
        }
    }

    /// See `effective_context_visual` — same fallback rule for tools.
    pub fn effective_tools_visual(&self) -> &str {
        if self.tools_visual.is_empty() {
            crate::render::frames::default_visuals_for(self.pane_style).tools_visual
        } else {
            &self.tools_visual
        }
    }

    /// See `effective_context_visual` — same fallback rule for todo.
    pub fn effective_todo_visual(&self) -> &str {
        if self.todo_visual.is_empty() {
            crate::render::frames::default_visuals_for(self.pane_style).todo_visual
        } else {
            &self.todo_visual
        }
    }
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
            show_cache_trend: false,
            context_visual: String::new(),
            show_quota: false,
            show_quota_five_hour: true,
            show_quota_seven_day: false,
            quota_visual: String::new(),
            agents_visual: String::new(),
            tools_visual: String::new(),
            todo_visual: String::new(),
            max_tool_lines: 2,
            max_completed_tools: 4,
            max_completed_lines: 1,
            max_agent_lines: 1,
            max_todo_lines: 1,
            show_tools: true,
            show_agents: true,
            show_todo: true,
            transcript_window_events: 400,
            transcript_poll_throttle_ms: 250,
            terminal_width: None,
            // Order matters: cheapest, least-destructive moves first.
            // CompressLine2 collapses L2 separators (no info loss);
            // CompressCoreLines truncates L1/L2/L3 with `...`;
            // DropActivityLinesFirst is last — it removes activity entirely,
            // so it should only fire if compression alone can't make room.
            max_total_lines: None,
            height_degrade_order: vec![
                HeightDegradeStrategy::DropRunningTools,
                HeightDegradeStrategy::CollapseCompletedTools,
                HeightDegradeStrategy::CollapseAgents,
                HeightDegradeStrategy::CollapseTodo,
                HeightDegradeStrategy::MergeActivity,
                HeightDegradeStrategy::MergeQuotaIntoL3,
                HeightDegradeStrategy::DropLine2,
                HeightDegradeStrategy::FuseCore,
            ],
            degrade_order: vec![
                WidthDegradeStrategy::CompressLine2,
                WidthDegradeStrategy::CompressCoreLines,
                WidthDegradeStrategy::DropActivityLinesFirst,
            ],
            pane_style: LayoutStyle::None,
            pane_min_width: 60,
            pane_max_width: 140,
            pane_cc_margin: crate::render::pane::DEFAULT_PANE_CC_MARGIN,
            pane_tonal_strata: true,
            pane_ledger_dense: false,
        }
    }
}

fn parse_layout_name(value: &str) -> LayoutStyle {
    match value.to_lowercase().as_str() {
        "none" => LayoutStyle::None,
        "compact" => LayoutStyle::Compact,
        "console" => LayoutStyle::Console,
        "ledger" => LayoutStyle::Ledger,
        "badge" => LayoutStyle::Badge,
        // Removed in the layout consolidations: zones / grid fold into the
        // flat default (they shared none's visual defaults); sections —
        // like cards, cockpit, flightstrip, and auto before it — folds
        // into console, its identity-in-title sibling.
        "zones" | "grid" => {
            eprintln!(
                "warning: layout.name {value:?} was removed; falling back to \
                 \"none\" (valid: none | compact | console | ledger | badge)"
            );
            LayoutStyle::None
        }
        "sections" | "cards" | "cockpit" | "flightstrip" | "auto" => {
            eprintln!(
                "warning: layout.name {value:?} was removed; falling back to \
                 \"console\" (valid: none | compact | console | ledger | badge)"
            );
            LayoutStyle::Console
        }
        unknown => {
            eprintln!(
                "warning: unknown layout.name {unknown:?}; falling back to \"none\" \
                 (valid: none | compact | console | ledger | badge)"
            );
            LayoutStyle::None
        }
    }
}

/// Resolve a user-supplied `*_visual` config string against the layout's
/// default. Empty user value means "use the layout's choice"; any non-empty
/// value wins outright. Returns an owned `String` because `RenderConfig`
/// stores effective specs as owned strings (the layout default is a
/// `'static str` literal).
fn resolve_visual_field(user_value: &str, default_value: &'static str) -> String {
    if user_value.is_empty() {
        default_value.to_string()
    } else {
        user_value.to_string()
    }
}

/// Resolve the terminal width from (in priority order): the `COLUMNS` env var,
/// then an `ioctl(TIOCGWINSZ)` probe. Returns `None` only when the process
/// has no controlling terminal at all — e.g. a daemon or systemd unit.
fn resolve_terminal_width(columns_env: Option<&str>, ioctl_probe: Option<u16>) -> Option<usize> {
    if let Some(raw) = columns_env {
        if let Ok(w) = raw.parse::<usize>() {
            if w > 0 {
                return Some(w);
            }
        }
    }
    ioctl_probe.map(|w| w as usize).filter(|w| *w > 0)
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

/// Height twin of `resolve_terminal_width`: `LINES` env (exported to the
/// statusline hook by CC 2.1.153+) wins, ioctl probe second.
fn resolve_terminal_height(lines_env: Option<&str>, ioctl_probe: Option<u16>) -> Option<usize> {
    if let Some(raw) = lines_env {
        if let Ok(h) = raw.parse::<usize>() {
            if h > 0 {
                return Some(h);
            }
        }
    }
    ioctl_probe.map(|h| h as usize).filter(|h| *h > 0)
}

/// Height twin of `probe_ioctl_width` — same two-stage probe, reading the
/// Height component instead.
fn probe_ioctl_height() -> Option<u16> {
    if let Some((_, terminal_size::Height(h))) = terminal_size::terminal_size() {
        return Some(h);
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;
        if let Ok(f) = std::fs::File::open("/dev/tty") {
            if let Some((_, terminal_size::Height(h))) = terminal_size::terminal_size_of(f.as_fd())
            {
                return Some(h);
            }
        }
    }
    None
}

fn detect_terminal_height() -> Option<usize> {
    let env_lines = std::env::var("LINES").ok();
    resolve_terminal_height(env_lines.as_deref(), probe_ioctl_height())
}

/// `"auto"` cap: at most ~25% of the terminal, clamped to [4, 10] so tiny
/// panes still show core metrics and tall monitors don't sprawl.
fn auto_height_cap(terminal_height: usize) -> usize {
    (terminal_height / 4).clamp(4, 10)
}

/// Resolve the TOML `max_total_lines` value into a concrete row cap.
/// Unknown string modes warn and disable the cap (fail open — a typo must
/// not suddenly truncate the user's statusline).
fn resolve_max_total_lines(value: Option<&MaxTotalLines>) -> Option<usize> {
    match value {
        None => None,
        Some(MaxTotalLines::Cap(n)) => Some(*n),
        Some(MaxTotalLines::Mode(mode)) if mode.eq_ignore_ascii_case("auto") => {
            detect_terminal_height().map(auto_height_cap)
        }
        Some(MaxTotalLines::Mode(unknown)) => {
            eprintln!(
                "warning: unknown layout.max_total_lines {unknown:?}; \
                 expected an integer or \"auto\""
            );
            None
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

    let palette = resolve_palette(
        &pulseline.display.theme,
        pulseline.display.variant.as_deref(),
        &pulseline.colors,
    );

    let terminal_width = detect_terminal_width();

    // Visual spec resolution (Variation B): when the user TOML leaves a
    // `*_visual` field empty, fall back to the layout's tasteful default.
    let layout_style = parse_layout_name(&pulseline.layout.name);
    let layout_visual_defaults = crate::render::frames::default_visuals_for(layout_style);

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
        // L2 is opt-in: the master `enabled` gate zeroes every show_*
        // toggle so the row (and framed layouts' ENV group) disappears.
        show_claude_md: pulseline.segments.config.enabled
            && pulseline.segments.config.show_claude_md,
        show_rules: pulseline.segments.config.enabled && pulseline.segments.config.show_rules,
        show_memory: pulseline.segments.config.enabled && pulseline.segments.config.show_memory,
        show_hooks: pulseline.segments.config.enabled && pulseline.segments.config.show_hooks,
        show_mcp: pulseline.segments.config.enabled && pulseline.segments.config.show_mcp,
        show_skills: pulseline.segments.config.enabled && pulseline.segments.config.show_skills,
        show_plugins: pulseline.segments.config.enabled && pulseline.segments.config.show_plugins,
        show_duration: pulseline.segments.config.enabled && pulseline.segments.config.show_duration,
        // L3 budget toggles
        show_context: pulseline.segments.budget.show_context,
        show_tokens: pulseline.segments.budget.show_tokens,
        show_cost: pulseline.segments.budget.show_cost,
        show_speed: pulseline.segments.budget.show_speed,
        show_cache_trend: pulseline.segments.budget.show_cache_trend,
        // Visual specs: empty user value defers to layout default. Resolved
        // here so the rest of the pipeline reads a fully-specified string.
        context_visual: resolve_visual_field(
            &pulseline.segments.budget.context_visual,
            layout_visual_defaults.context_visual,
        ),
        // Quota
        show_quota: pulseline.segments.quota.enabled,
        show_quota_five_hour: pulseline.segments.quota.show_five_hour,
        show_quota_seven_day: pulseline.segments.quota.show_seven_day,
        quota_visual: resolve_visual_field(
            &pulseline.segments.quota.visual,
            layout_visual_defaults.quota_visual,
        ),
        agents_visual: resolve_visual_field(
            &pulseline.segments.agents.visual,
            layout_visual_defaults.agents_visual,
        ),
        tools_visual: resolve_visual_field(
            &pulseline.segments.tools.visual,
            layout_visual_defaults.tools_visual,
        ),
        todo_visual: resolve_visual_field(
            &pulseline.segments.todo.visual,
            layout_visual_defaults.todo_visual,
        ),
        // Activity
        max_tool_lines: pulseline.segments.tools.max_lines,
        max_completed_tools: pulseline.segments.tools.max_completed,
        max_completed_lines: pulseline.segments.tools.max_completed_lines,
        max_agent_lines: pulseline.segments.agents.max_lines,
        max_todo_lines: pulseline.segments.todo.max_lines,
        show_tools: pulseline.segments.tools.enabled,
        show_agents: pulseline.segments.agents.enabled,
        show_todo: pulseline.segments.todo.enabled,
        pane_style: layout_style,
        pane_min_width: pulseline.layout.min_width,
        pane_max_width: pulseline.layout.max_width,
        pane_cc_margin: pulseline.layout.cc_margin,
        pane_tonal_strata: pulseline.layout.tonal_strata,
        pane_ledger_dense: pulseline.layout.ledger_dense,
        max_total_lines: resolve_max_total_lines(pulseline.layout.max_total_lines.as_ref()),
        transcript_window_events: pulseline.performance.transcript_window_events,
        transcript_poll_throttle_ms: pulseline.performance.transcript_poll_throttle_ms,
        ..RenderConfig::default()
    }
}

#[cfg(test)]
mod terminal_height_tests {
    use super::{auto_height_cap, resolve_max_total_lines, resolve_terminal_height, MaxTotalLines};

    #[test]
    fn lines_env_wins_over_ioctl() {
        assert_eq!(resolve_terminal_height(Some("48"), Some(24)), Some(48));
    }

    #[test]
    fn falls_back_to_ioctl_when_env_absent_or_bad() {
        assert_eq!(resolve_terminal_height(None, Some(40)), Some(40));
        assert_eq!(resolve_terminal_height(Some("bogus"), Some(40)), Some(40));
        assert_eq!(resolve_terminal_height(Some("0"), Some(40)), Some(40));
    }

    #[test]
    fn returns_none_when_both_sources_fail() {
        assert_eq!(resolve_terminal_height(None, None), None);
    }

    #[test]
    fn auto_cap_is_quarter_height_clamped() {
        assert_eq!(auto_height_cap(48), 10); // 12 → clamped to 10
        assert_eq!(auto_height_cap(32), 8);
        assert_eq!(auto_height_cap(24), 6);
        assert_eq!(auto_height_cap(10), 4); // 2 → clamped to 4
    }

    #[test]
    fn integer_cap_passes_through() {
        assert_eq!(
            resolve_max_total_lines(Some(&MaxTotalLines::Cap(6))),
            Some(6)
        );
        assert_eq!(resolve_max_total_lines(None), None);
    }

    #[test]
    fn unknown_mode_fails_open() {
        assert_eq!(
            resolve_max_total_lines(Some(&MaxTotalLines::Mode("never".to_string()))),
            None
        );
    }

    #[test]
    fn toml_accepts_integer_and_auto_forms() {
        let int_form: super::PulselineConfig =
            toml::from_str("[layout]\nmax_total_lines = 6\n").expect("int form parses");
        assert_eq!(int_form.layout.max_total_lines, Some(MaxTotalLines::Cap(6)));

        let auto_form: super::PulselineConfig =
            toml::from_str("[layout]\nmax_total_lines = \"auto\"\n").expect("auto form parses");
        assert_eq!(
            auto_form.layout.max_total_lines,
            Some(MaxTotalLines::Mode("auto".to_string()))
        );
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

    #[test]
    fn columns_env_zero_falls_through_to_ioctl() {
        // Background shells / statusline hook contexts can inherit `COLUMNS=0`;
        // treat it as "unknown" so ioctl probe gets a chance.
        assert_eq!(resolve_terminal_width(Some("0"), Some(160)), Some(160));
    }

    #[test]
    fn columns_env_zero_with_no_ioctl_yields_none() {
        assert_eq!(resolve_terminal_width(Some("0"), None), None);
    }

    #[test]
    fn ioctl_zero_treated_as_none() {
        assert_eq!(resolve_terminal_width(None, Some(0)), None);
    }
}
