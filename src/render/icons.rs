use crate::config::GlyphMode;

pub const ICON_MODEL: &str = "\u{e26d}";
pub const ICON_STYLE: &str = "\u{f040}";
pub const ICON_VERSION: &str = "\u{f427}";
pub const ICON_PROJECT: &str = "\u{f024b}";
pub const ICON_GIT: &str = "\u{f02a2}";
pub const ICON_CLAUDE_MD: &str = "\u{f0219}";
pub const ICON_RULES: &str = "\u{f0c47}"; // 󰱇 nf-md-gavel (rules to follow)
pub const ICON_MEMORY: &str = "\u{f09dc}"; // 󰧜 nf-md-brain (memory/knowledge)
pub const ICON_HOOKS: &str = "\u{f1b67}"; // 󱭧 nf-md-robot (automated execution)
pub const ICON_MCP: &str = "\u{f01a7}";
pub const ICON_SKILLS: &str = "\u{f0e7}"; // ⚡ Lightning bolt (ability/speed)
pub const ICON_ELAPSED: &str = "\u{f2f2}"; // ⏱️ Stopwatch (duration)
pub const ICON_CONTEXT: &str = "\u{f49b}";
pub const ICON_TOOL: &str = "\u{f0ad}";
pub const ICON_AGENT: &str = "\u{f19bb}";
pub const ICON_AGENT_DONE: &str = "✓";
/// Heterogeneous parallel-group prefix. NF: `‖` (math "parallel"), ASCII: `||`.
pub const ICON_GROUP_PARALLEL: (&str, &str) = ("\u{2016}", "||");
pub const ICON_TODO: &str = "\u{f0c8}";
pub const ICON_QUOTA: &str = "\u{f080}"; // nf-fa-bar_chart (usage/quota)

// Token type icons
pub const ICON_TOKEN_INPUT: &str = "\u{f093}";
pub const ICON_TOKEN_OUTPUT: &str = "\u{f019}";
pub const ICON_TOKEN_CACHE_CREATE: &str = "\u{f0c7}";

// Effort / thinking indicators (CC 2.1.119+)
pub const ICON_EFFORT: &str = "\u{f0ccd}"; // 󰳍 nf-md-speedometer
pub const ICON_THINKING: &str = "\u{f0335}"; // 󰌵 nf-md-lightbulb-on

// Plugins (CC 2.0.12+)
pub const ICON_PLUGIN: &str = "\u{f0431}"; // 󰐱 nf-md-puzzle

// Compaction marker (⟳ = U+27F3 clockwise arrow over circle; ASCII: ~)
pub const ICON_COMPACT: &str = "\u{27F3}";

// API-error badge (⚠ = U+26A0 warning sign; ASCII: !)
pub const ICON_API_ERROR: &str = "\u{26A0}";

// Frame-corner constants for test assertions (tests/console_layout.rs).
// The actual frame chrome — full corner/edge/junction set with Ascii
// variants — lives in `frames::shared::glyphs()`; these two must match
// its Icon-mode corners.
pub const FRAME_TL: char = '\u{256D}'; // ╭
pub const FRAME_BL: char = '\u{2570}'; // ╰

pub fn glyph(mode: GlyphMode, icon: &str, ascii: &str) -> String {
    match mode {
        GlyphMode::Icon => format!("{icon} "),
        GlyphMode::Ascii => ascii.to_string(),
    }
}
