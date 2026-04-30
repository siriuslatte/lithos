//! Terminal rendering and confirmation prompts for the deploy preview.
//!
//! Rendering is deliberately compact and dependency-light. The richer
//! "polished" UI comes from layout, alignment, color, and explicit risk
//! callouts rather than a full-screen TUI – which keeps the integration
//! narrow and the binary small while still being materially better than
//! the plain hash-level diff that exists today.
//!
//! All TTY/stdin interaction is funnelled through this module so deploy
//! logic can stay synchronous-friendly and testable.

use std::io::{self, BufRead, IsTerminal, Write};

use yansi::Paint;

use crate::tui;
use crate::ui;

use super::model::{ActionKind, Plan, RiskLevel};

/// How the user wants the preview presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    /// Default: rich preview when stdout is a TTY, plain summary otherwise.
    Auto,
    /// Force the plain (non-decorated) summary regardless of TTY.
    Plain,
    /// Skip the preview entirely.
    Off,
}

/// What the user (or environment) decided to do with the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Apply the plan.
    Approve,
    /// Cancel without applying.
    Cancel,
}

/// Inputs that govern preview behavior. Centralized so callers don't have
/// to thread several flags through every layer.
#[derive(Debug, Clone, Copy)]
pub struct PreviewOptions {
    pub mode: PreviewMode,
    /// `--yes`: auto-approve without prompting.
    pub assume_yes: bool,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            mode: PreviewMode::Auto,
            assume_yes: false,
        }
    }
}

/// Render the plan and obtain a confirmation decision, honoring the
/// supplied options and the current TTY environment.
///
/// Behavior:
/// - `mode = Off`: returns `Approve` immediately without rendering.
/// - `mode = Plain` or non-TTY stdout: prints a plain text summary. To
///   preserve compatibility with existing CI/scripted users that have no
///   way to answer a prompt, we auto-approve in non-interactive contexts
///   instead of hanging. Users who want strict gating can still pass
///   `--yes` for explicitness or invoke `diff` first.
/// - `mode = Auto` with TTY: prints a rich color-coded preview, then
///   prompts on stdin. `assume_yes` skips the prompt.
pub fn preview_and_confirm(plan: &Plan, options: PreviewOptions) -> Decision {
    if matches!(options.mode, PreviewMode::Off) {
        return Decision::Approve;
    }

    let stdout_is_tty = io::stdout().is_terminal();
    let stdin_is_tty = io::stdin().is_terminal();
    let interactive = matches!(options.mode, PreviewMode::Auto) && stdout_is_tty && stdin_is_tty;

    // Defer rendering to the interactive flow when we'll be opening the
    // alt-screen viewer so we don't paint the panel twice. Non-interactive
    // paths still render inline below.
    if !interactive {
        render_plain(plan);
    }

    if plan.counts.total_changes() == 0 {
        // Nothing to apply. Nothing to confirm. Drift-unknown is reported
        // for visibility but doesn't block.
        if interactive {
            render_rich(plan);
        }
        return Decision::Approve;
    }

    if options.assume_yes {
        if interactive {
            render_rich(plan);
        }
        eprintln!("{}", Paint::new("Auto-approving plan (--yes).").dimmed());
        return Decision::Approve;
    }

    if !interactive {
        // Non-interactive: never hang on stdin. Auto-approve so existing
        // CI/scripted workflows keep working, but make it visible.
        eprintln!(
            "{}",
            Paint::new(
                "Non-interactive environment: auto-approving plan. Use --yes to silence this notice or run `lithos diff` first to review."
            )
            .dimmed()
        );
        return Decision::Approve;
    }

    interactive_flow(plan)
}

