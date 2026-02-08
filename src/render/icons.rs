use crate::config::GlyphMode;

pub const ICON_MODEL: &str = "\u{e26d}";
pub const ICON_STYLE: &str = "\u{f040}";
pub const ICON_VERSION: &str = "\u{f427}";
pub const ICON_PROJECT: &str = "\u{f024b}";
pub const ICON_GIT: &str = "\u{f02a2}";
pub const ICON_CLAUDE_MD: &str = "\u{f0219}";
pub const ICON_RULES: &str = "\u{f0c47}";   // 󰱇 nf-md-gavel (rules to follow)
pub const ICON_HOOKS: &str = "\u{f1b67}";   // 󱭧 nf-md-robot (automated execution)
pub const ICON_MCP: &str = "\u{f01a7}";
pub const ICON_SKILLS: &str = "\u{f0e7}";     // ⚡ Lightning bolt (ability/speed)
pub const ICON_ELAPSED: &str = "\u{f2f2}";    // ⏱️ Stopwatch (duration)
pub const ICON_CONTEXT: &str = "\u{f49b}";
pub const ICON_TOKENS: &str = "\u{f061d}";
pub const ICON_COST: &str = "\u{eec1}";
pub const ICON_TOOL: &str = "\u{f0ad}";
pub const ICON_AGENT: &str = "\u{f19bb}";
pub const ICON_TODO: &str = "\u{f0c8}";

// Token type icons
pub const ICON_TOKEN_INPUT: &str = "\u{f093}";
pub const ICON_TOKEN_OUTPUT: &str = "\u{f019}";
pub const ICON_TOKEN_CACHE_CREATE: &str = "\u{f0c7}";
pub const ICON_TOKEN_CACHE_READ: &str = "\u{f021}";

pub fn glyph(mode: GlyphMode, icon: &str, ascii: &str) -> String {
    match mode {
        GlyphMode::Icon => format!("{icon} "),
        GlyphMode::Ascii => ascii.to_string(),
    }
}

pub fn glyph_label(mode: GlyphMode, icon: &str, label: &str) -> String {
    match mode {
        GlyphMode::Icon => format!("{icon} {label}"),
        GlyphMode::Ascii => label.to_string(),
    }
}
