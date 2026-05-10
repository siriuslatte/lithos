//! Live progress spinners.
//!
//! Wraps [`indicatif`] so callers don't have to know about `MultiProgress`,
//! draw targets, or template strings. A [`Spinner`] is RAII: dropping it
//! without calling [`Spinner::success`] or [`Spinner::fail`] finishes it
//! silently, which keeps short-circuit `?` paths from leaving zombie
//! spinners on screen.
//!
//! Output goes to **stderr** to match the rest of the CLI chrome, so
//! machine-readable stdout (e.g. `outputs --format json`) stays clean.

use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use super::interactive_enabled;

const TICK_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const TICK_INTERVAL: Duration = Duration::from_millis(80);

/// A single live status line. Drop without `success`/`fail` to dismiss
/// silently (used on early-return error paths).
pub struct Spinner {
    bar: ProgressBar,
    label: String,
    finished: bool,
}

impl Spinner {
    /// Start a new spinner with the given label. Falls back to a plain
    /// stderr line when not running in an interactive terminal so log
    /// output stays linear in CI.
    pub fn start(label: impl Into<String>) -> Self {
        let label = label.into();

        if !interactive_enabled() {
            eprintln!("→ {}", label);
            return Self {
                bar: ProgressBar::hidden(),
                label,
                finished: true,
            };
        }

        let bar = ProgressBar::new_spinner();
        bar.set_draw_target(ProgressDrawTarget::stderr());
        bar.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .expect("static spinner template")
                .tick_strings(TICK_FRAMES),
        );
        bar.set_message(label.clone());
        bar.enable_steady_tick(TICK_INTERVAL);

        Self {
            bar,
            label,
            finished: false,
        }
    }

    /// Update the message shown next to the spinner.
    #[allow(dead_code)]
    pub fn update(&self, message: impl Into<String>) {
        if self.finished {
            return;
        }
        self.bar.set_message(message.into());
    }

    /// Mark the operation as successful and replace the spinner with a
    /// final ✓ line. After this the spinner is consumed.
    pub fn success(mut self, message: impl Into<String>) {
        let message = message.into();
        self.finished = true;
        if self.bar.is_hidden() {
            eprintln!("✓ {}", message);
        } else {
            self.bar.set_style(
                ProgressStyle::with_template("{prefix:.green.bold} {msg}")
                    .expect("static success template"),
            );
            self.bar.set_prefix("✓");
            self.bar.finish_with_message(message);
        }
    }

    /// Mark the operation as failed.
    #[allow(dead_code)]
    pub fn fail(mut self, message: impl Into<String>) {
        let message = message.into();
        self.finished = true;
        if self.bar.is_hidden() {
            eprintln!("✗ {}", message);
        } else {
            self.bar.set_style(
                ProgressStyle::with_template("{prefix:.red.bold} {msg}")
                    .expect("static failure template"),
            );
            self.bar.set_prefix("✗");
            self.bar.finish_with_message(message);
        }
    }

    /// Clear the spinner without printing a final state. Used when a
    /// follow-up renderer wants to take over the same line.
    #[allow(dead_code)]
    pub fn dismiss(mut self) {
        self.finished = true;
        if !self.bar.is_hidden() {
            self.bar.finish_and_clear();
        }
    }

    /// Borrow the original label (useful when reusing it in a final
    /// success/fail message).
    #[allow(dead_code)]
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if !self.finished && !self.bar.is_hidden() {
            self.bar.finish_and_clear();
        }
    }
}
