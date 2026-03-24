use crate::config::{user_home, ColorTheme, ColorsConfig};
use serde::Deserialize;

pub const RESET: &str = "\x1b[0m";

// ── ThemePalette — runtime color set built from presets + overrides ──

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePalette {
    // Emphasis tiers (vary by dark/light variant)
    pub primary: String,
    pub secondary: String,
    pub structural: String,
    pub separator: String,
    // Alert tier
    pub alert_red: String,
    pub alert_orange: String,
    pub alert_magenta: String,
    // Active tier
    pub active_cyan: String,
    pub active_purple: String,
    pub active_teal: String,
    pub active_amber: String,
    pub active_coral: String,
    // Stable tier
    pub stable_blue: String,
    pub stable_green: String,
    // Indicator tier (L2 icons)
    pub indicator_claude_md: String,
    pub indicator_rules: String,
    pub indicator_memory: String,
    pub indicator_hooks: String,
    pub indicator_mcp: String,
    pub indicator_skills: String,
    pub indicator_duration: String,
    // Completed accent
    pub completed_check: String,
    // Cost tier
    pub cost_base: String,
    pub cost_low_rate: String,
    pub cost_med_rate: String,
    pub cost_high_rate: String,
}

/// Context usage thresholds for color switching.
pub const CTX_WARN_THRESHOLD: u64 = 55;
pub const CTX_CRITICAL_THRESHOLD: u64 = 70;

/// Semantic aliases and color-selection helpers.
impl ThemePalette {
    /// Pick the context color for a given usage percentage.
    pub fn color_for_ctx_pct(&self, pct: u64) -> &str {
        if pct >= CTX_CRITICAL_THRESHOLD {
            self.ctx_critical()
        } else if pct >= CTX_WARN_THRESHOLD {
            self.ctx_warn()
        } else {
            self.ctx_good()
        }
    }

    /// Pick the cost burn rate color for a given $/h value.
    pub fn color_for_burn_rate(&self, per_hour: f64) -> &str {
        if per_hour > 50.0 {
            &self.cost_high_rate
        } else if per_hour > 10.0 {
            &self.cost_med_rate
        } else {
            &self.cost_low_rate
        }
    }

    pub fn git_green(&self) -> &str {
        &self.stable_green
    }
    pub fn git_modified(&self) -> &str {
        &self.alert_orange
    }
    pub fn git_added(&self) -> &str {
        &self.stable_green
    }
    pub fn git_deleted(&self) -> &str {
        &self.alert_red
    }
    pub fn git_ahead(&self) -> &str {
        &self.active_coral
    }
    pub fn git_behind(&self) -> &str {
        &self.active_coral
    }
    pub fn ctx_good(&self) -> &str {
        &self.stable_green
    }
    pub fn ctx_warn(&self) -> &str {
        &self.active_amber
    }
    pub fn ctx_critical(&self) -> &str {
        &self.alert_red
    }
    pub fn tool_blue(&self) -> &str {
        &self.active_cyan
    }
    pub fn agent_purple(&self) -> &str {
        &self.active_purple
    }
    pub fn todo_teal(&self) -> &str {
        &self.active_teal
    }
}

impl Default for ThemePalette {
    fn default() -> Self {
        let preset =
            load_builtin_theme("tokyo-night").expect("built-in tokyo-night theme must parse");
        build_palette(&preset.dark)
    }
}

// ── Theme file parsing ──

/// Maps directly to the `palette_mapping` object in theme JSON files.
/// Field names match JSON keys; emphasis fields accept both `primary` and `emphasis_primary`.
#[derive(Clone, Copy, Deserialize)]
struct PresetColors {
    #[serde(alias = "emphasis_primary")]
    primary: u8,
    #[serde(alias = "emphasis_secondary")]
    secondary: u8,
    #[serde(alias = "emphasis_structural")]
    structural: u8,
    #[serde(alias = "emphasis_separator")]
    separator: u8,
    alert_red: u8,
    alert_orange: u8,
    alert_magenta: u8,
    active_cyan: u8,
    active_purple: u8,
    active_teal: u8,
    active_amber: u8,
    active_coral: u8,
    stable_blue: u8,
    stable_green: u8,
    indicator_claude_md: u8,
    indicator_rules: u8,
    indicator_memory: u8,
    indicator_hooks: u8,
    indicator_mcp: u8,
    indicator_skills: u8,
    indicator_duration: u8,
    completed_check: u8,
    cost_base: u8,
    cost_low_rate: u8,
    cost_med_rate: u8,
    cost_high_rate: u8,
}

