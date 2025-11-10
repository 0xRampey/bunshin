use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use zellij_tile::prelude::*;

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
    session_dirs: HashMap<String, String>, // session_name -> working_directory
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
        ]);
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::OpenTerminalsOrPlugins,
            PermissionType::RunCommands,
            PermissionType::FullHdAccess,
        ]);
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
                // Reload session directories
                self.load_session_dirs();
                should_render = true;
            }
            Event::ModeUpdate(mode_info) => {
                self.colors = mode_info.style.colors;
                should_render = true;
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
        if let Some(name) = &self.new_session_name {
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

    fn load_session_dirs(&mut self) {
        // Read session directories from ~/.bunshin/session-dirs.json
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(&home);
            let mut path = home_path.clone();
            path.push(".bunshin");
            path.push("session-dirs.json");

            let debug_path = home_path.join(".bunshin").join("plugin-debug.log");

            match fs::read_to_string(&path) {
                Ok(contents) => {
                    match serde_json::from_str::<HashMap<String, String>>(&contents) {
                        Ok(dirs) => {
                            // Write debug info to a file we can check
                            let debug_msg = format!("Loaded {} entries: {:?}\n", dirs.len(), dirs);
                            let _ = fs::write(&debug_path, debug_msg);

                            self.session_dirs = dirs;
                        }
                        Err(e) => {
                            // JSON parse error
                            let _ = fs::write(&debug_path, format!("JSON parse error: {:?}\n", e));
                            self.session_dirs.clear();
                        }
                    }
                }
                Err(e) => {
                    // File doesn't exist or can't be read
                    let _ = fs::write(&debug_path, format!("File read error: {:?}\n", e));
                    self.session_dirs.clear();
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

    fn create_claude_session(&self) {
        // Create a new session with Claude Code auto-started
        let session_name = format!("claude-{}", chrono::Utc::now().timestamp());

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

        // Group sessions by CWD
        let mut cwd_groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (idx, session) in self.sessions.iter().enumerate() {
            let cwd = self.session_dirs.get(&session.name)
                .cloned()
                .unwrap_or_else(|| format!("N/A (looking for: '{}')", session.name));
            cwd_groups.entry(cwd).or_insert_with(Vec::new).push(idx);
        }

        // Render grouped sessions
        let list_start_y = 3;
        let mut current_y = list_start_y;
        let max_y = rows.saturating_sub(3);

        for (cwd, session_indices) in cwd_groups.iter() {
            if current_y >= max_y {
                break;
            }

            // Render CWD header
            let cwd_display = if cwd.len() > cols.saturating_sub(4) {
                format!("...{}", &cwd[cwd.len().saturating_sub(cols - 7)..])
            } else {
                cwd.clone()
            };
            let cwd_text = Text::new(&cwd_display).color_range(3, 0..cwd_display.len());
            print_text_with_coordinates(cwd_text, 2, current_y, None, None);
            current_y += 1;

            // Render sessions in this group (indented)
            for &session_idx in session_indices {
                if current_y >= max_y {
                    break;
                }

                let session = &self.sessions[session_idx];
                let is_selected = session_idx == self.selected_index;
                let is_current = session.is_current_session;

                let session_indicator = if is_current { "*" } else { " " };
                let windows_count = session.tabs.len();
                let windows_text = if windows_count == 1 {
                    "window".to_string()
                } else {
                    format!("{} windows", windows_count)
                };

                let line = format!(
                    "  {}{:<width$}  {}",
                    session_indicator,
                    session.name,
                    windows_text,
                    width = cols.saturating_sub(20).max(10),
                );

                let mut text = Text::new(&line);
                if is_selected {
                    text = text.selected();
                }
                if is_current {
                    text = text.color_range(2, 2..2 + session.name.len() + 1);
                }

                print_text_with_coordinates(text, 2, current_y, None, None);
                current_y += 1;
            }

            // Add spacing between groups
            current_y += 1;
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
        let status = format!(
            "{} sessions | ?: Help | q: Quit",
            self.sessions.len()
        );
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
}
