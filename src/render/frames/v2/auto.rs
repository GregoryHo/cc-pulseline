//! Auto layout — width-bracket resolver for `pane.style = "auto"`.
//!
//! The brackets:
//!   width >= 130   → console
//!   110..130       → cockpit
//!   90..110        → flightstrip
//!   < 90           → cockpit (which itself collapses below 80 cols)
//!
//! The resolver re-runs every render tick — window resize triggers a layout
//! switch on the next CC poll without any state.

use crate::config::RenderConfig;
use crate::render::color::ThemePalette;
use crate::render::pane::PaneStyle;

use super::{cockpit, console, flightstrip};

/// Resolve "auto" to a concrete style based on `terminal_width`.
pub fn resolve(terminal_width: Option<usize>) -> PaneStyle {
    let w = terminal_width.unwrap_or(120);
    if w >= 130 {
        PaneStyle::V2Console
    } else if w >= 110 {
        PaneStyle::V2Cockpit
    } else if w >= 90 {
        PaneStyle::V2Flightstrip
    } else {
        PaneStyle::V2Cockpit
    }
}

/// Render — dispatches to the resolved layout.
pub fn render(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Vec<String> {
    match resolve(config.terminal_width) {
        PaneStyle::V2Console => console::render(frame, config, p),
        PaneStyle::V2Flightstrip => flightstrip::render(frame, config, p),
        _ => cockpit::render(frame, config, p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_thresholds() {
        assert_eq!(resolve(Some(150)), PaneStyle::V2Console);
        assert_eq!(resolve(Some(130)), PaneStyle::V2Console);
        assert_eq!(resolve(Some(129)), PaneStyle::V2Cockpit);
        assert_eq!(resolve(Some(120)), PaneStyle::V2Cockpit);
        assert_eq!(resolve(Some(110)), PaneStyle::V2Cockpit);
        assert_eq!(resolve(Some(109)), PaneStyle::V2Flightstrip);
        assert_eq!(resolve(Some(90)), PaneStyle::V2Flightstrip);
        assert_eq!(resolve(Some(89)), PaneStyle::V2Cockpit);
        assert_eq!(resolve(Some(60)), PaneStyle::V2Cockpit);
    }

    #[test]
    fn resolver_defaults_to_cockpit_when_width_unknown() {
        // None → defaults to 120 → Cockpit
        assert_eq!(resolve(None), PaneStyle::V2Cockpit);
    }
}