/// Light variant emphasis overrides (only 4 fields differ from dark).
#[derive(Deserialize)]
struct LightEmphasis {
    primary: u8,
    secondary: u8,
    structural: u8,
    separator: u8,
}

/// Top-level theme JSON file structure.
/// Only `palette_mapping` is required; all other fields are design documentation.
#[derive(Deserialize)]
struct ThemeFile {
    palette_mapping: PresetColors,
    light_emphasis: Option<LightEmphasis>,
    #[serde(default)]
    description: Option<String>,
}

struct ThemePreset {
    dark: PresetColors,
    light: PresetColors,
}

/// Parse a theme JSON string into a ThemePreset (dark + light variants).
fn parse_theme_json(json: &str) -> Result<ThemePreset, String> {
    let file: ThemeFile =
        serde_json::from_str(json).map_err(|e| format!("invalid theme JSON: {e}"))?;
    let dark = file.palette_mapping;
    let light = match file.light_emphasis {
        Some(le) => PresetColors {
            primary: le.primary,
            secondary: le.secondary,
            structural: le.structural,
            separator: le.separator,
            ..dark
        },
        None => dark,
    };
    Ok(ThemePreset { dark, light })
}

// ── Built-in themes (embedded at compile time, parsed once) ──

static BUILTIN_THEMES: &[(&str, &str)] = &[
    ("tokyo-night", include_str!("../themes/tokyo-night.json")),
    (
        "echo-sub-zero",
        include_str!("../themes/echo-sub-zero.json"),
    ),
    (
        "titanium-precision",
        include_str!("../themes/titanium-precision.json"),
    ),
    (
        "cnc-telemetry",
        include_str!("../themes/cnc-telemetry.json"),
    ),
    (
        "cyberdeck-hud",
        include_str!("../themes/cyberdeck-hud.json"),
    ),
    ("stark-hud", include_str!("../themes/stark-hud.json")),
    ("mako-reactor", include_str!("../themes/mako-reactor.json")),
    (
        "aburaya-twilight",
        include_str!("../themes/aburaya-twilight.json"),
    ),
    (
        "matte-carbon-neon",
        include_str!("../themes/matte-carbon-neon.json"),
    ),
];

/// Load a built-in theme by name. Parsed once per process via OnceLock cache.
fn load_builtin_theme(name: &str) -> Option<ThemePreset> {
    // Cache parsed presets to avoid re-parsing embedded JSON on every call
    static CACHE: std::sync::OnceLock<Vec<(String, ThemePreset)>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        BUILTIN_THEMES
            .iter()
            .filter_map(|(n, json)| {
                parse_theme_json(json)
                    .map(|preset| (n.to_string(), preset))
                    .ok()
            })
            .collect()
    });
    cache
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, preset)| ThemePreset {
            dark: preset.dark,
            light: preset.light,
        })
}

/// Load a custom theme from `~/.claude/pulseline/themes/{name}.json`.
/// Cached per process — file is only read once per theme name.
fn load_custom_theme(name: &str) -> Option<ThemePreset> {
    type Cache = std::sync::Mutex<Vec<(String, Option<(PresetColors, PresetColors)>)>>;
    static CUSTOM_CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();

    let cache = CUSTOM_CACHE.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    let mut entries = cache.lock().ok()?;

    if let Some((_, cached)) = entries.iter().find(|(n, _)| n == name) {
        return cached.map(|(dark, light)| ThemePreset { dark, light });
    }

    let result = load_custom_theme_from_disk(name);
    let cache_entry = result.as_ref().map(|p| (p.dark, p.light));
    entries.push((name.to_string(), cache_entry));
    result
}

