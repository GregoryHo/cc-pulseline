//! Layout frames.
//!
//! All frames here decorate the flat-row pipeline assembled in
//! `layout::render_frame`: `apply_pane` hands `(lines, groups)` to the
//! frame's `render` fn, which wraps it in box-drawing chrome. Console
//! is a thin variant of Sections (identity hoisted into the top frame
//! title).
//!
//! `Ledger` is the lone exception — it owns its full pipeline because
//! the TAG-column rhythm doesn't compose cleanly via `apply_pane`. See
//! `frames/ledger.rs` for the dispatch from `render_frame`.
//!
//! `frames/shared.rs` holds the box-drawing glyphs, label padding, and
//! widget dispatch hubs (`render_context_visual`, etc.) shared across
//! frames.

pub mod console;
pub mod grid;
pub mod ledger;
pub mod sections;
pub mod shared;
pub mod zones;

use super::pane::LayoutStyle;

/// The set of per-segment visual specs a layout proposes when the user TOML
/// leaves a `*_visual` field empty. Variation B from
/// `designs/composable-redesign.md`: each layout asserts a tasteful default,
/// but users can override per segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentVisualDefaults {
    pub context_visual: &'static str,
    pub cost_visual: &'static str,
    pub quota_visual: &'static str,
    pub tools_visual: &'static str,
    /// Agent visual spec — `+`-joined atoms `description`, `model`. The
    /// agent name is always rendered; `description` adds the body line
    /// and `model` adds a `[haiku]`-style slack tail.
    pub agents_visual: &'static str,
}

/// Per-layout default visual specs.
///
/// All layouts default to `text` / `list` for non-CTX segments; the
/// widget composition is opt-in via the `*_visual` config fields. Ledger
/// is the lone exception — it ships the sparkline on the CTX row by
/// default since the typographic rhythm leaves natural room for it.
pub const fn default_visuals_for(layout: LayoutStyle) -> SegmentVisualDefaults {
    match layout {
        // Flat / decorated-flat layouts: plain text everywhere.
        LayoutStyle::None
        | LayoutStyle::Zones
        | LayoutStyle::Grid
        | LayoutStyle::Sections
        | LayoutStyle::Console => SegmentVisualDefaults {
            context_visual: "text",
            cost_visual: "text",
            quota_visual: "text",
            tools_visual: "list",
            agents_visual: "name+description+model",
        },
        // Ledger ships sparkline by default on the CTX row — `text+sparkline`
        // renders the `43% 86.0k/200.0k` numbers and appends a braille
        // sparkline + delta-time tail. Users can opt out with
        // `context_visual = "text"`.
        LayoutStyle::Ledger => SegmentVisualDefaults {
            context_visual: "text+sparkline",
            cost_visual: "text",
            quota_visual: "text",
            tools_visual: "list",
            agents_visual: "name+description+model",
        },
    }
}

/// Resolve a user's `*_visual` config string against the layout default:
/// empty user value → use the default; otherwise the user value wins.
pub fn resolve_visual<'a>(user_value: &'a str, default_value: &'a str) -> &'a str {
    if user_value.is_empty() {
        default_value
    } else {
        user_value
    }
}