/// Interactive flow: alt-screen plan viewer first, then a Copilot-style
/// arrow-key confirmation prompt. Falls back to the inline `render_rich`
/// + y/N readline prompt when the alt-screen viewer can't be used (e.g.
///   the terminal supports color but not raw mode).
fn interactive_flow(plan: &Plan) -> Decision {
    if !tui::interactive_enabled() {
        render_rich(plan);
        return prompt_confirm();
    }

    // First pass: short-lived alt-screen viewer with arrow keys.
    let initial = tui::plan_view::run(plan);
    if matches!(initial, Decision::Cancel) {
        render_rich(plan);
        eprintln!("{}", Paint::yellow("Cancelled."));
        return Decision::Cancel;
    }

    // Re-print the rich inline plan once we leave the alt-screen so the
    // user has a persistent record above the prompt of what they're about
    // to approve.
    render_rich(plan);

    // For destructive plans, require a second arrow-key confirmation so a
    // stray `y` keystroke inside the viewer can't trigger a destroy.
    let destructive = plan.counts.deletes > 0 || plan.counts.drift_recreate > 0;
    if !destructive {
        return Decision::Approve;
    }

    loop {
        let choice = tui::prompts::confirm_apply(true);
        if let Some(decision) = choice.into_decision() {
            if matches!(decision, Decision::Cancel) {
                eprintln!("{}", Paint::yellow("Cancelled."));
            }
            return decision;
        }
        // ShowDetails: re-open the alt-screen viewer, then re-prompt.
        let again = tui::plan_view::run(plan);
        if matches!(again, Decision::Cancel) {
            eprintln!("{}", Paint::yellow("Cancelled."));
            return Decision::Cancel;
        }
        render_rich(plan);
    }
}

fn render_rich(plan: &Plan) {
    let mut body: Vec<String> = Vec::new();
    body.push(format_header(plan));
    body.push(String::new());

    if plan.rows.is_empty() {
        body.push(format!(
            "{}",
            Paint::new("No changes. Your infrastructure is up to date.").dimmed()
        ));
        ui::print_panel("Plan", &body);
        eprintln!();
        return;
    }

    let type_width = plan
        .rows
        .iter()
        .map(|r| r.resource_type.chars().count())
        .max()
        .unwrap_or(0)
        .min(28);

    for row in &plan.rows {
        let marker = colorize_marker(row.action);
        let rtype = pad(&row.resource_type, type_width);
        body.push(format!(
            "{} {}  {}",
            marker,
            Paint::new(rtype).bold(),
            row.label,
        ));

        for line in &row.summary {
            body.push(format!("      {}", Paint::new(line).dimmed()));
        }
        if let Some(risk) = row.risk.label() {
            let painted = match row.risk {
                RiskLevel::Destructive => format!(
                    "{}",
                    Paint::red(format!("      ⚠ warning: {}", risk)).bold()
                ),
                RiskLevel::Caution => {
                    format!("{}", Paint::yellow(format!("      ! note: {}", risk)))
                }
                RiskLevel::Safe => String::new(),
            };
            if !painted.is_empty() {
                body.push(painted);
            }
        }
    }
    ui::print_panel("Plan", &body);
    eprintln!();
}

fn render_plain(plan: &Plan) {
    eprintln!();
    eprintln!("Lithos plan");
    eprintln!("{}", strip_color(&format_header(plan)));
    eprintln!();
    if plan.rows.is_empty() {
        eprintln!("  No changes. Your infrastructure is up to date.");
        eprintln!();
        return;
    }
    for row in &plan.rows {
        eprintln!(
            "  {} {} {} {}",
            row.action.marker(),
            row.action.verb(),
            row.resource_type,
            row.label
        );
        for line in &row.summary {
            eprintln!("      {}", line);
        }
        if let Some(risk) = row.risk.label() {
            eprintln!("      [{}]", risk);
        }
    }
    eprintln!();
}

fn format_header(plan: &Plan) -> String {
    let c = &plan.counts;
    format!(
        "{} create   {} update   {} delete   {} dep   {} drift!   {} drift?",
        Paint::green(format!("+{}", c.creates)).bold(),
        Paint::yellow(format!("~{}", c.updates)).bold(),
        Paint::red(format!("-{}", c.deletes)).bold(),
        Paint::new(format!("○{}", c.dependency_changes)).dimmed(),
        Paint::red(format!("!{}", c.drift_recreate)).bold(),
        Paint::yellow(format!("?{}", c.drift_unknown)),
    )
}