fn load_custom_theme_from_disk(name: &str) -> Option<ThemePreset> {
    let home = user_home()?;
    let path = std::path::PathBuf::from(home)
        .join(".claude/pulseline/themes")
        .join(format!("{name}.json"));
    let content = std::fs::read_to_string(&path).ok()?;
    match parse_theme_json(&content) {
        Ok(preset) => Some(preset),
        Err(e) => {
            eprintln!("warning: failed to load theme \"{name}\": {e}");
            None
        }
    }
}

// ── Palette builder ──

/// Pre-computed ANSI 256-color escape strings.
/// Initialized once on first use, avoids repeated format! allocations.
static ANSI256_TABLE: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

fn ansi256(code: u8) -> String {
    let table =
        ANSI256_TABLE.get_or_init(|| (0..=255u8).map(|i| format!("\x1b[38;5;{i}m")).collect());
    table[code as usize].clone()
}

fn build_palette(preset: &PresetColors) -> ThemePalette {
    ThemePalette {
        primary: ansi256(preset.primary),
        secondary: ansi256(preset.secondary),
        structural: ansi256(preset.structural),
        separator: ansi256(preset.separator),
        alert_red: ansi256(preset.alert_red),
        alert_orange: ansi256(preset.alert_orange),
        alert_magenta: ansi256(preset.alert_magenta),
        active_cyan: ansi256(preset.active_cyan),
        active_purple: ansi256(preset.active_purple),
        active_teal: ansi256(preset.active_teal),
        active_amber: ansi256(preset.active_amber),
        active_coral: ansi256(preset.active_coral),
        stable_blue: ansi256(preset.stable_blue),
        stable_green: ansi256(preset.stable_green),
        indicator_claude_md: ansi256(preset.indicator_claude_md),
        indicator_rules: ansi256(preset.indicator_rules),
        indicator_memory: ansi256(preset.indicator_memory),
        indicator_hooks: ansi256(preset.indicator_hooks),
        indicator_mcp: ansi256(preset.indicator_mcp),
        indicator_skills: ansi256(preset.indicator_skills),
        indicator_duration: ansi256(preset.indicator_duration),
        completed_check: ansi256(preset.completed_check),
        cost_base: ansi256(preset.cost_base),
        cost_low_rate: ansi256(preset.cost_low_rate),
        cost_med_rate: ansi256(preset.cost_med_rate),
        cost_high_rate: ansi256(preset.cost_high_rate),
    }
}

/// Apply TOML color overrides on top of a preset palette.
pub fn apply_color_overrides(palette: &mut ThemePalette, overrides: &ColorsConfig) {
    macro_rules! apply {
        ($field:ident) => {
            if let Some(code) = overrides.$field {
                palette.$field = ansi256(code);
            }
        };
    }
    apply!(primary);
    apply!(secondary);
    apply!(structural);
    apply!(separator);
    apply!(alert_red);
    apply!(alert_orange);
    apply!(alert_magenta);
    apply!(active_cyan);
    apply!(active_purple);
    apply!(active_teal);
    apply!(active_amber);
    apply!(active_coral);
    apply!(stable_blue);
    apply!(stable_green);
    apply!(indicator_claude_md);
    apply!(indicator_rules);
    apply!(indicator_memory);
    apply!(indicator_hooks);
    apply!(indicator_mcp);
    apply!(indicator_skills);
    apply!(indicator_duration);
    apply!(completed_check);
    apply!(cost_base);
    apply!(cost_low_rate);
    apply!(cost_med_rate);
    apply!(cost_high_rate);
}

