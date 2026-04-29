//! Per-tool / per-agent / per-todo Cell builders consumed by the activity
//! row builder and (post-cluster) the ledger layout's TOOL row.
//!
//! These produce `activity::cell::Cell` values; layout-level packing lives
//! in `activity::builder` and `activity::budget`.

pub mod recent_tool;
