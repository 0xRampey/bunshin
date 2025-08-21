use crate::session::{Session, SessionManager};
use crate::git::GitWorktree;
use crate::shell::ShellManager;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
    },
    Frame, Terminal,
};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum AppState {
    SessionList,
    CreateSession,
    SessionDetails,
}

pub struct App {
    pub session_manager: SessionManager,
    pub shell_manager: ShellManager,
    pub state: AppState,
    pub selected_session: usize,
    pub session_list_state: ListState,
    pub config_path: PathBuf,
    pub create_session_form: CreateSessionForm,
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CreateSessionForm {
    pub name: String,
    pub repo_path: String,
    pub branch: String,
    pub current_field: usize,
    pub available_branches: Vec<String>,
}

impl App {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("bunshin");
        
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("sessions.json");
        
        let session_manager = SessionManager::load_from_file(&config_path)?;
        
        Ok(Self {
            session_manager,
            shell_manager: ShellManager::new(),
            state: AppState::SessionList,
            selected_session: 0,
            session_list_state: ListState::default(),
            config_path,
            create_session_form: CreateSessionForm::default(),
            status_message: None,
        })
    }

    pub fn save_sessions(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.session_manager.save_to_file(&self.config_path)
    }

    pub fn next_session(&mut self) {
        if !self.session_manager.sessions.is_empty() {
            self.selected_session = (self.selected_session + 1) % self.session_manager.sessions.len();
            self.session_list_state.select(Some(self.selected_session));
        }
    }

    pub fn previous_session(&mut self) {
        if !self.session_manager.sessions.is_empty() {
            self.selected_session = if self.selected_session == 0 {
                self.session_manager.sessions.len() - 1
            } else {
                self.selected_session - 1
            };
            self.session_list_state.select(Some(self.selected_session));
        }
    }

    pub fn get_selected_session(&self) -> Option<&Session> {
        self.session_manager.sessions.get(self.selected_session)
    }
}

pub fn draw_sessions_list(f: &mut Frame, app: &App, area: Rect) {
    let constraints = if app.status_message.is_some() {
        [Constraint::Min(3), Constraint::Length(3), Constraint::Length(3)]
    } else {
        [Constraint::Min(3), Constraint::Length(3), Constraint::Length(0)]
    };
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints.as_ref())
        .split(area);

    let sessions: Vec<ListItem> = app
        .session_manager
        .sessions
        .iter()
        .enumerate()
        .map(|(_i, session)| {
            let claude_status = if session.is_active() { "●" } else { "○" };
            let has_shell = app.shell_manager.shells.contains_key(&session.branch) &&
                           app.shell_manager.shells[&session.branch].is_running();
            let shell_status = if has_shell { "⚡" } else { " " };
            
            let content = Line::from(vec![
                Span::styled(
                    format!("{} ", claude_status),
                    Style::default().fg(if session.is_active() { Color::Green } else { Color::Gray }),
                ),
                Span::styled(
                    format!("{} ", shell_status),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(&session.name, Style::default().fg(Color::White)),
                Span::styled(
                    format!(" ({})", session.branch),
                    Style::default().fg(Color::Yellow),
                ),
            ]);
            ListItem::new(content)
        })
        .collect();

    let sessions_list = List::new(sessions)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Sessions")
                .border_style(Style::default().fg(Color::White)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut list_state = app.session_list_state.clone();
    f.render_stateful_widget(sessions_list, chunks[0], &mut list_state);

    let help = Paragraph::new("↑/↓: Navigate | Enter: Attach Session | c: Launch Claude | n: New | d: Delete | q: Quit")
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true });
    f.render_widget(help, chunks[1]);

    if let Some(ref message) = app.status_message {
        let status = Paragraph::new(message.clone())
            .block(Block::default().borders(Borders::ALL).title("Status"))
            .style(Style::default().fg(Color::Yellow))
            .wrap(Wrap { trim: true });
        f.render_widget(status, chunks[2]);
    }
}

pub fn draw_create_session(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(60, 50, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(popup_area);

    let form = &app.create_session_form;

    let name_block = Block::default()
        .borders(Borders::ALL)
        .title("Session Name")
        .border_style(if form.current_field == 0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let name_input = Paragraph::new(form.name.as_str()).block(name_block);
    f.render_widget(name_input, chunks[0]);

    let repo_block = Block::default()
        .borders(Borders::ALL)
        .title("Repository Path")
        .border_style(if form.current_field == 1 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let repo_input = Paragraph::new(form.repo_path.as_str()).block(repo_block);
    f.render_widget(repo_input, chunks[1]);

    let branch_block = Block::default()
        .borders(Borders::ALL)
        .title("Branch (will be created if doesn't exist)")
        .border_style(if form.current_field == 2 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let branch_input = Paragraph::new(form.branch.as_str()).block(branch_block);
    f.render_widget(branch_input, chunks[2]);

    let help_text = if form.name.is_empty() || form.repo_path.is_empty() || form.branch.is_empty() {
        "Fill all fields | Tab: Next Field | Enter: Create | Esc: Cancel"
    } else {
        "Tab: Next Field | Enter: Create | Esc: Cancel"
    };
    
    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .alignment(Alignment::Center);
    f.render_widget(help, chunks[3]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}