/// Resolve theme name + variant to a ThemePalette, with TOML overrides applied.
pub fn resolve_palette(
    theme: &str,
    variant: Option<&str>,
    overrides: &ColorsConfig,
) -> ThemePalette {
    let theme_lower = theme.to_lowercase();

    // Resolve variant: explicit field > inferred from theme name > default dark
    let resolved_variant = match variant {
        Some(v) if v.eq_ignore_ascii_case("light") => ColorTheme::Light,
        Some(_) => ColorTheme::Dark,
        None => {
            if theme_lower == "light" {
                ColorTheme::Light
            } else {
                ColorTheme::Dark
            }
        }
    };

    // Resolve theme: backward compat aliases → built-in → custom → fallback
    let resolved_name = match theme_lower.as_str() {
        "dark" | "light" => "tokyo-night",
        other => other,
    };

    let preset = load_builtin_theme(resolved_name)
        .or_else(|| load_custom_theme(resolved_name))
        .unwrap_or_else(|| {
            eprintln!("warning: unknown theme \"{resolved_name}\", falling back to tokyo-night");
            load_builtin_theme("tokyo-night").expect("built-in tokyo-night must exist")
        });

    let preset_colors = match resolved_variant {
        ColorTheme::Dark => &preset.dark,
        ColorTheme::Light => &preset.light,
    };
    let mut palette = build_palette(preset_colors);
    apply_color_overrides(&mut palette, overrides);
    palette
}

/// Returns built-in preset names + any custom themes found in `~/.claude/pulseline/themes/`.
pub fn available_presets() -> Vec<String> {
    theme_descriptions()
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

/// Returns `(name, description)` pairs for all available themes (built-in + custom).
pub fn theme_descriptions() -> Vec<(String, String)> {
    let mut descs: Vec<(String, String)> = BUILTIN_THEMES
        .iter()
        .map(|(name, json)| {
            let desc = serde_json::from_str::<ThemeFile>(json)
                .ok()
                .and_then(|f| f.description)
                .unwrap_or_default();
            (name.to_string(), desc)
        })
        .collect();

    if let Some(custom_dir) = custom_themes_dir() {
        if let Ok(entries) = std::fs::read_dir(custom_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    if let Some(stem) = path.file_stem() {
                        let name = stem.to_string_lossy().to_string();
                        if !descs.iter().any(|(n, _)| n == &name) {
                            let desc = std::fs::read_to_string(&path)
                                .ok()
                                .and_then(|json| serde_json::from_str::<ThemeFile>(&json).ok())
                                .and_then(|f| f.description)
                                .unwrap_or_default();
                            descs.push((name, desc));
                        }
                    }
                }
            }
        }
    }

    descs
}

/// Extract the ANSI 256-color code number from an escape string like `\x1b[38;5;111m`.
pub fn extract_ansi_code(escape: &str) -> Option<u8> {
    escape
        .strip_prefix("\x1b[38;5;")
        .and_then(|s| s.strip_suffix('m'))
        .and_then(|s| s.parse().ok())
}

fn custom_themes_dir() -> Option<std::path::PathBuf> {
    user_home().map(|home| std::path::PathBuf::from(home).join(".claude/pulseline/themes"))
}

// ── Legacy pub const — used by integration tests for Tokyo Night assertions ──

pub const ALERT_RED: &str = "\x1b[38;5;196m";
pub const ALERT_ORANGE: &str = "\x1b[38;5;214m";
pub const ALERT_MAGENTA: &str = "\x1b[38;5;201m";
pub const ACTIVE_CYAN: &str = "\x1b[38;5;117m";
pub const ACTIVE_PURPLE: &str = "\x1b[38;5;183m";
pub const ACTIVE_TEAL: &str = "\x1b[38;5;80m";
pub const ACTIVE_AMBER: &str = "\x1b[38;5;178m";
pub const ACTIVE_CORAL: &str = "\x1b[38;5;209m";
pub const STABLE_BLUE: &str = "\x1b[38;5;111m";
pub const STABLE_GREEN: &str = "\x1b[38;5;71m";
pub const INDICATOR_CLAUDE_MD: &str = "\x1b[38;5;109m";
pub const INDICATOR_RULES: &str = "\x1b[38;5;108m";
pub const INDICATOR_MEMORY: &str = "\x1b[38;5;182m";
pub const INDICATOR_HOOKS: &str = "\x1b[38;5;179m";
pub const INDICATOR_MCP: &str = "\x1b[38;5;139m";
pub const INDICATOR_SKILLS: &str = "\x1b[38;5;73m";
pub const INDICATOR_DURATION: &str = "\x1b[38;5;174m";
pub const COMPLETED_CHECK: &str = "\x1b[38;5;67m";
pub const COST_BASE: &str = "\x1b[38;5;222m";
pub const COST_LOW_RATE: &str = "\x1b[38;5;186m";
pub const COST_MED_RATE: &str = "\x1b[38;5;221m";
pub const COST_HIGH_RATE: &str = "\x1b[38;5;201m";
pub const GIT_GREEN: &str = STABLE_GREEN;
pub const GIT_MODIFIED: &str = ALERT_ORANGE;
pub const GIT_ADDED: &str = GIT_GREEN;
pub const GIT_DELETED: &str = ALERT_RED;
pub const GIT_AHEAD: &str = ACTIVE_CORAL;
pub const GIT_BEHIND: &str = ACTIVE_CORAL;
pub const CTX_GOOD: &str = STABLE_GREEN;
pub const CTX_WARN: &str = ACTIVE_AMBER;
pub const CTX_CRITICAL: &str = ALERT_RED;
pub const TOOL_BLUE: &str = ACTIVE_CYAN;
pub const AGENT_PURPLE: &str = ACTIVE_PURPLE;
pub const TODO_TEAL: &str = ACTIVE_TEAL;

