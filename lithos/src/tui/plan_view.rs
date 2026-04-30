//! Interactive plan viewer.
//!
//! Short-lived alt-screen TUI built on [`ratatui`]. Spawned only when both
//! [`super::interactive_enabled`] is true and the caller explicitly wants
//! the rich preview. The viewer:
//!
//! - shows the plan header with create/update/delete/drift counts,
//! - lists every row with an action marker, type, and label,
//! - lets the user expand a row to see the field-level summary,
//! - returns [`Decision::Approve`] / [`Decision::Cancel`] via keybinds.
//!
//! On any error setting up the terminal (e.g. a sandboxed CI runner that
//! claims to be a TTY but isn't), the viewer falls through to `Cancel`
//! after restoring terminal state; the caller is expected to render a
//! plain summary in that case.

use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::preview::model::{ActionKind, Plan, RiskLevel};
use crate::preview::render::Decision;

type Term = Terminal<CrosstermBackend<Stdout>>;

/// Run the interactive plan viewer to completion. Returns the user's
/// decision. On terminal setup failure, returns `Decision::Cancel` and the
/// caller is expected to fall back to plain rendering.
pub fn run(plan: &Plan) -> Decision {
    let mut terminal = match setup_terminal() {
        Ok(t) => t,
        Err(_) => return Decision::Cancel,
    };

    let result = event_loop(&mut terminal, plan);

    // Always restore the terminal, even if the loop errored or panicked
    // upstream of here.
    let _ = restore_terminal(&mut terminal);

    result.unwrap_or(Decision::Cancel)
}

fn setup_terminal() -> io::Result<Term> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

struct ViewState {
    selected: usize,
    expanded: bool,
    list_state: ListState,
}

impl ViewState {
    fn new(row_count: usize) -> Self {
        let mut list_state = ListState::default();
        if row_count > 0 {
            list_state.select(Some(0));
        }
        Self {
            selected: 0,
            expanded: false,
            list_state,
        }
    }

    fn move_down(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        self.selected = (self.selected + 1) % len;
        self.list_state.select(Some(self.selected));
        self.expanded = false;
    }

    fn move_up(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        self.selected = if self.selected == 0 {
            len - 1
        } else {
            self.selected - 1
        };
        self.list_state.select(Some(self.selected));
        self.expanded = false;
    }

    fn toggle_expand(&mut self) {
        self.expanded = !self.expanded;
    }
}

fn event_loop(terminal: &mut Term, plan: &Plan) -> io::Result<Decision> {
    let mut state = ViewState::new(plan.rows.len());
    // Tick at ~60fps so the focus highlight stays responsive without
    // burning CPU.
    let tick_rate = Duration::from_millis(16);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| draw(frame, plan, &mut state))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('n') => {
                        return Ok(Decision::Cancel)
                    }
                    KeyCode::Char('y') | KeyCode::Char('a') => return Ok(Decision::Approve),
                    KeyCode::Enter => state.toggle_expand(),
                    KeyCode::Down | KeyCode::Char('j') => state.move_down(plan.rows.len()),
                    KeyCode::Up | KeyCode::Char('k') => state.move_up(plan.rows.len()),
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn draw(frame: &mut Frame, plan: &Plan, state: &mut ViewState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(5),    // body
            Constraint::Length(3), // footer / hints
        ])
        .split(frame.area());

    draw_header(frame, plan, chunks[0]);
    draw_body(frame, plan, state, chunks[1]);
    draw_footer(frame, plan, chunks[2]);
}

fn draw_header(frame: &mut Frame, plan: &Plan, area: Rect) {
    let c = &plan.counts;
    let line = Line::from(vec![
        Span::styled(
            format!("+{} ", c.creates),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("~{} ", c.updates),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("-{} ", c.deletes),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("○{} ", c.dependency_changes),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(
            format!("!{} ", c.drift_recreate),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("?{}", c.drift_unknown),
            Style::default().fg(Color::Yellow),
        ),
    ]);

    let header = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " Lithos plan ",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(header, area);
}

fn draw_body(frame: &mut Frame, plan: &Plan, state: &mut ViewState, area: Rect) {
    if plan.rows.is_empty() {
        let p = Paragraph::new("No changes. Your infrastructure is up to date.")
            .style(Style::default().add_modifier(Modifier::DIM))
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        frame.render_widget(p, area);
        return;
    }

    if state.expanded {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(8)])
            .split(area);
        render_list(frame, plan, state, chunks[0]);
        render_detail(frame, plan, state, chunks[1]);
    } else {
        render_list(frame, plan, state, area);
    }
}

fn render_list(frame: &mut Frame, plan: &Plan, state: &mut ViewState, area: Rect) {
    let items: Vec<ListItem> = plan
        .rows
        .iter()
        .map(|row| {
            let (marker, color) = marker_style(row.action);
            let mut spans = vec![
                Span::styled(
                    format!("{} ", marker),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<24} ", truncate(&row.resource_type, 24)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(row.label.clone()),
            ];
            if let Some(label) = row.risk.label() {
                let style = match row.risk {
                    RiskLevel::Destructive => Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                    RiskLevel::Caution => Style::default().fg(Color::Yellow),
                    RiskLevel::Safe => Style::default(),
                };
                spans.push(Span::raw("  "));
                spans.push(Span::styled(format!("[{}]", label), style));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Changes "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut state.list_state);
}

fn render_detail(frame: &mut Frame, plan: &Plan, state: &ViewState, area: Rect) {
    let row = match plan.rows.get(state.selected) {
        Some(r) => r,
        None => return,
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("id: ", Style::default().add_modifier(Modifier::DIM)),
        Span::raw(row.resource_id.clone()),
    ]));
    if row.summary.is_empty() {
        lines.push(Line::from(Span::styled(
            "no field-level details",
            Style::default().add_modifier(Modifier::DIM),
        )));
    } else {
        for s in &row.summary {
            lines.push(Line::from(s.clone()));
        }
    }

    let title = format!(" Details: {} ", row.label);
    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

fn draw_footer(frame: &mut Frame, plan: &Plan, area: Rect) {
    let destructive = plan.counts.deletes > 0 || plan.counts.drift_recreate > 0;
    let mut spans = vec![
        Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
        Span::raw(" move  "),
        Span::styled("enter", Style::default().fg(Color::Cyan)),
        Span::raw(" expand  "),
        Span::styled("y", Style::default().fg(Color::Green)),
        Span::raw(" apply  "),
        Span::styled("n/q/esc", Style::default().fg(Color::Red)),
        Span::raw(" cancel"),
    ];
    if destructive {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            "⚠ destructive changes present",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    let p = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title(" Keys "));
    frame.render_widget(p, area);
}

fn marker_style(kind: ActionKind) -> (&'static str, Color) {
    match kind {
        ActionKind::Create => ("+", Color::Green),
        ActionKind::Update => ("~", Color::Yellow),
        ActionKind::Delete => ("-", Color::Red),
        ActionKind::DependencyChange => ("○", Color::Gray),
        ActionKind::DriftRecreate => ("!", Color::Red),
        ActionKind::DriftUnknown => ("?", Color::Yellow),
    }
}

fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
