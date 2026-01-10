use std::collections::{BTreeMap, HashMap};
use zellij_tile::prelude::*;

const REPO_QUERY_CONTEXT_KEY: &str = "repo_query_session";
const LOAD_SESSIONS_CONTEXT_KEY: &str = "load_sessions";
const SESSIONS_FILE: &str = ".bunshin/sessions.json";

#[derive(Default)]
struct State {
    sessions: Vec<SessionInfo>,
    selected_index: usize,
    mode: Mode,
    colors: Styling,
    show_help: bool,
    new_session_name: Option<String>,
    rename_input: Option<String>,
    error_message: Option<String>,
    // Repo grouping
    session_repos: HashMap<String, String>, // session_name -> repo_name
    pending_repo_query: Option<String>,     // session name waiting for git result
    current_repo: Option<String>,           // repo detected at plugin load
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    List,
    Create,
    Rename,
    ConfirmKill,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::List
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        subscribe(&[
            EventType::Key,
            EventType::SessionUpdate,
            EventType::ModeUpdate,
            EventType::RunCommandResult,
        ]);
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::OpenTerminalsOrPlugins,
            PermissionType::RunCommands,
        ]);

        // Detect current repo at plugin load time
        self.detect_current_repo();

        // Load persisted session->repo mappings
        self.load_session_repos();
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::Key(key) => {
                should_render = self.handle_key(key);
            }
            Event::SessionUpdate(sessions, _dead_sessions) => {
                self.sessions = sessions;
                // Clamp selected index to valid range
                if !self.sessions.is_empty() && self.selected_index >= self.sessions.len() {
                    self.selected_index = self.sessions.len() - 1;
                }
                should_render = true;
            }
            Event::ModeUpdate(mode_info) => {
                self.colors = mode_info.style.colors;
                should_render = true;
            }
            Event::RunCommandResult(exit_code, stdout, _stderr, context) => {
                should_render = self.handle_run_command_result(exit_code, stdout, context);
            }
            _ => {}
        }
        should_render
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if self.show_help {
            self.render_help(rows, cols);
        } else {
            match self.mode {
                Mode::List => self.render_session_list(rows, cols),
                Mode::Create => self.render_create_session(rows, cols),
                Mode::Rename => self.render_rename_session(rows, cols),
                Mode::ConfirmKill => self.render_confirm_kill(rows, cols),
            }
        }
    }
}

impl State {
    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        // Clear error message on any key press
        if self.error_message.is_some() {
            self.error_message = None;
            return true;
        }

        if self.show_help {
            return self.handle_help_key(key);
        }