// ── Core utility functions ──

pub fn colorize(text: &str, color: &str, enabled: bool) -> String {
    if enabled {
        format!("{color}{text}{RESET}")
    } else {
        text.to_string()
    }
}

pub fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

pub fn visible_width(s: &str) -> usize {
    strip_ansi(s).chars().count()
}

/// Take the first `count` visible characters from a string, preserving ANSI escape sequences.
pub fn take_visible_chars(s: &str, count: usize) -> String {
    let mut result = String::new();
    let mut visible = 0;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            result.push(ch);
            if chars.peek() == Some(&'[') {
                result.push(chars.next().unwrap());
                for next in chars.by_ref() {
                    result.push(next);
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            if visible >= count {
                break;
            }
            result.push(ch);
            visible += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorize_enabled() {
        let result = colorize("hello", STABLE_BLUE, true);
        assert_eq!(result, "\x1b[38;5;111mhello\x1b[0m");
    }

    #[test]
    fn colorize_disabled() {
        assert_eq!(colorize("hello", STABLE_BLUE, false), "hello");
    }

    #[test]
    fn strip_ansi_removes_escapes() {
        let colored = format!("{STABLE_BLUE}hello{RESET} {TOOL_BLUE}world{RESET}");
        assert_eq!(strip_ansi(&colored), "hello world");
    }

    #[test]
    fn strip_ansi_passes_plain_text() {
        assert_eq!(strip_ansi("no escapes"), "no escapes");
    }

    #[test]
    fn visible_width_ignores_ansi() {
        let colored = format!("{STABLE_BLUE}M:{RESET}{STABLE_BLUE}Opus{RESET}");
        assert_eq!(visible_width(&colored), 6); // "M:Opus"
    }

    #[test]
    fn take_visible_chars_preserves_ansi() {
        let colored = format!("{CTX_CRITICAL}hello{RESET}");
        let taken = take_visible_chars(&colored, 3);
        assert_eq!(visible_width(&taken), 3);
        assert!(taken.contains(CTX_CRITICAL));
    }

    #[test]
    fn take_visible_chars_plain_text() {
        assert_eq!(take_visible_chars("hello world", 5), "hello");
    }

    #[test]
    fn strip_256_color_codes() {
        let colored = format!("{AGENT_PURPLE}test{RESET}");
        assert_eq!(strip_ansi(&colored), "test");
        assert_eq!(visible_width(&colored), 4);
    }

    #[test]
    fn palette_dark_emphasis_tiers() {
        let p = resolve_palette("tokyo-night", Some("dark"), &ColorsConfig::default());
        assert!(p.primary.contains("251"));
        assert!(p.secondary.contains("146"));
        assert!(p.structural.contains("103"));
        assert!(p.separator.contains("238"));
    }

    #[test]
    fn palette_light_emphasis_tiers() {
        let p = resolve_palette("tokyo-night", Some("light"), &ColorsConfig::default());
        assert!(p.primary.contains("234"));
        assert!(p.secondary.contains("240"));
        assert!(p.structural.contains("246"));
        assert!(p.separator.contains("253"));
    }

    #[test]
    fn indicator_colors_are_distinct() {
        let indicators = [
            INDICATOR_CLAUDE_MD,
            INDICATOR_RULES,
            INDICATOR_MEMORY,
            INDICATOR_HOOKS,
            INDICATOR_MCP,
            INDICATOR_SKILLS,
            INDICATOR_DURATION,
        ];
        for i in 0..indicators.len() {
            for j in (i + 1)..indicators.len() {
                assert_ne!(
                    indicators[i], indicators[j],
                    "indicator colors should be distinct"
                );
            }
        }
    }

    #[test]
    fn completed_check_color_exists() {
        assert!(
            COMPLETED_CHECK.contains("67"),
            "completed check should use steel blue (67)"
        );
    }

    // ── ThemePalette tests ──

    #[test]
    fn tokyo_night_dark_palette_matches_legacy_consts() {
        let preset = load_builtin_theme("tokyo-night").unwrap();
        let p = build_palette(&preset.dark);
        assert_eq!(p.stable_blue, STABLE_BLUE);
        assert_eq!(p.alert_red, ALERT_RED);
        assert_eq!(p.active_cyan, ACTIVE_CYAN);
        assert_eq!(p.completed_check, COMPLETED_CHECK);
        assert_eq!(p.cost_base, COST_BASE);
    }

    #[test]
    fn echo_sub_zero_dark_palette_values() {
        let preset = load_builtin_theme("echo-sub-zero").unwrap();
        let p = build_palette(&preset.dark);
        assert!(p.primary.contains("255"));
        assert!(p.active_cyan.contains("110"));
        assert!(p.active_purple.contains("110")); // same as cyan
        assert!(p.active_amber.contains("191")); // lime warning
        assert!(p.alert_red.contains("160"));
        assert!(p.completed_check.contains("108"));
        assert!(p.cost_low_rate.contains("248")); // ghost gray
    }

    #[test]
    fn apply_overrides_patches_individual_colors() {
        let mut p = ThemePalette::default();
        let overrides = ColorsConfig {
            alert_red: Some(160),
            stable_blue: Some(75),
            ..ColorsConfig::default()
        };
        apply_color_overrides(&mut p, &overrides);
        assert!(p.alert_red.contains("160"));
        assert!(p.stable_blue.contains("75"));
        // Unchanged fields stay at default
        assert!(p.active_cyan.contains("117"));
    }

    #[test]
    fn resolve_palette_backward_compat_dark() {
        let p = resolve_palette("dark", None, &ColorsConfig::default());
        // "dark" maps to tokyo-night dark variant
        assert!(p.primary.contains("251"));
    }

    #[test]
    fn resolve_palette_backward_compat_light() {
        let p = resolve_palette("light", None, &ColorsConfig::default());
        // "light" maps to tokyo-night light variant
        assert!(p.primary.contains("234"));
    }

    #[test]
    fn resolve_palette_echo_sub_zero() {
        let p = resolve_palette("echo-sub-zero", None, &ColorsConfig::default());
        assert!(p.primary.contains("255"));
        assert!(p.active_cyan.contains("110"));
    }

    #[test]
    fn resolve_palette_unknown_falls_back() {
        let p = resolve_palette("nonexistent", None, &ColorsConfig::default());
        // Falls back to tokyo-night dark
        assert!(p.primary.contains("251"));
    }

    #[test]
    fn legacy_alias_methods() {
        let p = ThemePalette::default();
        assert_eq!(p.git_green(), &p.stable_green);
        assert_eq!(p.ctx_good(), &p.stable_green);
        assert_eq!(p.ctx_warn(), &p.active_amber);
        assert_eq!(p.ctx_critical(), &p.alert_red);
        assert_eq!(p.tool_blue(), &p.active_cyan);
        assert_eq!(p.agent_purple(), &p.active_purple);
        assert_eq!(p.todo_teal(), &p.active_teal);
    }
}
