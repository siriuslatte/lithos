//! Per-operation dispatchers for [`RobloxResourceManager`].
//!
//! The dispatchers in this module perform Roblox API side effects. They are
//! intentionally separated from the manager's construction code so the root
//! file remains a thin orchestration layer.

pub(super) mod create;
pub(super) mod delete;
pub(super) mod price;
pub(super) mod update;
