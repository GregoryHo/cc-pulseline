//! Console — sections layout with the identity row hoisted into the
//! top frame title.
//!
//! Output rows (full width):
//! ```text
//!   ╭─ <identity-headline> ─────────────────────────────────────╮
//!   │ Config   │ <config-row>                                    │
//!   ├──────────┼─────────────────────────────────────────────────┤
//!   │ Budget   │ <ctx-row>  <tok-row>  <cost-row>  <quota-row>   │
//!   ├──────────┼─────────────────────────────────────────────────┤
//!   │ Activity │ <tools-row>  <agents-rows>  <todo>              │
//!   ╰──────────┴─────────────────────────────────────────────────╯
//! ```
//!
//! Console is structurally just `Sections` with `identity_in_frame_title`
//! turned on:
//! - The first row of the Identity group is reused verbatim as the
//!   title text (composed by `layout::render_frame` via
//!   `shared::identity_headline` with `" · "` separators).
//! - The top frame border becomes `╭─ <title> ───╮`, the Identity
//!   range is skipped during body walling.
//! - Every other group renders identically to Sections.
//!
//! No bespoke widget pipeline lives here anymore — gauge / sparkline /
//! arc / tape composition is handled by the shared visual hubs that all
//! flat-row layouts use, so any composability override picked up by
//! Sections is automatically picked up by Console too.

use std::ops::Range;

use crate::render::pane::{LineKind, PaneConfig};

use super::sections;
use super::shared::FrameGlyphs;

pub fn render(
    lines: &[String],
    groups: &[(LineKind, Range<usize>)],
    cfg: &PaneConfig,
    g: &FrameGlyphs,
) -> Vec<String> {
    let title = first_identity_line(lines, groups);
    sections::render_with_options(lines, groups, cfg, g, title.as_deref())
}

/// First raw line in the Identity group, or `None` if the layout-level
/// pipeline didn't produce one (e.g. all `show_*` identity toggles off).
fn first_identity_line(lines: &[String], groups: &[(LineKind, Range<usize>)]) -> Option<String> {
    groups
        .iter()
        .find(|(k, _)| *k == LineKind::Identity)
        .and_then(|(_, range)| lines.get(range.start).cloned())
}