fn colorize_marker(kind: ActionKind) -> Paint<&'static str> {
    match kind {
        ActionKind::Create => Paint::green(kind.marker()).bold(),
        ActionKind::Update => Paint::yellow(kind.marker()).bold(),
        ActionKind::Delete => Paint::red(kind.marker()).bold(),
        ActionKind::DependencyChange => Paint::new(kind.marker()).dimmed(),
        ActionKind::DriftRecreate => Paint::red(kind.marker()).bold(),
        ActionKind::DriftUnknown => Paint::yellow(kind.marker()).bold(),
    }
}

fn pad(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_owned()
    } else {
        let mut out = String::from(s);
        out.extend(std::iter::repeat(' ').take(width - len));
        out
    }
}

fn strip_color(s: &str) -> String {
    // Simple ANSI escape stripper for the plain renderer's reuse of the
    // colored header; avoids a regex dependency.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            while let Some(&n) = chars.peek() {
                chars.next();
                if n == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn prompt_confirm() -> Decision {
    let prompt = format!(
        "{} {}: ",
        Paint::new("Apply this plan?").bold(),
        Paint::new("[y/N]").dimmed()
    );
    let _ = write!(io::stderr(), "{}", prompt);
    let _ = io::stderr().flush();

    let stdin = io::stdin();
    let mut line = String::new();
    match stdin.lock().read_line(&mut line) {
        Ok(0) | Err(_) => Decision::Cancel,
        Ok(_) => {
            let answer = line.trim().to_ascii_lowercase();
            if answer == "y" || answer == "yes" {
                Decision::Approve
            } else {
                eprintln!("{}", Paint::yellow("Cancelled."));
                Decision::Cancel
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::model::{Plan, PlanCounts, PlanRow};

    fn empty_plan() -> Plan {
        Plan::default()
    }

    fn plan_with_one_create() -> Plan {
        Plan {
            rows: vec![PlanRow {
                action: ActionKind::Create,
                resource_type: "Badge".into(),
                resource_id: "badge_welcome".into(),
                label: "Welcome".into(),
                summary: vec![],
                risk: RiskLevel::Safe,
            }],
            counts: PlanCounts {
                creates: 1,
                ..Default::default()
            },
        }
    }

    #[test]
    fn off_mode_approves_without_prompting() {
        let plan = plan_with_one_create();
        let decision = preview_and_confirm(
            &plan,
            PreviewOptions {
                mode: PreviewMode::Off,
                assume_yes: false,
            },
        );
        assert_eq!(decision, Decision::Approve);
    }

    #[test]
    fn no_changes_auto_approves() {
        let plan = empty_plan();
        let decision = preview_and_confirm(
            &plan,
            PreviewOptions {
                mode: PreviewMode::Plain,
                assume_yes: false,
            },
        );
        assert_eq!(decision, Decision::Approve);
    }

    #[test]
    fn non_interactive_with_changes_and_no_yes_auto_approves() {
        // PreviewMode::Plain forces the non-interactive code path. To keep
        // existing CI/scripted users working, deploy auto-approves in
        // non-interactive contexts (rather than hanging or cancelling).
        let plan = plan_with_one_create();
        let decision = preview_and_confirm(
            &plan,
            PreviewOptions {
                mode: PreviewMode::Plain,
                assume_yes: false,
            },
        );
        assert_eq!(decision, Decision::Approve);
    }

    #[test]
    fn non_interactive_with_assume_yes_approves() {
        let plan = plan_with_one_create();
        let decision = preview_and_confirm(
            &plan,
            PreviewOptions {
                mode: PreviewMode::Plain,
                assume_yes: true,
            },
        );
        assert_eq!(decision, Decision::Approve);
    }
}
