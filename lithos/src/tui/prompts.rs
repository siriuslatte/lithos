//! Arrow-key prompts.
//!
//! Thin wrapper over [`inquire`]. Centralized here so the rest of the
//! codebase doesn't have to think about render targets or non-interactive
//! fallbacks: every prompt has a deterministic answer when we're not
//! attached to a terminal.

use inquire::Select;

use crate::preview::render::Decision;

use super::interactive_enabled;

/// Result of the apply prompt. Mirrors [`Decision`] but with an extra
/// "show details" branch the caller handles by re-rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyChoice {
    Apply,
    ShowDetails,
    Cancel,
}

impl ApplyChoice {
    pub fn into_decision(self) -> Option<Decision> {
        match self {
            ApplyChoice::Apply => Some(Decision::Approve),
            ApplyChoice::Cancel => Some(Decision::Cancel),
            ApplyChoice::ShowDetails => None,
        }
    }
}

/// Arrow-key apply confirmation. Returns [`ApplyChoice::Cancel`] if the
/// terminal isn't interactive (callers should already have routed to a
/// plain-text path before reaching here).
pub fn confirm_apply(destructive: bool) -> ApplyChoice {
    if !interactive_enabled() {
        return ApplyChoice::Cancel;
    }

    let header = if destructive {
        "Apply this plan? (contains destructive changes)"
    } else {
        "Apply this plan?"
    };

    let options = vec!["Apply", "Show details", "Cancel"];

    match Select::new(header, options)
        .with_help_message("↑/↓ to move · enter to choose · esc to cancel")
        .prompt()
    {
        Ok("Apply") => ApplyChoice::Apply,
        Ok("Show details") => ApplyChoice::ShowDetails,
        _ => ApplyChoice::Cancel,
    }
}

/// Pick one environment from a list. Returns `None` on cancel or when not
/// interactive (callers fall back to existing branch-matching logic).
#[allow(dead_code)]
pub fn select_environment(labels: &[String]) -> Option<String> {
    if !interactive_enabled() || labels.is_empty() {
        return None;
    }
    Select::new("Select an environment", labels.to_vec())
        .with_help_message("↑/↓ to move · enter to choose · esc to cancel")
        .prompt()
        .ok()
}
