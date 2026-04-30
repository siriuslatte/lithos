//! Branded CLI chrome: banner, panels, and small layout helpers.
//!
//! All decorative output goes to stderr (matching `logger`) so it never
//! pollutes machine-readable stdout from commands like `outputs` or
//! `diff --format json`. Color and Unicode box drawing are suppressed when
//! stderr is not a terminal so CI logs stay clean.

use std::io::{self, IsTerminal};

use clap::crate_version;
use yansi::{Color, Paint, Style};

/// Whether the decorative chrome should be rendered. Centralized so every
/// caller agrees on the policy.
pub fn chrome_enabled() -> bool {
    io::stderr().is_terminal()
}

/// Print the branded CLI banner. Shown once per invocation, before any
/// command logic, so users always see the tool name and version.
pub fn print_banner(subcommand: Option<&str>) {
    if !chrome_enabled() {
        return;
    }
    let title = format!("Lithos {}", crate_version!());
    let tagline = "Infra-as-code for Roblox";
    let cmd_line = subcommand.map(|s| format!("› {}", s));

    let mut lines: Vec<(String, Style)> = Vec::new();
    lines.push((title, Style::new(Color::Cyan).bold()));
    lines.push((tagline.to_owned(), Style::new(Color::Default).dimmed()));
    if let Some(c) = cmd_line {
        lines.push((c, Style::new(Color::Magenta)));
    }

    let inner_width = lines
        .iter()
        .map(|(s, _)| s.chars().count())
        .max()
        .unwrap_or(0)
        .max(28);

    let horiz: String = "─".repeat(inner_width + 2);
    let border = Style::new(Color::Cyan).dimmed();

    eprintln!();
    eprintln!(
        "{}{}{}",
        Paint::new("╭").with_style(border),
        Paint::new(&horiz).with_style(border),
        Paint::new("╮").with_style(border)
    );
    for (text, style) in &lines {
        let padding = " ".repeat(inner_width - text.chars().count());
        eprintln!(
            "{} {}{} {}",
            Paint::new("│").with_style(border),
            Paint::new(text).with_style(*style),
            padding,
            Paint::new("│").with_style(border),
        );
    }
    eprintln!(
        "{}{}{}",
        Paint::new("╰").with_style(border),
        Paint::new(&horiz).with_style(border),
        Paint::new("╯").with_style(border)
    );
    eprintln!();
}

/// Print a bordered panel with the given title and pre-rendered body
/// lines. The body is responsible for its own coloring; the panel just
/// adds chrome and a heading. When `chrome_enabled()` is false, falls back
/// to a plain title + body block.
pub fn print_panel(title: &str, body: &[String]) {
    if !chrome_enabled() {
        eprintln!("{}", title);
        for line in body {
            eprintln!("  {}", line);
        }
        return;
    }

    let inner_width = body
        .iter()
        .map(|l| visible_width(l))
        .chain(std::iter::once(title.chars().count() + 2))
        .max()
        .unwrap_or(0)
        .max(40);

    let border = Style::new(Color::Cyan).dimmed();
    let horiz: String = "─".repeat(inner_width + 2);

    // Top border with embedded title
    let title_styled = format!(
        "{}{}{}",
        Paint::new("┤ ").with_style(border),
        Paint::new(title).with_style(Style::new(Color::Cyan).bold()),
        Paint::new(" ├").with_style(border),
    );
    let title_width = visible_width(&title_styled);
    let remaining = (inner_width + 4).saturating_sub(title_width + 2);
    eprintln!(
        "{}{}{}{}",
        Paint::new("╭─").with_style(border),
        title_styled,
        Paint::new("─".repeat(remaining)).with_style(border),
        Paint::new("╮").with_style(border),
    );

    for line in body {
        let pad = inner_width.saturating_sub(visible_width(line));
        eprintln!(
            "{} {}{} {}",
            Paint::new("│").with_style(border),
            line,
            " ".repeat(pad),
            Paint::new("│").with_style(border),
        );
    }
    eprintln!(
        "{}{}{}",
        Paint::new("╰").with_style(border),
        Paint::new(&horiz).with_style(border),
        Paint::new("╯").with_style(border),
    );
}

/// Visible width of a string, ignoring ANSI escape sequences. Used so the
/// panel borders line up regardless of how many color codes a line carries.
pub fn visible_width(s: &str) -> usize {
    let mut count = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Skip ANSI escape: ESC [ ... m
            while let Some(&n) = chars.peek() {
                chars.next();
                if n == 'm' {
                    break;
                }
            }
        } else {
            count += 1;
        }
    }
    count
}
