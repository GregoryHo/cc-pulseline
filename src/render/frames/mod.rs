//! Layout frames — flat module list (no v1/v2 split).
//!
//! Two flavours of frame coexist here:
//! - **Decorating frames** (`zones`, `grid`, `cards`, `sections`) take the
//!   already-rendered v1 line output and wrap it in box-drawing chrome.
//! - **Instrument-cluster layouts** (`cockpit`, `console`, `flightstrip`,
//!   `auto`) own the entire rendering pipeline for their style: they consume
//!   `RenderFrame` directly and emit `Vec<String>` ready for stdout. They
//!   never go through `apply_pane` — they are the pane.
//!
//! Both flavours share `frames/shared.rs` for box-drawing glyphs, label
//! padding, widget call helpers, and cluster row builders.

pub mod auto;
pub mod cards;
pub mod cockpit;
pub mod console;
pub mod flightstrip;
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
/// Instrument-cluster layouts (cockpit / console / flightstrip / auto) ship
/// the rich gauge / sparkline / arc / tape personality that defines them.
/// Flat-row layouts (none / zones / grid / cards / sections) default to
/// `text` / `list` because they currently render the budget and tools rows
/// as plain text and lists; widget composition in those layouts is a
/// follow-up (see `designs/composable-redesign.md` § "Phase 3 scope").
pub const fn default_visuals_for(layout: LayoutStyle) -> SegmentVisualDefaults {
    match layout {
        LayoutStyle::Cockpit => SegmentVisualDefaults {
            context_visual: "gauge+sparkline",
            cost_visual: "text+arc",
            quota_visual: "text",
            tools_visual: "tape",
            agents_visual: "name+description+model",
        },
        LayoutStyle::Console => SegmentVisualDefaults {
            context_visual: "gauge+sparkline",
            cost_visual: "text+arc",
            quota_visual: "bar",
            // Default to brief tape — bisect of an earlier "tape+detail"
            // default showed it still broke multi-line output for at
            // least one user even after the recent_tool sanitiser fix
            // (root cause not yet identified). Users who want the
            // detailed per-tool format can opt in explicitly:
            //     [segments.tools]
            //     visual = "tape+detail"
            tools_visual: "tape",
            agents_visual: "name+description+model",
        },
        LayoutStyle::Flightstrip => SegmentVisualDefaults {
            context_visual: "gauge",
            cost_visual: "text",
            quota_visual: "text",
            tools_visual: "tape",
            agents_visual: "name+description",
        },
        // `auto` resolves to one of the three above per width; pick the
        // mid-width (cockpit) defaults so config preview matches what the
        // user is most likely to see.
        LayoutStyle::Auto => SegmentVisualDefaults {
            context_visual: "gauge+sparkline",
            cost_visual: "text+arc",
            quota_visual: "text",
            tools_visual: "tape",
            agents_visual: "name+description+model",
        },
        // Flat layouts: text / list. The widget call sites still hardcode
        // these forms — the visual fields are forward-compat for when those
        // sites get rewritten.
        LayoutStyle::None
        | LayoutStyle::Zones
        | LayoutStyle::Grid
        | LayoutStyle::Cards
        | LayoutStyle::Sections => SegmentVisualDefaults {
            context_visual: "text",
            cost_visual: "text",
            quota_visual: "text",
            tools_visual: "list",
            agents_visual: "name+description+model",
        },
        // Ledger ships sparkline by default on the CTX row — `text+sparkline`
        // means the existing `text` cell renders the `43% 86.0k/200.0k`
        // numbers and the sparkline appends after. Users can opt out with
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
