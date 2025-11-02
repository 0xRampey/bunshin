use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::time::Duration;

pub struct OverlayState {
    pub session_name: String,
    pub worktree_path: String,
    pub branch_name: String,
    pub agent_count: usize,
    pub active: bool,
}

impl OverlayState {
    pub fn new(session_name: String, worktree_path: String, branch_name: String) -> Self {
        Self {
            session_name,
            worktree_path,
            branch_name,
            agent_count: 0,
            active: false,
        }
    }
}

/// Enter the overlay UI in alternate screen mode
pub fn enter_overlay_ui(state: &OverlayState) -> anyhow::Result<bool> {
    // Switch to alt screen
    execute!(io::stdout(), EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let result = run_overlay_ui(&mut terminal, state)?;

    // Clean up
    terminal.show_cursor()?;
    terminal::disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    Ok(result)
}

/// Run the overlay UI loop
fn run_overlay_ui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &OverlayState,
) -> anyhow::Result<bool> {
    loop {
        terminal.draw(|f| draw_overlay(f, state))?;

        // Poll for input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    // Ctrl-~ or Esc to exit overlay
                    KeyCode::Char('~') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(false); // Don't quit session
                    }
                    KeyCode::Esc => {
                        return Ok(false); // Don't quit session
                    }
                    // q to quit session
                    KeyCode::Char('q') => {
                        return Ok(true); // Quit session
                    }
                    // h for help (do nothing for now)
                    KeyCode::Char('h') => {
                        // TODO: Show help screen
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Draw the overlay UI
fn draw_overlay(f: &mut Frame, state: &OverlayState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(7),  // Session info
            Constraint::Min(5),     // Agent list
            Constraint::Length(3),  // Footer/Help
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("🔧 ", Style::default().fg(Color::Cyan)),
        Span::styled("Bunshin Session Overlay", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)))
    .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    // Session info
    let info_text = vec![
        Line::from(vec![
            Span::styled("Session:  ", Style::default().fg(Color::Yellow)),
            Span::raw(&state.session_name),
        ]),
        Line::from(vec![
            Span::styled("Branch:   ", Style::default().fg(Color::Yellow)),
            Span::raw(&state.branch_name),
        ]),
        Line::from(vec![
            Span::styled("Worktree: ", Style::default().fg(Color::Yellow)),
            Span::raw(&state.worktree_path),
        ]),
        Line::from(vec![
            Span::styled("Agents:   ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{} active", state.agent_count)),
        ]),
    ];

    let session_info = Paragraph::new(info_text)
        .block(Block::default().borders(Borders::ALL).title("Session Info").border_style(Style::default().fg(Color::Green)))
        .wrap(Wrap { trim: false });
    f.render_widget(session_info, chunks[1]);

    // Agent list (placeholder)
    let agents: Vec<ListItem> = vec![
        ListItem::new("● Claude Code - Running in worktree").style(Style::default().fg(Color::Green)),
    ];

    let agent_list = List::new(agents)
        .block(Block::default().borders(Borders::ALL).title("Active Agents").border_style(Style::default().fg(Color::Magenta)));
    f.render_widget(agent_list, chunks[2]);

    // Help footer
    let help = Paragraph::new("Ctrl-~ / Esc: Close Overlay  |  q: Quit Session  |  h: Help")
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray));
    f.render_widget(help, chunks[3]);
}

/// Draw a status bar at the bottom of the terminal (alternative approach)
pub fn draw_status_bar(state: &OverlayState, rows: u16) -> anyhow::Result<()> {
    use crossterm::{cursor, style::Print, QueueableCommand};
    use std::io::Write;

    let status = format!(
        " {} | {} | agents: {} ",
        state.session_name,
        state.branch_name,
        state.agent_count
    );

    let mut out = io::stdout();
    out.queue(cursor::SavePosition)?;
    out.queue(cursor::MoveTo(0, rows - 1))?;
    out.queue(Print("\x1b[2K"))?; // Clear line
    out.queue(Print(status))?;
    out.queue(cursor::RestorePosition)?;
    out.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_state_creation() {
        let state = OverlayState::new(
            "test-session".to_string(),
            "/tmp/worktree".to_string(),
            "main".to_string(),
        );
        assert_eq!(state.session_name, "test-session");
        assert_eq!(state.branch_name, "main");
        assert_eq!(state.agent_count, 0);
    }
}