        match self.mode {
            Mode::List => self.handle_list_key(key),
            Mode::Create => self.handle_create_key(key),
            Mode::Rename => self.handle_rename_key(key),
            Mode::ConfirmKill => self.handle_confirm_kill_key(key),
        }
    }

    fn handle_help_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Char('?') | BareKey::Char('q') | BareKey::Esc => {
                self.show_help = false;
                true
            }
            _ => false,
        }
    }

    fn handle_list_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            // Navigation
            BareKey::Down | BareKey::Char('j') if key.has_no_modifiers() => {
                if !self.sessions.is_empty() {
                    self.selected_index = (self.selected_index + 1) % self.sessions.len();
                }
                true
            }
            BareKey::Up | BareKey::Char('k') if key.has_no_modifiers() => {
                if !self.sessions.is_empty() {
                    self.selected_index = if self.selected_index == 0 {
                        self.sessions.len() - 1
                    } else {
                        self.selected_index - 1
                    };
                }
                true
            }
            BareKey::Home | BareKey::Char('g') if key.has_no_modifiers() => {
                self.selected_index = 0;
                true
            }
            BareKey::End | BareKey::Char('G') if key.has_no_modifiers() => {
                if !self.sessions.is_empty() {
                    self.selected_index = self.sessions.len() - 1;
                }
                true
            }

            // Session actions
            BareKey::Enter => {
                self.switch_to_selected_session();
                true
            }
            BareKey::Char('c') if key.has_no_modifiers() => {
                self.mode = Mode::Create;
                self.new_session_name = Some(String::new());
                true
            }
            BareKey::Char('$') if key.has_no_modifiers() => {
                if self.is_current_session_selected() {
                    self.mode = Mode::Rename;
                    self.rename_input = Some(String::new());
                    true
                } else {
                    self.error_message = Some("Can only rename current session".to_string());
                    true
                }
            }
            BareKey::Char('x') if key.has_no_modifiers() => {
                if !self.is_current_session_selected() {
                    self.mode = Mode::ConfirmKill;
                    true
                } else {
                    self.error_message = Some("Cannot kill current session".to_string());
                    true
                }
            }
            BareKey::Char('d') if key.has_no_modifiers() => {
                detach();
                false
            }
            BareKey::Char('(') if key.has_no_modifiers() => {
                self.switch_to_previous_session();
                true
            }
            BareKey::Char(')') if key.has_no_modifiers() => {
                self.switch_to_next_session();
                true
            }

            // Claude Code orchestration
            BareKey::Char('C') if key.has_no_modifiers() => {
                self.launch_claude_pane();
                hide_self();
                false
            }
            BareKey::Char('A') if key.has_no_modifiers() => {
                self.launch_claude_tab();
                hide_self();
                false
            }
            BareKey::Char('N') if key.has_no_modifiers() => {
                self.create_claude_session();
                hide_self();
                false
            }

            // UI
            BareKey::Char('?') if key.has_no_modifiers() => {
                self.show_help = true;
                true
            }
            BareKey::Char('q') | BareKey::Esc if key.has_no_modifiers() => {
                hide_self();
                false
            }
            _ => false,
        }
    }

    fn handle_create_key(&mut self, key: KeyWithModifier) -> bool {
        if let Some(ref mut name) = self.new_session_name {
            match key.bare_key {
                BareKey::Char(c) if key.has_no_modifiers() => {
                    if c != '\n' {
                        name.push(c);
                    } else {
                        return self.create_session();
                    }
                    true
                }
                BareKey::Backspace if key.has_no_modifiers() => {
                    name.pop();
                    true
                }
                BareKey::Enter => self.create_session(),
                BareKey::Esc if key.has_no_modifiers() => {
                    self.mode = Mode::List;
                    self.new_session_name = None;
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    fn handle_rename_key(&mut self, key: KeyWithModifier) -> bool {
        if let Some(ref mut name) = self.rename_input {
            match key.bare_key {
                BareKey::Char(c) if key.has_no_modifiers() => {
                    if c != '\n' {
                        name.push(c);
                    } else {
                        return self.rename_session();
                    }
                    true
                }
                BareKey::Backspace if key.has_no_modifiers() => {
                    name.pop();
                    true
                }
                BareKey::Enter => self.rename_session(),
                BareKey::Esc if key.has_no_modifiers() => {
                    self.mode = Mode::List;
                    self.rename_input = None;
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    fn handle_confirm_kill_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Char('y') | BareKey::Char('Y') if key.has_no_modifiers() => {
                self.kill_selected_session();
                self.mode = Mode::List;
                true
            }
            BareKey::Char('n') | BareKey::Char('N') | BareKey::Esc if key.has_no_modifiers() => {
                self.mode = Mode::List;
                true
            }
            _ => false,
        }
    }

    fn switch_to_selected_session(&mut self) {
        if let Some(session) = self.sessions.get(self.selected_index) {
            if !session.is_current_session {
                switch_session(Some(&session.name));
                hide_self(); // Close plugin after switching
            }
        }
    }

    fn switch_to_previous_session(&mut self) {
        if self.sessions.len() > 1 {
            let current_idx = self
                .sessions
                .iter()
                .position(|s| s.is_current_session)
                .unwrap_or(0);
            let prev_idx = if current_idx == 0 {
                self.sessions.len() - 1
            } else {
                current_idx - 1
            };
            if let Some(session) = self.sessions.get(prev_idx) {
                switch_session(Some(&session.name));
            }
        }
    }

    fn switch_to_next_session(&mut self) {
        if self.sessions.len() > 1 {
            let current_idx = self
                .sessions
                .iter()
                .position(|s| s.is_current_session)
                .unwrap_or(0);
            let next_idx = (current_idx + 1) % self.sessions.len();
            if let Some(session) = self.sessions.get(next_idx) {
                switch_session(Some(&session.name));
            }
        }
    }

    fn create_session(&mut self) -> bool {
        if let Some(name) = &self.new_session_name.clone() {
            if name.is_empty() {
                self.error_message = Some("Session name cannot be empty".to_string());
                self.mode = Mode::List;
                self.new_session_name = None;
                return true;
            }
            if name.contains('/') {
                self.error_message = Some("Session name cannot contain '/'".to_string());
                self.mode = Mode::List;
                self.new_session_name = None;
                return true;
            }
            if name.len() >= 108 {
                self.error_message = Some("Session name too long (max 107 chars)".to_string());
                self.mode = Mode::List;
                self.new_session_name = None;
                return true;
            }

            // Register repo for this session before creating it
            self.register_session_repo(name);

            switch_session(Some(name));
            self.mode = Mode::List;
            self.new_session_name = None;
            hide_self();
        }
        true
    }

    fn rename_session(&mut self) -> bool {
        if let Some(name) = &self.rename_input {
            if name.is_empty() {
                self.error_message = Some("Session name cannot be empty".to_string());
                self.mode = Mode::List;
                self.rename_input = None;
                return true;
            }
            if name.contains('/') {
                self.error_message = Some("Session name cannot contain '/'".to_string());
                self.mode = Mode::List;
                self.rename_input = None;
                return true;
            }
            if name.len() >= 108 {
                self.error_message = Some("Session name too long (max 107 chars)".to_string());
                self.mode = Mode::List;
                self.rename_input = None;
                return true;
            }

            rename_session(name);
            self.mode = Mode::List;
            self.rename_input = None;
        }
        true
    }

    fn kill_selected_session(&mut self) {
        if let Some(session) = self.sessions.get(self.selected_index) {
            if !session.is_current_session {
                kill_sessions(&[session.name.clone()]);
                // Adjust selected index if needed
                if self.selected_index > 0 && self.selected_index >= self.sessions.len() - 1 {
                    self.selected_index -= 1;
                }
            }
        }
    }

    fn is_current_session_selected(&self) -> bool {
        self.sessions
            .get(self.selected_index)
            .map(|s| s.is_current_session)
            .unwrap_or(false)
    }

    // Repo detection functions
    fn detect_current_repo(&mut self) {
        let mut context = BTreeMap::new();
        context.insert(REPO_QUERY_CONTEXT_KEY.to_string(), "__current__".to_string());
        run_command(
            &["git", "rev-parse", "--show-toplevel"],
            context,
        );
    }

    fn query_repo_for_session(&mut self, session_name: &str) {
        self.pending_repo_query = Some(session_name.to_string());
        let mut context = BTreeMap::new();
        context.insert(REPO_QUERY_CONTEXT_KEY.to_string(), session_name.to_string());
        run_command(
            &["git", "rev-parse", "--show-toplevel"],
            context,
        );
    }

    fn handle_run_command_result(
        &mut self,
        exit_code: Option<i32>,
        stdout: Vec<u8>,
        context: BTreeMap<String, String>,
    ) -> bool {
        // Check if this is a load sessions result
        if context.get(LOAD_SESSIONS_CONTEXT_KEY).is_some() {
            if exit_code == Some(0) {
                let json = String::from_utf8_lossy(&stdout).to_string();
                self.parse_session_repos_json(&json);
                return true;
            }
            // File doesn't exist yet, that's fine
            return false;
        }

        // Check if this is a repo query result
        if let Some(session_name) = context.get(REPO_QUERY_CONTEXT_KEY) {
            if exit_code == Some(0) {
                let repo_path = String::from_utf8_lossy(&stdout).trim().to_string();
                let repo_name = self.extract_repo_name(&repo_path);

                if session_name == "__current__" {
                    // This is the initial repo detection at plugin load
                    self.current_repo = Some(repo_name);
                } else {
                    // This is a session-specific repo query
                    self.session_repos.insert(session_name.clone(), repo_name);
                    if self.pending_repo_query.as_ref() == Some(session_name) {
                        self.pending_repo_query = None;
                    }
                    // Persist the updated mappings
                    self.persist_session_repos();
                }
                return true;
            } else {
                // Not in a git repo, use "Other" or current directory name
                if session_name == "__current__" {
                    self.current_repo = None;
                }
                if self.pending_repo_query.as_ref() == Some(session_name) {
                    self.pending_repo_query = None;
                }
            }
        }
        false
    }

    fn extract_repo_name(&self, repo_path: &str) -> String {
        std::path::Path::new(repo_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn register_session_repo(&mut self, session_name: &str) {
        // If we know the current repo, associate it with this session
        if let Some(ref repo) = self.current_repo {
            self.session_repos.insert(session_name.to_string(), repo.clone());
            // Persist to file
            self.persist_session_repos();
        } else {
            // Query git for this session
            self.query_repo_for_session(session_name);
        }
    }

    fn load_session_repos(&self) {
        // Load session->repo mappings from file using cat command
        let mut context = BTreeMap::new();
        context.insert(LOAD_SESSIONS_CONTEXT_KEY.to_string(), "true".to_string());
        run_command(
            &["cat", &format!("$HOME/{}", SESSIONS_FILE)],
            context,
        );
    }

    fn persist_session_repos(&self) {
        // Convert HashMap to simple JSON format
        let mut json_parts: Vec<String> = Vec::new();
        for (session, repo) in &self.session_repos {
            // Escape any quotes in session/repo names
            let escaped_session = session.replace('\\', "\\\\").replace('"', "\\\"");
            let escaped_repo = repo.replace('\\', "\\\\").replace('"', "\\\"");
            json_parts.push(format!("\"{}\":\"{}\"", escaped_session, escaped_repo));
        }
        let json = format!("{{{}}}", json_parts.join(","));

        // Write to file using shell
        // First ensure directory exists, then write the file
        let cmd = format!(
            "mkdir -p $HOME/.bunshin && echo '{}' > $HOME/{}",
            json, SESSIONS_FILE
        );
        run_command(&["sh", "-c", &cmd], BTreeMap::new());
    }

    fn parse_session_repos_json(&mut self, json: &str) {
        // Simple JSON parser for {"key":"value",...} format
        let json = json.trim();
        if !json.starts_with('{') || !json.ends_with('}') {
            return;
        }
        let inner = &json[1..json.len() - 1];
        if inner.is_empty() {
            return;
        }

        // Split by comma, but be careful about commas inside strings
        let mut pairs = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        let mut escape_next = false;

        for c in inner.chars() {
            if escape_next {
                current.push(c);
                escape_next = false;
                continue;
            }
            match c {
                '\\' => {
                    escape_next = true;
                    current.push(c);
                }
                '"' => {
                    in_string = !in_string;
                    current.push(c);
                }
                ',' if !in_string => {
                    pairs.push(current.trim().to_string());
                    current = String::new();
                }
                _ => current.push(c),
            }
        }
        if !current.is_empty() {
            pairs.push(current.trim().to_string());
        }

        // Parse each "key":"value" pair
        for pair in pairs {
            if let Some(colon_pos) = pair.find(':') {
                let key = pair[..colon_pos].trim();
                let value = pair[colon_pos + 1..].trim();

                // Remove quotes
                if key.len() >= 2 && value.len() >= 2 {
                    let key = &key[1..key.len() - 1];
                    let value = &value[1..value.len() - 1];
                    // Unescape
                    let key = key.replace("\\\"", "\"").replace("\\\\", "\\");
                    let value = value.replace("\\\"", "\"").replace("\\\\", "\\");
                    self.session_repos.insert(key, value);
                }
            }
        }
    }

    fn launch_claude_pane(&self) {
        // Launch Claude Code in a new pane in the current session
        let command = CommandToRun {
            path: "claude".into(),
            args: vec![],
            cwd: None,
        };
        let context = BTreeMap::new();
        open_command_pane(command, context);
    }

    fn launch_claude_tab(&self) {
        // Launch Claude Code in a new tab
        let command = CommandToRun {
            path: "claude".into(),
            args: vec![],
            cwd: None,
        };
        // First create a new tab
        new_tab(Some("Claude"), None::<&str>);
        // Then open the command in it
        let context = BTreeMap::new();
        open_command_pane(command, context);
    }

    fn create_claude_session(&mut self) {
        // Create a new session with Claude Code auto-started
        let session_name = format!("claude-{}", chrono::Utc::now().timestamp());

        // Register repo for this session before creating it
        self.register_session_repo(&session_name);

        // Create the session first
        switch_session(Some(&session_name));

        // Then launch Claude in it
        let command = CommandToRun {
            path: "claude".into(),
            args: vec![],
            cwd: None,
        };
        let context = BTreeMap::new();
        open_command_pane(command, context);
    }

    /// Groups sessions by their associated repo name
    fn get_grouped_sessions(&self) -> Vec<(String, Vec<&SessionInfo>)> {
        let mut groups: HashMap<String, Vec<&SessionInfo>> = HashMap::new();

        for session in &self.sessions {
            let repo = self
                .session_repos
                .get(&session.name)
                .cloned()
                .unwrap_or_else(|| "Other".to_string());
            groups.entry(repo).or_default().push(session);
        }

        // Convert to sorted vec (alphabetically by repo name, "Other" last)
        let mut result: Vec<_> = groups.into_iter().collect();
        result.sort_by(|a, b| {
            if a.0 == "Other" {
                std::cmp::Ordering::Greater
            } else if b.0 == "Other" {
                std::cmp::Ordering::Less
            } else {
                a.0.cmp(&b.0)
            }
        });
        result
    }

    fn render_session_list(&self, rows: usize, cols: usize) {
        if rows < 5 || cols < 40 {
            print_text(Text::new("Terminal too small"));
            return;
        }

        // Title
        let title = "Bunshin - Claude Code Orchestrator";
        let title_text = Text::new(title).color_range(3, 0..title.len());
        print_text_with_coordinates(
            title_text,
            (cols.saturating_sub(title.len())) / 2,
            1,
            None,
            None,
        );

        // If no sessions, show message
        if self.sessions.is_empty() {
            let message = "No sessions found. Loading...";
            print_text_with_coordinates(
                Text::new(message),
                (cols.saturating_sub(message.len())) / 2,
                rows / 2,
                None,
                None,
            );
            return;
        }

        // Separator after title
        let separator = "─".repeat(cols.saturating_sub(4));
        print_text_with_coordinates(Text::new(&separator), 2, 3, None, None);

        // Get grouped sessions
        let grouped = self.get_grouped_sessions();

        // Calculate display metrics
        let list_start_y = 4;
        let max_visible_lines = rows.saturating_sub(list_start_y + 3);
        let name_col = 2;
        let windows_col = cols.saturating_sub(40);

        // Build a flat list of display items (headers and sessions)
        // Each item is either a header (None) or a session (Some(global_idx))
        let mut display_items: Vec<(bool, Option<usize>, String, Option<&SessionInfo>)> = Vec::new();
        let mut global_idx = 0;

        for (repo, sessions) in &grouped {
            // Add repo header
            let header = format!("▼ {} ({})", repo, sessions.len());
            display_items.push((true, None, header, None));

            // Add sessions under this repo
            for session in sessions {
                display_items.push((false, Some(global_idx), session.name.clone(), Some(*session)));
                global_idx += 1;
            }
        }

        // Calculate scroll offset based on selected_index
        // We need to find which display item corresponds to selected_index
        let mut selected_display_idx = 0;
        for (i, item) in display_items.iter().enumerate() {
            if item.1 == Some(self.selected_index) {
                selected_display_idx = i;
                break;
            }
        }

        let start_display_idx = if selected_display_idx >= max_visible_lines {
            selected_display_idx.saturating_sub(max_visible_lines - 1)
        } else {
            0
        };
        let end_display_idx = (start_display_idx + max_visible_lines).min(display_items.len());

        // Render visible items
        let mut current_row = list_start_y;
        for display_idx in start_display_idx..end_display_idx {
            let (is_header, session_idx, ref label, session_opt) = &display_items[display_idx];

            if *is_header {
                // Render repo header
                let header_text = Text::new(label).color_range(3, 0..label.len());
                print_text_with_coordinates(header_text, name_col, current_row, None, None);
            } else if let Some(session) = session_opt {
                // Render session row
                let is_selected = *session_idx == Some(self.selected_index);
                let is_current = session.is_current_session;

                let session_indicator = if is_current { "*" } else { " " };
                let name_display = format!("  {} {}", session_indicator, session.name);

                let windows_count = session.tabs.len();
                let panes_count: usize = session.panes.panes.len();
                let clients_count = session.connected_clients;

                let line = format!(
                    "{:<width1$}  {:<width2$}  {:<width3$}  {:<width4$}",
                    name_display,
                    windows_count,
                    panes_count,
                    clients_count,
                    width1 = windows_col.saturating_sub(name_col + 2).max(10),
                    width2 = 7,
                    width3 = 5,
                    width4 = 7,
                );

                let mut text = Text::new(&line);
                if is_selected {
                    text = text.selected();
                }
                if is_current {
                    text = text.color_range(2, 0..name_display.len());
                }

                print_text_with_coordinates(text, name_col, current_row, None, None);
            }

            current_row += 1;
        }

        // Status line
        self.render_status_line(rows, cols);

        // Error message
        if let Some(ref error) = self.error_message {
            self.render_error(error, rows, cols);
        }
    }

    fn render_create_session(&self, rows: usize, cols: usize) {
        if rows < 10 || cols < 50 {
            print_text(Text::new("Terminal too small"));
            return;
        }

        let box_width = 50.min(cols.saturating_sub(4));
        let box_height = 7;
        let box_x = (cols.saturating_sub(box_width)) / 2;
        let box_y = (rows.saturating_sub(box_height)) / 2;

        // Title
        let title = " Create New Session ";
        print_text_with_coordinates(
            Text::new(title).color_range(3, 0..title.len()),
            box_x + (box_width.saturating_sub(title.len())) / 2,
            box_y,
            None,
            None,
        );

        // Border
        let top_border = format!("┌{}┐", "─".repeat(box_width.saturating_sub(2)));
        let bottom_border = format!("└{}┘", "─".repeat(box_width.saturating_sub(2)));
        print_text_with_coordinates(Text::new(&top_border), box_x, box_y + 1, None, None);
        print_text_with_coordinates(Text::new(&bottom_border), box_x, box_y + box_height - 1, None, None);

        // Sides
        for i in 2..box_height - 1 {
            print_text_with_coordinates(Text::new("│"), box_x, box_y + i, None, None);
            print_text_with_coordinates(
                Text::new("│"),
                box_x + box_width - 1,
                box_y + i,
                None,
                None,
            );
        }

        // Prompt
        let prompt = "Session name:";
        print_text_with_coordinates(Text::new(prompt), box_x + 2, box_y + 3, None, None);

        // Input
        let empty_string = String::new();
        let input = self.new_session_name.as_ref().unwrap_or(&empty_string);
        let input_display = format!("{}_", input);
        print_text_with_coordinates(
            Text::new(&input_display).color_range(2, 0..input_display.len()),
            box_x + 2,
            box_y + 4,
            None,
            None,
        );

        // Help text
        let help = "Enter: Create | Esc: Cancel";
        print_text_with_coordinates(
            Text::new(help).color_range(0, 0..help.len()),
            box_x + (box_width.saturating_sub(help.len())) / 2,
            box_y + box_height + 1,
            None,
            None,
        );
    }

    fn render_rename_session(&self, rows: usize, cols: usize) {
        if rows < 10 || cols < 50 {
            print_text(Text::new("Terminal too small"));
            return;
        }

        let box_width = 50.min(cols.saturating_sub(4));
        let box_height = 7;
        let box_x = (cols.saturating_sub(box_width)) / 2;
        let box_y = (rows.saturating_sub(box_height)) / 2;

        // Title
        let title = " Rename Session ";
        print_text_with_coordinates(
            Text::new(title).color_range(3, 0..title.len()),
            box_x + (box_width.saturating_sub(title.len())) / 2,
            box_y,
            None,
            None,
        );

        // Border
        let top_border = format!("┌{}┐", "─".repeat(box_width.saturating_sub(2)));
        let bottom_border = format!("└{}┘", "─".repeat(box_width.saturating_sub(2)));
        print_text_with_coordinates(Text::new(&top_border), box_x, box_y + 1, None, None);
        print_text_with_coordinates(Text::new(&bottom_border), box_x, box_y + box_height - 1, None, None);

        // Sides
        for i in 2..box_height - 1 {
            print_text_with_coordinates(Text::new("│"), box_x, box_y + i, None, None);
            print_text_with_coordinates(
                Text::new("│"),
                box_x + box_width - 1,
                box_y + i,
                None,
                None,
            );
        }

        // Prompt
        let prompt = "New name:";
        print_text_with_coordinates(Text::new(prompt), box_x + 2, box_y + 3, None, None);

        // Input
        let empty_string = String::new();
        let input = self.rename_input.as_ref().unwrap_or(&empty_string);
        let input_display = format!("{}_", input);
        print_text_with_coordinates(
            Text::new(&input_display).color_range(2, 0..input_display.len()),
            box_x + 2,
            box_y + 4,
            None,
            None,
        );

        // Help text
        let help = "Enter: Rename | Esc: Cancel";
        print_text_with_coordinates(
            Text::new(help).color_range(0, 0..help.len()),
            box_x + (box_width.saturating_sub(help.len())) / 2,
            box_y + box_height + 1,
            None,
            None,
        );
    }

    fn render_confirm_kill(&self, rows: usize, cols: usize) {
        if rows < 10 || cols < 50 {
            print_text(Text::new("Terminal too small"));
            return;
        }

        let session_name = self
            .sessions
            .get(self.selected_index)
            .map(|s| s.name.as_str())
            .unwrap_or("unknown");

        let box_width = 60.min(cols.saturating_sub(4));
        let box_height = 7;
        let box_x = (cols.saturating_sub(box_width)) / 2;
        let box_y = (rows.saturating_sub(box_height)) / 2;

        // Title
        let title = " Confirm Kill Session ";
        print_text_with_coordinates(
            Text::new(title).color_range(1, 0..title.len()),
            box_x + (box_width.saturating_sub(title.len())) / 2,
            box_y,
            None,
            None,
        );

        // Border
        let top_border = format!("┌{}┐", "─".repeat(box_width.saturating_sub(2)));
        let bottom_border = format!("└{}┘", "─".repeat(box_width.saturating_sub(2)));
        print_text_with_coordinates(Text::new(&top_border), box_x, box_y + 1, None, None);
        print_text_with_coordinates(Text::new(&bottom_border), box_x, box_y + box_height - 1, None, None);

        // Sides
        for i in 2..box_height - 1 {
            print_text_with_coordinates(Text::new("│"), box_x, box_y + i, None, None);
            print_text_with_coordinates(
                Text::new("│"),
                box_x + box_width - 1,
                box_y + i,
                None,
                None,
            );
        }

        // Message
        let msg = format!("Kill session '{}'?", session_name);
        print_text_with_coordinates(
            Text::new(&msg).color_range(1, 14..14 + session_name.len()),
            box_x + (box_width.saturating_sub(msg.len())) / 2,
            box_y + 3,
            None,
            None,
        );

        // Help text
        let help = "y: Yes | n: No | Esc: Cancel";
        print_text_with_coordinates(
            Text::new(help).color_range(0, 0..help.len()),
            box_x + (box_width.saturating_sub(help.len())) / 2,
            box_y + box_height + 1,
            None,
            None,
        );
    }

    fn render_help(&self, rows: usize, cols: usize) {
        if rows < 20 || cols < 60 {
            print_text(Text::new("Terminal too small"));
            return;
        }

        let title = "Bunshin - Help";
        print_text_with_coordinates(
            Text::new(title).color_range(3, 0..title.len()),
            (cols.saturating_sub(title.len())) / 2,
            1,
            None,
            None,
        );

        let separator = "─".repeat(cols.saturating_sub(4));
        print_text_with_coordinates(Text::new(&separator), 2, 2, None, None);

        let help_text = vec![
            "",
            "NAVIGATION",
            "  j, ↓         Move down",
            "  k, ↑         Move up",
            "  g, Home      Go to first session",
            "  G, End       Go to last session",
            "",
            "SESSION ACTIONS",
            "  Enter        Switch to selected session",
            "  c            Create new session",
            "  $            Rename current session",
            "  x            Kill selected session",
            "  d            Detach from session",
            "  (            Switch to previous session",
            "  )            Switch to next session",
            "",
            "CLAUDE CODE ORCHESTRATION",
            "  C            Launch Claude in new pane",
            "  A            Launch Claude in new tab",
            "  N            Create new session with Claude",
            "",
            "OTHER",
            "  ?            Toggle this help",
            "  q, Esc       Close manager",
            "",
        ];

        let start_y = 4;
        for (i, line) in help_text.iter().enumerate() {
            let y = start_y + i;
            if y >= rows - 2 {
                break;
            }
            let text = if line.starts_with("  ") {
                Text::new(line)
            } else if line.is_empty() {
                Text::new(line)
            } else {
                Text::new(line).color_range(2, 0..line.len())
            };
            print_text_with_coordinates(text, 4, y, None, None);
        }

        // Footer
        let footer = "Press ? or q to close help";
        print_text_with_coordinates(
            Text::new(footer).color_range(0, 0..footer.len()),
            (cols.saturating_sub(footer.len())) / 2,
            rows - 2,
            None,
            None,
        );
    }

    fn render_status_line(&self, rows: usize, cols: usize) {
        let status = format!("{} sessions", self.sessions.len());
        print_text_with_coordinates(
            Text::new(&status).color_range(0, 0..status.len()),
            (cols.saturating_sub(status.len())) / 2,
            rows - 2,
            None,
            None,
        );
    }

    fn render_error(&self, error: &str, rows: usize, cols: usize) {
        let error_text = format!("Error: {}", error);
        print_text_with_coordinates(
            Text::new(&error_text).color_range(1, 0..error_text.len()),
            (cols.saturating_sub(error_text.len().min(cols - 4))) / 2,
            rows - 3,
            None,
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session(name: &str, is_current: bool) -> SessionInfo {
        SessionInfo {
            name: name.to_string(),
            tabs: vec![],
            panes: PaneManifest {
                panes: std::collections::HashMap::new(),
            },
            connected_clients: 1,
            is_current_session: is_current,
            available_layouts: vec![],
            plugins: std::collections::BTreeMap::new(),
            web_clients_allowed: true,
            web_client_count: 0,
            tab_history: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn test_state_default() {
        let state = State::default();
        assert_eq!(state.sessions.len(), 0);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.mode, Mode::List);
        assert!(!state.show_help);
        assert!(state.new_session_name.is_none());
        assert!(state.rename_input.is_none());
        assert!(state.error_message.is_none());
    }

    #[test]
    fn test_navigation_down() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", true),
            create_test_session("session2", false),
            create_test_session("session3", false),
        ];

        assert_eq!(state.selected_index, 0);

        state.handle_list_key(KeyWithModifier::new(BareKey::Down));
        assert_eq!(state.selected_index, 1);

        state.handle_list_key(KeyWithModifier::new(BareKey::Down));
        assert_eq!(state.selected_index, 2);

        // Wrap around
        state.handle_list_key(KeyWithModifier::new(BareKey::Down));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_navigation_up() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", true),
            create_test_session("session2", false),
            create_test_session("session3", false),
        ];

        assert_eq!(state.selected_index, 0);

        // Wrap around backwards
        state.handle_list_key(KeyWithModifier::new(BareKey::Up));
        assert_eq!(state.selected_index, 2);

        state.handle_list_key(KeyWithModifier::new(BareKey::Up));
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_navigation_vim_keys() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", true),
            create_test_session("session2", false),
        ];

        // Test 'j' (down)
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('j')));
        assert_eq!(state.selected_index, 1);

        // Test 'k' (up)
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('k')));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_navigation_home_end() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", true),
            create_test_session("session2", false),
            create_test_session("session3", false),
            create_test_session("session4", false),
        ];

        state.selected_index = 2;

        // Test Home
        state.handle_list_key(KeyWithModifier::new(BareKey::Home));
        assert_eq!(state.selected_index, 0);

        // Test End
        state.handle_list_key(KeyWithModifier::new(BareKey::End));
        assert_eq!(state.selected_index, 3);

        // Test 'g' (home)
        state.selected_index = 2;
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('g')));
        assert_eq!(state.selected_index, 0);

        // Test 'G' (end)
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('G')));
        assert_eq!(state.selected_index, 3);
    }

    #[test]
    fn test_mode_transitions() {
        let mut state = State::default();
        state.sessions = vec![create_test_session("session1", true)];

        // Start in List mode
        assert_eq!(state.mode, Mode::List);

        // Switch to Create mode
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('c')));
        assert_eq!(state.mode, Mode::Create);
        assert!(state.new_session_name.is_some());

        // Cancel back to List mode
        state.handle_create_key(KeyWithModifier::new(BareKey::Esc));
        assert_eq!(state.mode, Mode::List);
        assert!(state.new_session_name.is_none());

        // Switch to Rename mode
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('$')));
        assert_eq!(state.mode, Mode::Rename);
        assert!(state.rename_input.is_some());

        // Cancel back to List mode
        state.handle_rename_key(KeyWithModifier::new(BareKey::Esc));
        assert_eq!(state.mode, Mode::List);
        assert!(state.rename_input.is_none());
    }

    #[test]
    fn test_help_toggle() {
        let mut state = State::default();
        assert!(!state.show_help);

        state.handle_list_key(KeyWithModifier::new(BareKey::Char('?')));
        assert!(state.show_help);

        state.handle_help_key(KeyWithModifier::new(BareKey::Char('?')));
        assert!(!state.show_help);
    }

    #[test]
    fn test_input_handling_create() {
        let mut state = State::default();
        state.mode = Mode::Create;
        state.new_session_name = Some(String::new());

        // Type some characters
        state.handle_create_key(KeyWithModifier::new(BareKey::Char('t')));
        state.handle_create_key(KeyWithModifier::new(BareKey::Char('e')));
        state.handle_create_key(KeyWithModifier::new(BareKey::Char('s')));
        state.handle_create_key(KeyWithModifier::new(BareKey::Char('t')));

        assert_eq!(state.new_session_name.as_ref().unwrap(), "test");

        // Test backspace
        state.handle_create_key(KeyWithModifier::new(BareKey::Backspace));
        assert_eq!(state.new_session_name.as_ref().unwrap(), "tes");
    }

    #[test]
    fn test_input_handling_rename() {
        let mut state = State::default();
        state.mode = Mode::Rename;
        state.rename_input = Some(String::new());

        // Type some characters
        state.handle_rename_key(KeyWithModifier::new(BareKey::Char('n')));
        state.handle_rename_key(KeyWithModifier::new(BareKey::Char('e')));
        state.handle_rename_key(KeyWithModifier::new(BareKey::Char('w')));

        assert_eq!(state.rename_input.as_ref().unwrap(), "new");

        // Test backspace
        state.handle_rename_key(KeyWithModifier::new(BareKey::Backspace));
        assert_eq!(state.rename_input.as_ref().unwrap(), "ne");
    }

    #[test]
    fn test_error_message_clearing() {
        let mut state = State::default();
        state.error_message = Some("Test error".to_string());

        assert!(state.error_message.is_some());

        // Any key should clear the error
        state.handle_key(KeyWithModifier::new(BareKey::Char('a')));

        assert!(state.error_message.is_none());
    }

    #[test]
    fn test_is_current_session_selected() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", false),
            create_test_session("session2", true),
            create_test_session("session3", false),
        ];

        state.selected_index = 0;
        assert!(!state.is_current_session_selected());

        state.selected_index = 1;
        assert!(state.is_current_session_selected());

        state.selected_index = 2;
        assert!(!state.is_current_session_selected());
    }

    #[test]
    fn test_rename_only_current_session() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", false),
            create_test_session("session2", true),
        ];

        // Try to rename non-current session
        state.selected_index = 0;
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('$')));

        // Should stay in List mode and show error
        assert_eq!(state.mode, Mode::List);
        assert!(state.error_message.is_some());
        assert!(state.rename_input.is_none());

        // Clear error
        state.error_message = None;

        // Select current session and try to rename
        state.selected_index = 1;
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('$')));

        // Should switch to Rename mode
        assert_eq!(state.mode, Mode::Rename);
        assert!(state.rename_input.is_some());
    }

    #[test]
    fn test_kill_not_current_session() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", false),
            create_test_session("session2", true),
        ];

        // Try to kill current session
        state.selected_index = 1;
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('x')));

        // Should stay in List mode and show error
        assert_eq!(state.mode, Mode::List);
        assert!(state.error_message.is_some());

        // Clear error
        state.error_message = None;

        // Try to kill non-current session
        state.selected_index = 0;
        state.handle_list_key(KeyWithModifier::new(BareKey::Char('x')));

        // Should switch to ConfirmKill mode
        assert_eq!(state.mode, Mode::ConfirmKill);
    }

    #[test]
    fn test_confirm_kill_dialog() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", false),
            create_test_session("session2", true),
        ];
        state.selected_index = 0;
        state.mode = Mode::ConfirmKill;

        // Test 'n' to cancel
        state.handle_confirm_kill_key(KeyWithModifier::new(BareKey::Char('n')));
        assert_eq!(state.mode, Mode::List);

        // Go back to confirm kill
        state.mode = Mode::ConfirmKill;

        // Test Esc to cancel
        state.handle_confirm_kill_key(KeyWithModifier::new(BareKey::Esc));
        assert_eq!(state.mode, Mode::List);
    }

    #[test]
    fn test_session_update_clamps_index() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", false),
            create_test_session("session2", false),
            create_test_session("session3", false),
        ];
        state.selected_index = 2;

        // Simulate session list shrinking
        let event = Event::SessionUpdate(
            vec![
                create_test_session("session1", false),
            ],
            vec![],
        );

        state.update(event);

        // Index should be clamped to valid range
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_empty_session_list() {
        let mut state = State::default();
        state.sessions = vec![];

        // Navigation should not panic with empty list
        state.handle_list_key(KeyWithModifier::new(BareKey::Down));
        assert_eq!(state.selected_index, 0);

        state.handle_list_key(KeyWithModifier::new(BareKey::Up));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_session_name_validation_create() {
        let mut state = State::default();
        state.mode = Mode::Create;

        // Empty name
        state.new_session_name = Some(String::new());
        state.create_session();
        assert!(state.error_message.is_some());
        assert!(state.error_message.as_ref().unwrap().contains("empty"));

        state.error_message = None;

        // Name with slash
        state.new_session_name = Some("session/name".to_string());
        state.mode = Mode::Create;
        state.create_session();
        assert!(state.error_message.is_some());
        assert!(state.error_message.as_ref().unwrap().contains("'/'"));

        state.error_message = None;

        // Name too long
        state.new_session_name = Some("a".repeat(110));
        state.mode = Mode::Create;
        state.create_session();
        assert!(state.error_message.is_some());
        assert!(state.error_message.as_ref().unwrap().contains("too long"));
    }

    #[test]
    fn test_session_name_validation_rename() {
        let mut state = State::default();
        state.mode = Mode::Rename;

        // Empty name
        state.rename_input = Some(String::new());
        state.rename_session();
        assert!(state.error_message.is_some());
        assert!(state.error_message.as_ref().unwrap().contains("empty"));

        state.error_message = None;

        // Name with slash
        state.rename_input = Some("session/name".to_string());
        state.mode = Mode::Rename;
        state.rename_session();
        assert!(state.error_message.is_some());
        assert!(state.error_message.as_ref().unwrap().contains("'/'"));

        state.error_message = None;

        // Name too long
        state.rename_input = Some("a".repeat(110));
        state.mode = Mode::Rename;
        state.rename_session();
        assert!(state.error_message.is_some());
        assert!(state.error_message.as_ref().unwrap().contains("too long"));
    }

    #[test]
    fn test_extract_repo_name() {
        let state = State::default();

        assert_eq!(state.extract_repo_name("/home/user/projects/bunshin"), "bunshin");
        assert_eq!(state.extract_repo_name("/home/user/my-app"), "my-app");
        assert_eq!(state.extract_repo_name("/"), "unknown");
        assert_eq!(state.extract_repo_name(""), "unknown");
    }

    #[test]
    fn test_parse_session_repos_json() {
        let mut state = State::default();

        // Empty JSON
        state.parse_session_repos_json("{}");
        assert!(state.session_repos.is_empty());

        // Simple JSON
        state.parse_session_repos_json(r#"{"session1":"repo1","session2":"repo2"}"#);
        assert_eq!(state.session_repos.get("session1"), Some(&"repo1".to_string()));
        assert_eq!(state.session_repos.get("session2"), Some(&"repo2".to_string()));

        // JSON with escaped quotes
        state.session_repos.clear();
        state.parse_session_repos_json(r#"{"my\"session":"my\"repo"}"#);
        assert_eq!(state.session_repos.get("my\"session"), Some(&"my\"repo".to_string()));
    }

    #[test]
    fn test_get_grouped_sessions() {
        let mut state = State::default();
        state.sessions = vec![
            create_test_session("session1", true),
            create_test_session("session2", false),
            create_test_session("session3", false),
        ];
        state.session_repos.insert("session1".to_string(), "repo-a".to_string());
        state.session_repos.insert("session2".to_string(), "repo-b".to_string());
        // session3 has no repo, should go to "Other"

        let grouped = state.get_grouped_sessions();

        // Should have 3 groups: repo-a, repo-b, Other
        assert_eq!(grouped.len(), 3);

        // Check order (alphabetical, Other last)
        assert_eq!(grouped[0].0, "repo-a");
        assert_eq!(grouped[1].0, "repo-b");
        assert_eq!(grouped[2].0, "Other");

        // Check sessions in each group
        assert_eq!(grouped[0].1.len(), 1);
        assert_eq!(grouped[0].1[0].name, "session1");

        assert_eq!(grouped[1].1.len(), 1);
        assert_eq!(grouped[1].1[0].name, "session2");

        assert_eq!(grouped[2].1.len(), 1);
        assert_eq!(grouped[2].1[0].name, "session3");
    }

    #[test]
    fn test_state_default_with_repos() {
        let state = State::default();
        assert!(state.session_repos.is_empty());
        assert!(state.pending_repo_query.is_none());
        assert!(state.current_repo.is_none());
    }
}
