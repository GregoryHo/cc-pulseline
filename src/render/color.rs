use crate::config::ColorTheme;

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";

// Emphasis tiers — dark theme defaults (use EmphasisTier for theme-aware rendering)
pub const EMPHASIS: &str = "\x1b[38;5;254m";
pub const MUTED: &str = "\x1b[38;5;246m";
pub const SUBDUED: &str = "\x1b[38;5;238m";

// Structural tier (icons, labels, supporting text)
pub const STRUCTURAL_DARK: &str = "\x1b[38;5;60m"; // Tokyo Night comment color
pub const STRUCTURAL_LIGHT: &str = "\x1b[38;5;246m";

// Separator tier (punctuation only: |, (), /)
pub const SEPARATOR_DARK: &str = "\x1b[38;5;238m";
pub const SEPARATOR_LIGHT: &str = "\x1b[38;5;250m";

// ── ALERT tier — bright, saturated, demands immediate attention ──
pub const ALERT_RED: &str = "\x1b[38;5;196m";
pub const ALERT_ORANGE: &str = "\x1b[38;5;214m";
pub const ALERT_MAGENTA: &str = "\x1b[38;5;201m";

// ── ACTIVE tier — mid-saturation, currently happening ──
pub const ACTIVE_CYAN: &str = "\x1b[38;5;117m"; // Tokyo Night bright cyan
pub const ACTIVE_PURPLE: &str = "\x1b[38;5;183m"; // Tokyo Night magenta
pub const ACTIVE_TEAL: &str = "\x1b[38;5;80m";
pub const ACTIVE_AMBER: &str = "\x1b[38;5;178m";
pub const ACTIVE_CORAL: &str = "\x1b[38;5;209m";

// ── STABLE tier — muted, informational, unchanging context ──
pub const STABLE_BLUE: &str = "\x1b[38;5;111m"; // Tokyo Night main blue
pub const STABLE_GREEN: &str = "\x1b[38;5;71m";

// ── COST tier — rate-based coloring ──
pub const COST_BASE: &str = "\x1b[38;5;222m";
pub const COST_LOW_RATE: &str = "\x1b[38;5;186m";
pub const COST_MED_RATE: &str = "\x1b[38;5;221m";
pub const COST_HIGH_RATE: &str = "\x1b[38;5;201m";

// Legacy aliases → tier-based constants
pub const MODEL_BLUE: &str = STABLE_BLUE;
pub const GIT_GREEN: &str = STABLE_GREEN;
pub const GIT_MODIFIED: &str = ALERT_ORANGE;
pub const GIT_AHEAD: &str = ACTIVE_CORAL;
pub const GIT_BEHIND: &str = ACTIVE_CORAL;
pub const CTX_GOOD: &str = STABLE_GREEN;
pub const CTX_WARN: &str = ACTIVE_AMBER;
pub const CTX_CRITICAL: &str = ALERT_RED;
pub const TOOL_BLUE: &str = ACTIVE_CYAN;
pub const AGENT_PURPLE: &str = ACTIVE_PURPLE;
pub const TODO_TEAL: &str = ACTIVE_TEAL;

pub struct EmphasisTier {
    pub primary: &'static str,
    pub secondary: &'static str,
    pub structural: &'static str,
    pub separator: &'static str,
}

pub fn emphasis_for_theme(theme: ColorTheme) -> EmphasisTier {
    match theme {
        ColorTheme::Dark => EmphasisTier {
            primary: "\x1b[38;5;251m",    // Tokyo Night primary text
            secondary: "\x1b[38;5;146m",  // Tokyo Night secondary text
            structural: STRUCTURAL_DARK,   // 60 — Tokyo Night comment
            separator: SEPARATOR_DARK,     // 238
        },
        ColorTheme::Light => EmphasisTier {
            primary: "\x1b[38;5;236m",
            secondary: "\x1b[38;5;243m",
            structural: STRUCTURAL_LIGHT,
            separator: SEPARATOR_LIGHT,
        },
    }
}

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
                while let Some(next) = chars.next() {
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
                while let Some(next) = chars.next() {
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
        let result = colorize("hello", MODEL_BLUE, true);
        assert_eq!(result, "\x1b[38;5;111mhello\x1b[0m");
    }

    #[test]
    fn colorize_disabled() {
        assert_eq!(colorize("hello", MODEL_BLUE, false), "hello");
    }

    #[test]
    fn strip_ansi_removes_escapes() {
        let colored = format!("{MODEL_BLUE}hello{RESET} {TOOL_BLUE}world{RESET}");
        assert_eq!(strip_ansi(&colored), "hello world");
    }

    #[test]
    fn strip_ansi_passes_plain_text() {
        assert_eq!(strip_ansi("no escapes"), "no escapes");
    }

    #[test]
    fn visible_width_ignores_ansi() {
        let colored = format!("{MUTED}M:{RESET}{MODEL_BLUE}Opus{RESET}");
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
    fn emphasis_tiers_dark_theme() {
        let tier = emphasis_for_theme(ColorTheme::Dark);
        assert!(tier.primary.contains("251"));
        assert!(tier.secondary.contains("146"));
        assert!(tier.structural.contains("60"));
        assert!(tier.separator.contains("238"));
    }

    #[test]
    fn emphasis_tiers_light_theme() {
        let tier = emphasis_for_theme(ColorTheme::Light);
        assert!(tier.primary.contains("236"));
        assert!(tier.secondary.contains("243"));
        assert!(tier.structural.contains("246"));
        assert!(tier.separator.contains("250"));
    }
}
