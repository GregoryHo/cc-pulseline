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
pub mod sections;
pub mod shared;
pub mod zones;
