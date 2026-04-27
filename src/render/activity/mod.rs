//! Width-aware activity-row rendering. See `designs/activity-width-budget.md`.

pub mod agent_groups;
pub mod budget;
pub mod builder;
pub mod cell;
pub mod truncate;

pub use builder::build_activity_rows;
