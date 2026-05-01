//! Atomic widgets: hand-rolled glyph composers.
//!
//! Each widget renders to a small `String` (with optional ANSI) so layouts
//! can compose them like any other segment. No new deps — just Unicode
//! blocks and braille from the BMP.

pub mod gauge;
pub mod sparkline;

#[cfg(test)]
pub(super) mod test_support {
    use crate::render::color::ThemePalette;

    /// Build a `ThemePalette` whose aurora fields embed the markers
    /// `LOW`/`MID`/`HIGH` so widget tests can assert which gradient stop got
    /// applied without scraping ANSI escape codes.
    pub fn aurora_marker_palette() -> ThemePalette {
        ThemePalette {
            primary: String::new(),
            secondary: String::new(),
            structural: String::new(),
            separator: String::new(),
            alert_red: String::new(),
            alert_orange: String::new(),
            alert_magenta: String::new(),
            active_cyan: String::new(),
            active_purple: String::new(),
            active_teal: String::new(),
            active_amber: String::new(),
            active_coral: String::new(),
            stable_blue: String::new(),
            stable_green: String::new(),
            indicator_claude_md: String::new(),
            indicator_rules: String::new(),
            indicator_memory: String::new(),
            indicator_hooks: String::new(),
            indicator_mcp: String::new(),
            indicator_skills: String::new(),
            indicator_duration: String::new(),
            completed_check: String::new(),
            cost_base: String::new(),
            cost_low_rate: String::new(),
            cost_med_rate: String::new(),
            cost_high_rate: String::new(),
            strata_state: String::new(),
            strata_activity: String::new(),
            aurora_low: "LOW".to_string(),
            aurora_mid: "MID".to_string(),
            aurora_high: "HIGH".to_string(),
            tag_label: String::new(),
            head_agent: String::new(),
            head_thinking: String::new(),
        }
    }
}
