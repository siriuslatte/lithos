//! Pre-apply plan preview.
//!
//! This module turns the resource-graph diff plus the live-state
//! reconciliation report into a compact, human-readable plan that we can
//! show to the user before deploy mutates anything on Roblox.
//!
//! Responsibilities are split intentionally:
//!
//! - [`model`]      – pure view-model types ([`Plan`], [`PlanRow`], [`ActionKind`],
//!                    [`RiskLevel`]). No I/O, no rendering.
//! - [`summarize`]  – pure helpers that turn [`RobloxInputs`] pairs into
//!                    human-friendly labels and field-level change lines.
//! - [`render`]     – terminal rendering and confirmation prompts. The only
//!                    place that touches stdin/stdout/stderr or color codes.
//!
//! Keeping these separated means deploy logic, diff logic, and tests can all
//! reuse the plan model without dragging in TTY or rendering concerns.

pub mod model;
pub mod render;
pub mod summarize;

#[allow(unused_imports)]
pub use model::{ActionKind, Plan, PlanCounts, PlanRow, RiskLevel};
