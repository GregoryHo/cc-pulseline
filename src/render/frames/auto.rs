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
use crate::render::pane::LayoutStyle;

use super::{cockpit, flightstrip};

/// Resolve "auto" to a concrete style based on `terminal_width`.
pub fn resolve(terminal_width: Option<usize>) -> LayoutStyle {
    let w = terminal_width.unwrap_or(120);
    if w >= 130 {
        LayoutStyle::Console
    } else if w >= 110 {
        LayoutStyle::Cockpit
    } else if w >= 90 {
        LayoutStyle::Flightstrip
    } else {
        LayoutStyle::Cockpit
    }
}

/// Render — dispatches to the resolved layout.
pub fn render(
    frame: &crate::types::RenderFrame,
    config: &RenderConfig,
    p: &ThemePalette,
) -> Vec<String> {
    match resolve(config.terminal_width) {
        LayoutStyle::Console => {
            // Console no longer owns its own pipeline — re-enter
            // `render_frame` with the pane style overridden so the flat
            // path runs and `apply_pane` routes Console → sections-with-
            // identity-in-title.
            let mut adjusted = config.clone();
            adjusted.pane_style = LayoutStyle::Console;
            crate::render::layout::render_frame(frame, &adjusted)
        }
        LayoutStyle::Flightstrip => flightstrip::render(frame, config, p),
        _ => cockpit::render(frame, config, p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_thresholds() {
        assert_eq!(resolve(Some(150)), LayoutStyle::Console);
        assert_eq!(resolve(Some(130)), LayoutStyle::Console);
        assert_eq!(resolve(Some(129)), LayoutStyle::Cockpit);
        assert_eq!(resolve(Some(120)), LayoutStyle::Cockpit);
        assert_eq!(resolve(Some(110)), LayoutStyle::Cockpit);
        assert_eq!(resolve(Some(109)), LayoutStyle::Flightstrip);
        assert_eq!(resolve(Some(90)), LayoutStyle::Flightstrip);
        assert_eq!(resolve(Some(89)), LayoutStyle::Cockpit);
        assert_eq!(resolve(Some(60)), LayoutStyle::Cockpit);
    }

    #[test]
    fn resolver_defaults_to_cockpit_when_width_unknown() {
        // None → defaults to 120 → Cockpit
        assert_eq!(resolve(None), LayoutStyle::Cockpit);
    }
}
