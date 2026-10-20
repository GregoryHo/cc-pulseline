//! Width-aware activity-row rendering. See `docs/architecture.md` → Activity Width Budget.

pub mod agent_groups;
pub mod budget;
pub mod builder;
pub mod cell;
pub mod cells;
pub mod truncate;

pub use builder::{
    build_activity_inline_row, build_activity_rows, build_activity_rows_running_first,
};
