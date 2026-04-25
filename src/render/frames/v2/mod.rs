//! v2 layouts — instrument-cluster style statusline.
//!
//! Unlike v1 *frames* (which decorate v1 line output), v2 *layouts* fully own
//! the rendering pipeline for their style: they consume `RenderFrame` directly
//! and emit `Vec<String>` ready for stdout. They never go through
//! `apply_pane` — they are the pane.
//!
//! See `designs/statusline-v2-redesign.md` for the visual contracts each
//! layout fulfils.

pub mod auto;
pub mod cockpit;
pub mod console;
pub mod flightstrip;
pub mod shared;
