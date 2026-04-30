//! Interactive TUI surface for the Lithos CLI.
//!
//! This module is the single entry point for "Copilot CLI feel" output:
//! live spinners during long-running phases, arrow-key prompts for
//! confirmation, and a short-lived alt-screen plan viewer.
//!
//! Every public function in here is a no-op / plain-text fallback when
//! [`crate::ui::chrome_enabled`] is false, so CI logs and piped output
//! retain the existing behavior.

pub mod plan_view;
pub mod progress;
pub mod prompts;

use std::io::{self, IsTerminal};

/// Whether full interactive TUI features (alt-screen, raw mode) are usable.
///
/// Stricter than [`crate::ui::chrome_enabled`] because alt-screen mode
/// requires both stdout and stdin to be real terminals; otherwise we
/// degrade to the inline indicatif/inquire path or to plain output.
pub fn interactive_enabled() -> bool {
    io::stdout().is_terminal() && io::stdin().is_terminal() && io::stderr().is_terminal()
}
