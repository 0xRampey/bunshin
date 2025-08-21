pub mod session;
pub mod git;
pub mod ui;
pub mod claude;
pub mod shell;
pub mod session_shell;
pub mod core;
pub mod cli;
pub mod manager;
pub mod process;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use clap::Parser;

use crate::claude::ClaudeCodeManager;
use crate::git::GitWorktree;
use crate::session::Session;
use crate::shell::ShellManager;
use crate::session_shell::SessionShell;
use crate::ui::{draw_create_session, draw_sessions_list, App, AppState};
use crate::cli::{Cli, Commands};
use crate::core::{BunshinSession, Window, Agent, AgentModel, Project};
use crate::manager::BunshinManager;
use crate::process::{ProcessManager, ProcessConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check if we're being called from within a session to return to manager
    if SessionShell::in_session() {
        if let Some((branch, path)) = SessionShell::current_session_info() {
            println!("Leaving session '{}' at {}", branch, path.display());
        }
    }
    
    // Parse CLI arguments
    let cli = Cli::parse();
    
    match cli.command {
        None => {
            // No command provided - run TUI session manager
            run_session_manager().await
        }
        Some(Commands::Manager) => {
            // Force TUI session manager
            run_session_manager().await
        }
        Some(Commands::Attach { target }) => {
            // Legacy attach command - try to attach to session by name
            attach_to_session(&target).await
        }
        Some(Commands::Shell { agent_id }) => {
            handle_agent_shell(agent_id).await
        }
        Some(Commands::Worktree { agent_id }) => {
            handle_agent_worktree(agent_id).await
        }
        Some(Commands::Ls { all, project, format }) => {
            handle_list_sessions(all, project, format).await
        }
        Some(Commands::Ps { session, all, format }) => {
            handle_list_agents(session, all, format).await
        }
        Some(Commands::Spawn { model, count, project, labels, window, task, tools }) => {
            handle_spawn_agents(model, count, project, labels, window, task, tools).await
        }
        Some(Commands::Clone { agent_id, count, project }) => {
            handle_clone_agent(agent_id, count, project).await
        }
        Some(Commands::Kill { targets, all, force }) => {
            handle_kill_entities(targets, all, force).await
        }
        Some(Commands::Broadcast { scope, project, window, labels, message }) => {
            handle_broadcast(scope, project, window, labels, message).await
        }
        Some(Commands::New { entity }) => {
            handle_new_entity(entity).await
        }
        Some(Commands::Info { target }) => {
            handle_show_info(target).await
        }
        Some(Commands::Project { action }) => {
            handle_project_action(action).await
        }
        Some(Commands::Logs { agent_id, lines, follow }) => {
            handle_tail_logs(agent_id, lines, follow).await
        }
        Some(Commands::Export { session_id, output, format }) => {
            handle_export(session_id, output, format).await
        }
        Some(Commands::Import { input, merge }) => {
            handle_import(input, merge).await
        }
    }
}

async fn run_session_manager() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;
    app.session_list_state.select(Some(0));

    let res = run_app(&mut terminal, &mut app).await;

    // Clean up terminal first
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    match res {
        Ok(Some(session)) => {
            // Launch session shell (tmux-like behavior)
            println!("Attaching to session '{}'...", session.name);
            SessionShell::launch_session_shell(&session.worktree_path, &session.branch)?;
        }
        Ok(None) => {
            // User quit normally
        }
        Err(err) => {
            println!("Error: {:?}", err);
        }
    }

    Ok(())
}

async fn attach_to_session(session_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new()?;
    
    // Find the session
    if let Some(session) = app.session_manager.sessions.iter().find(|s| s.name == session_name) {
        println!("Attaching to session '{}'...", session_name);
        SessionShell::launch_session_shell(&session.worktree_path, &session.branch)?;
    } else {
        println!("Session '{}' not found.", session_name);
        println!("Available sessions:");
        for session in &app.session_manager.sessions {
            println!("  - {}", session.name);
        }
    }
    
    Ok(())
}


async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<Option<Session>, Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| {
            match app.state {
                AppState::SessionList | AppState::SessionDetails => {
                    draw_sessions_list(f, app, f.area());
                }
                AppState::CreateSession => {
                    draw_sessions_list(f, app, f.area());
                    draw_create_session(f, app, f.area());
                }
            }
        })?;

        if let Event::Key(key) = event::read()? {
            match app.state {
                AppState::SessionList => {
                    match key.code {
                        KeyCode::Char('q') => return Ok(None),
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.next_session();
                            app.status_message = None;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.previous_session();
                            app.status_message = None;
                            // Cleanup dead shells periodically
                            app.shell_manager.cleanup_dead_shells();
                        }
                        KeyCode::Enter => {
                            if app.session_manager.sessions.is_empty() {
                                app.status_message = Some("No sessions available. Press 'n' to create a new session.".to_string());
                            } else if let Some(session) = app.get_selected_session() {
                                // Return the selected session to launch it (tmux-like behavior)
                                return Ok(Some(session.clone()));
                            }
                        }
                        KeyCode::Char('c') => {
                            if app.session_manager.sessions.is_empty() {
                                app.status_message = Some("No sessions available. Press 'n' to create a new session.".to_string());
                            } else if let Some(session) = app.get_selected_session().cloned() {
                                let mut session = session;
                                match ClaudeCodeManager::launch_claude_code(&mut session) {
                                    Ok(()) => {
                                        if let Some(s) = app.session_manager.get_session_mut(&session.name) {
                                            s.claude_pid = session.claude_pid;
                                        }
                                        app.save_sessions()?;
                                        app.status_message = Some(format!("Launched Claude Code for session '{}'", session.name));
                                    }
                                    Err(e) => {
                                        app.status_message = Some(format!("Failed to launch Claude Code: {}", e));
                                    }
                                }
                            }
                        }
                        KeyCode::Char('n') => {
                            app.state = AppState::CreateSession;
                            app.create_session_form = Default::default();
                        }
                        KeyCode::Char('d') => {
                            if let Some(session) = app.get_selected_session() {
                                let session_name = session.name.clone();
                                let branch_name = session.branch.clone();
                                if let Some(mut session) = app.session_manager.get_session(&session_name).cloned() {
                                    ClaudeCodeManager::kill_claude_code(&mut session).ok();
                                    GitWorktree::remove_worktree(&session.repo_path, &session.worktree_path).ok();
                                    // Close the shell for this branch
                                    app.shell_manager.close_shell(&branch_name).ok();
                                }
                                app.session_manager.remove_session(&session_name);
                                app.save_sessions()?;
                                if app.selected_session >= app.session_manager.sessions.len() && !app.session_manager.sessions.is_empty() {
                                    app.selected_session = app.session_manager.sessions.len() - 1;
                                }
                                app.session_list_state.select(if app.session_manager.sessions.is_empty() { None } else { Some(app.selected_session) });
                                app.status_message = Some(format!("Deleted session and closed shell for branch '{}'", branch_name));
                            }
                        }
                        _ => {}
                    }
                }
                AppState::CreateSession => {
                    match key.code {
                        KeyCode::Esc => app.state = AppState::SessionList,
                        KeyCode::Tab => {
                            app.create_session_form.current_field = 
                                (app.create_session_form.current_field + 1) % 3;
                        }
                        KeyCode::Enter => {
                            if app.create_session_form.name.is_empty() ||
                               app.create_session_form.repo_path.is_empty() ||
                               app.create_session_form.branch.is_empty() {
                                app.status_message = Some("Please fill in all fields".to_string());
                            } else {
                                let repo_path = PathBuf::from(&app.create_session_form.repo_path);
                                if !GitWorktree::is_git_repo(&repo_path) {
                                    app.status_message = Some("Invalid Git repository path".to_string());
                                } else {
                                    // Create worktree in a safer location - use temp directory structure
                                    let worktree_base = dirs::home_dir()
                                        .unwrap_or_else(|| PathBuf::from("/tmp"))
                                        .join(".bunshin")
                                        .join("worktrees");
                                    
                                    // Ensure worktree base directory exists
                                    std::fs::create_dir_all(&worktree_base).ok();
                                    
                                    let worktree_path = worktree_base.join(format!("{}-{}", 
                                        app.create_session_form.name,
                                        app.create_session_form.branch
                                    ));
                                    
                                    app.status_message = Some(format!("Creating worktree for branch '{}'...", app.create_session_form.branch));
                                    
                                    // Debug info
                                    eprintln!("Debug - Repo path: {:?}", repo_path);
                                    eprintln!("Debug - Worktree path: {:?}", worktree_path);
                                    eprintln!("Debug - Branch: {}", app.create_session_form.branch);
                                    
                                    match GitWorktree::create_worktree(
                                        &repo_path,
                                        &worktree_path,
                                        &app.create_session_form.branch,
                                    ) {
                                        Ok(()) => {
                                            let session = Session::new(
                                                app.create_session_form.name.clone(),
                                                worktree_path.clone(),
                                                app.create_session_form.branch.clone(),
                                                repo_path,
                                            );
                                            app.session_manager.add_session(session);
                                            app.save_sessions()?;
                                            
                                            // Automatically open a shell in the new worktree
                                            match app.shell_manager.open_shell(&app.create_session_form.branch, &worktree_path) {
                                                Ok(()) => {
                                                    app.status_message = Some(format!(
                                                        "Created session '{}' with branch '{}' and opened shell", 
                                                        app.create_session_form.name,
                                                        app.create_session_form.branch
                                                    ));
                                                }
                                                Err(e) => {
                                                    app.status_message = Some(format!(
                                                        "Created session '{}' with branch '{}', but failed to open shell: {}", 
                                                        app.create_session_form.name,
                                                        app.create_session_form.branch,
                                                        e
                                                    ));
                                                }
                                            }
                                            
                                            app.state = AppState::SessionList;
                                            app.session_list_state.select(Some(app.session_manager.sessions.len() - 1));
                                            app.selected_session = app.session_manager.sessions.len() - 1;
                                        }
                                        Err(e) => {
                                            app.status_message = Some(format!("Failed to create worktree/branch: {}", e));
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            match app.create_session_form.current_field {
                                0 => app.create_session_form.name.push(c),
                                1 => app.create_session_form.repo_path.push(c),
                                2 => app.create_session_form.branch.push(c),
                                _ => {}
                            }
                        }
                        KeyCode::Backspace => {
                            match app.create_session_form.current_field {
                                0 => { app.create_session_form.name.pop(); }
                                1 => { app.create_session_form.repo_path.pop(); }
                                2 => { app.create_session_form.branch.pop(); }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                AppState::SessionDetails => {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => app.state = AppState::SessionList,
                        _ => {}
                    }
                }
            }
        }
    }
}

// CLI Command Handlers

async fn handle_list_sessions(_all: bool, _project: Option<String>, format: String) -> Result<(), Box<dyn std::error::Error>> {
    let manager = BunshinManager::new()?;
    let sessions = manager.list_sessions();
    
    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }
    
    match format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&sessions)?);
        }
        "compact" => {
            for session in &sessions {
                let agent_count = session.total_agents();
                println!("{}: {} agents, ${:.4} total cost", 
                    session.name, agent_count, session.total_cost());
            }
        }
        _ => {
            // Table format (default)
            use tabled::{Table, Tabled};
            
            #[derive(Tabled)]
            struct SessionRow {
                name: String,
                agents: String,
                windows: String,
                cost: String,
                tokens: String,
                created: String,
            }
            
            let rows: Vec<SessionRow> = sessions.iter().map(|s| SessionRow {
                name: s.name.clone(),
                agents: s.total_agents().to_string(),
                windows: s.windows.len().to_string(),
                cost: format!("${:.4}", s.total_cost()),
                tokens: s.total_tokens().to_string(),
                created: s.created_at.format("%Y-%m-%d %H:%M").to_string(),
            }).collect();
            
            let table = Table::new(rows);
            println!("{}", table);
        }
    }
    
    Ok(())
}

async fn handle_list_agents(session: Option<String>, all: bool, format: String) -> Result<(), Box<dyn std::error::Error>> {
    let manager = BunshinManager::new()?;
    
    let agents = if all {
        manager.list_all_agents()
    } else if let Some(session_id) = session {
        manager.list_agents_in_session(&session_id)
            .into_iter()
            .map(|agent| ("", "", agent))
            .collect()
    } else if let (Some(session_id), _) = manager.get_current_context() {
        manager.list_agents_in_session(session_id)
            .into_iter()
            .map(|agent| ("", "", agent))
            .collect()
    } else {
        println!("No current session. Use --session <id> or --all flag.");
        return Ok(());
    };
    
    if agents.is_empty() {
        println!("No agents found.");
        return Ok(());
    }
    
    match format.as_str() {
        "json" => {
            let agent_data: Vec<&Agent> = agents.iter().map(|(_, _, agent)| *agent).collect();
            println!("{}", serde_json::to_string_pretty(&agent_data)?);
        }
        "compact" => {
            for (_, _, agent) in &agents {
                println!("{}: {} ({}) - {}", 
                    agent.id, agent.name, agent.model, agent.state);
            }
        }
        _ => {
            // Table format (default)
            use tabled::{Table, Tabled};
            
            #[derive(Tabled)]
            struct AgentRow {
                id: String,
                name: String,
                model: String,
                state: String,
                uptime: String,
                tokens: String,
                cost: String,
            }
            
            let rows: Vec<AgentRow> = agents.iter().map(|(_, _, agent)| AgentRow {
                id: agent.id.clone(),
                name: agent.name.clone(),
                model: agent.model.to_string(),
                state: agent.state.to_string(),
                uptime: agent.uptime_string(),
                tokens: agent.tokens_used.to_string(),
                cost: format!("${:.4}", agent.estimated_cost),
            }).collect();
            
            let table = Table::new(rows);
            println!("{}", table);
        }
    }
    
    Ok(())
}

async fn handle_spawn_agents(model: String, count: u32, project: Option<String>, labels: Vec<String>, 
                           _window: Option<String>, task: Option<String>, tools: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = BunshinManager::new()?;
    let mut process_manager = ProcessManager::new()?;
    
    let agent_model: AgentModel = model.parse()?;
    
    match manager.spawn_agents_in_current_window(count, agent_model, project.clone(), labels, task.clone(), tools) {
        Ok(agent_data) => {
            println!("Successfully created {} agents:", count);
            
            // Now spawn actual processes for each agent
            let mut spawned_count = 0;
            let total_agents = agent_data.len();
            for (agent_id, worktree_path) in &agent_data {
                // Create process config
                let mut config = ProcessConfig::default();
                
                // Use worktree path if available, otherwise current directory
                config.working_directory = worktree_path.clone()
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
                
                // Add project-specific environment variables
                if let Some(ref project_name) = project {
                    config.environment_vars.insert("BUNSHIN_PROJECT".to_string(), project_name.clone());
                }
                
                if let Some(ref task_desc) = task {
                    config.environment_vars.insert("BUNSHIN_TASK".to_string(), task_desc.clone());
                }
                
                // Add worktree information
                if let Some(path) = &worktree_path {
                    config.environment_vars.insert("BUNSHIN_WORKTREE".to_string(), path.display().to_string());
                    println!("  📁 Agent {} worktree: {}", agent_id, path.display());
                }
                
                // Get mutable reference to agent and spawn process
                if let Some((_, _, agent)) = manager.find_agent_mut(&agent_id) {
                    let agent_name = agent.name.clone();
                    match process_manager.spawn_agent_process(agent, config) {
                        Ok(()) => {
                            spawned_count += 1;
                            println!("  ✅ {} ({})", agent_id, agent_name);
                        }
                        Err(e) => {
                            println!("  ❌ {} ({}): {}", agent_id, agent_name, e);
                            // Mark agent as error state
                            agent.set_error(format!("Failed to spawn process: {}", e));
                        }
                    }
                } else {
                    println!("  ❌ {} (agent not found)", agent_id);
                }
            }
            
            manager.save_to_disk()?;
            println!("\nSpawned {}/{} agent processes successfully", spawned_count, total_agents);
            
            if spawned_count < total_agents {
                println!("Some agents failed to start. Check logs or try different models.");
                println!("Available models: claude-code, claude-3.5-sonnet, gpt-4o, gpt-4o-mini");
                println!("Or use custom commands like: cmd:python3 my_script.py");
            }
        }
        Err(e) => {
            println!("Failed to create agents: {}", e);
            return Ok(());
        }
    }
    
    Ok(())
}

async fn handle_clone_agent(_agent_id: String, _count: u32, _project: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Agent cloning not yet implemented");
    Ok(())
}

async fn handle_kill_entities(targets: Vec<String>, all: bool, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = BunshinManager::new()?;
    let mut process_manager = ProcessManager::new()?;
    
    let agent_ids_to_kill = if all {
        // Kill all agents
        if !force {
            println!("This will kill ALL running agents. Use --force to confirm.");
            return Ok(());
        }
        manager.list_all_agents().into_iter().map(|(_, _, agent)| agent.id.clone()).collect()
    } else {
        targets
    };
    
    if agent_ids_to_kill.is_empty() {
        println!("No agents specified to kill.");
        return Ok(());
    }
    
    let mut killed_count = 0;
    for agent_id in agent_ids_to_kill {
        if let Some((_, _, agent)) = manager.find_agent_mut(&agent_id) {
            match process_manager.kill_agent(&agent_id, agent) {
                Ok(()) => {
                    killed_count += 1;
                    println!("✅ Killed agent {}", agent_id);
                }
                Err(e) => {
                    println!("❌ Failed to kill agent {}: {}", agent_id, e);
                }
            }
        } else {
            println!("❌ Agent {} not found", agent_id);
        }
    }
    
    manager.save_to_disk()?;
    println!("Killed {} agents", killed_count);
    
    Ok(())
}

async fn handle_broadcast(scope: Option<String>, project: Option<String>, window: Option<String>, 
                         labels: Vec<String>, message: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = BunshinManager::new()?;
    let mut process_manager = ProcessManager::new()?;
    
    // Determine target agents
    let target_agent_ids = if let Some(project_name) = project {
        manager.broadcast_to_project(&project_name, &message)?
    } else if !labels.is_empty() {
        manager.broadcast_to_labels(&labels, &message)?
    } else if let Some(window_id) = window {
        // Find all agents in the specified window
        let mut agent_ids = Vec::new();
        for (session_id, session) in &manager.sessions {
            if let Some(win) = session.windows.get(&window_id) {
                agent_ids.extend(win.agents.keys().cloned());
            }
        }
        agent_ids
    } else {
        // Broadcast to all agents if no specific scope
        manager.list_all_agents()
            .into_iter()
            .map(|(_, _, agent)| agent.id.clone())
            .collect()
    };
    
    if target_agent_ids.is_empty() {
        println!("No agents found matching the specified criteria.");
        return Ok(());
    }
    
    println!("Broadcasting message to {} agents...", target_agent_ids.len());
    println!("Message: {}", message);
    println!();
    
    let successful = process_manager.broadcast_message(&target_agent_ids, &message)?;
    
    println!("Successfully sent message to {}/{} agents:", successful.len(), target_agent_ids.len());
    for agent_id in &successful {
        println!("  ✅ {}", agent_id);
    }
    
    if successful.len() < target_agent_ids.len() {
        println!("\nFailed to reach {} agents (not running or process error)", 
                target_agent_ids.len() - successful.len());
    }
    
    Ok(())
}

async fn handle_new_entity(entity: crate::cli::NewEntity) -> Result<(), Box<dyn std::error::Error>> {
    use crate::cli::NewEntity;
    let mut manager = BunshinManager::new()?;
    
    match entity {
        NewEntity::Session { name, repo, branch, project: _ } => {
            // Create session using git worktree - integrate with existing logic
            use crate::git::GitWorktree;
            
            if !GitWorktree::is_git_repo(&repo) {
                println!("Error: Invalid Git repository path: {:?}", repo);
                return Ok(());
            }
            
            let worktree_base = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".bunshin")
                .join("worktrees");
            
            std::fs::create_dir_all(&worktree_base).ok();
            let worktree_path = worktree_base.join(format!("{}-{}", name, branch));
            
            match GitWorktree::create_worktree(&repo, &worktree_path, &branch) {
                Ok(()) => {
                    let session_id = manager.create_session(name.clone(), worktree_path.clone());
                    manager.save_to_disk()?;
                    println!("Created session '{}' with ID: {}", name, session_id);
                    println!("  Worktree: {}", worktree_path.display());
                    println!("  Branch: {}", branch);
                }
                Err(e) => {
                    println!("Failed to create session: {}", e);
                }
            }
        }
        NewEntity::Window { name, session, project: _, dir: _ } => {
            let session_id = if let Some(sid) = session {
                sid
            } else if let (Some(current_session), _) = manager.get_current_context() {
                current_session.to_string()
            } else {
                println!("No current session. Use --session <id> to specify target session.");
                return Ok(());
            };
            
            match manager.create_window(&session_id, name.clone()) {
                Ok(window_id) => {
                    manager.save_to_disk()?;
                    println!("Created window '{}' with ID: {}", name, window_id);
                }
                Err(e) => {
                    println!("Failed to create window: {}", e);
                }
            }
        }
        NewEntity::Project { name, description, repo, labels } => {
            let project = Project {
                name: name.clone(),
                description,
                repository: repo,
                labels,
                created_at: chrono::Utc::now(),
            };
            
            match manager.create_project(project) {
                Ok(()) => {
                    manager.save_to_disk()?;
                    println!("Created project '{}'", name);
                }
                Err(e) => {
                    println!("Failed to create project: {}", e);
                }
            }
        }
    }
    Ok(())
}

async fn handle_show_info(_target: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("Info display not yet implemented for target: {}", _target);
    Ok(())
}

async fn handle_project_action(action: crate::cli::ProjectAction) -> Result<(), Box<dyn std::error::Error>> {
    use crate::cli::ProjectAction;
    let mut manager = BunshinManager::new()?;
    
    match action {
        ProjectAction::List => {
            let projects = manager.list_projects();
            if projects.is_empty() {
                println!("No projects found.");
            } else {
                use tabled::{Table, Tabled};
                
                #[derive(Tabled)]
                struct ProjectRow {
                    name: String,
                    description: String,
                    repository: String,
                    labels: String,
                    created: String,
                }
                
                let rows: Vec<ProjectRow> = projects.iter().map(|p| ProjectRow {
                    name: p.name.clone(),
                    description: p.description.clone().unwrap_or_else(|| "".to_string()),
                    repository: p.repository.clone().unwrap_or_else(|| "".to_string()),
                    labels: p.labels.join(", "),
                    created: p.created_at.format("%Y-%m-%d %H:%M").to_string(),
                }).collect();
                
                let table = Table::new(rows);
                println!("{}", table);
            }
        }
        ProjectAction::Show { name } => {
            if let Some(project) = manager.get_project(&name) {
                println!("Project: {}", project.name);
                if let Some(desc) = &project.description {
                    println!("Description: {}", desc);
                }
                if let Some(repo) = &project.repository {
                    println!("Repository: {}", repo);
                }
                if !project.labels.is_empty() {
                    println!("Labels: {}", project.labels.join(", "));
                }
                println!("Created: {}", project.created_at.format("%Y-%m-%d %H:%M"));
            } else {
                println!("Project '{}' not found.", name);
            }
        }
        ProjectAction::Update { name, description, repo, add_labels, remove_labels } => {
            match manager.update_project(&name, description, repo, add_labels, remove_labels) {
                Ok(()) => {
                    manager.save_to_disk()?;
                    println!("Updated project '{}'", name);
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        ProjectAction::Delete { name, force } => {
            if !force {
                println!("Are you sure you want to delete project '{}'? Use --force to confirm.", name);
                return Ok(());
            }
            match manager.delete_project(&name) {
                Ok(()) => {
                    manager.save_to_disk()?;
                    println!("Deleted project '{}'", name);
                }
                Err(e) => println!("Error: {}", e),
            }
        }
    }
    Ok(())
}

async fn handle_tail_logs(agent_id: String, lines: u32, follow: bool) -> Result<(), Box<dyn std::error::Error>> {
    let process_manager = ProcessManager::new()?;
    
    match process_manager.tail_logs(&agent_id, lines, follow) {
        Ok(()) => {
            if !follow {
                println!("--- End of log for agent {} ---", agent_id);
            }
        }
        Err(e) => {
            println!("Failed to tail logs for agent {}: {}", agent_id, e);
            
            // Try to show log file location
            if let Some(stats) = process_manager.get_process_stats(&agent_id) {
                if let Some(log_file) = stats.log_file {
                    println!("Log file: {}", log_file.display());
                } else {
                    let logs_dir = dirs::home_dir()
                        .map(|h| h.join(".bunshin").join("logs"))
                        .unwrap_or_else(|| PathBuf::from("/tmp/bunshin-logs"));
                    let log_file = logs_dir.join(format!("{}.log", agent_id));
                    println!("Expected log file: {}", log_file.display());
                }
            }
        }
    }
    
    Ok(())
}

async fn handle_export(_session_id: String, _output: Option<PathBuf>, _format: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("Export not yet implemented for session: {}", _session_id);
    Ok(())
}

async fn handle_import(_input: PathBuf, _merge: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Import not yet implemented for file: {:?}", _input);
    Ok(())
}

async fn handle_agent_shell(agent_id: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔌 Connecting to agent {} interactive shell...", agent_id);
    println!("📋 Available commands:");
    println!("  help     - Show available commands");  
    println!("  status   - Show agent status");
    println!("  ping     - Test agent responsiveness");
    println!("  echo <text> - Echo back text");
    println!("  simulate <task> - Simulate working on a task");
    println!("  quit     - Exit interactive session");
    println!();
    
    // Check if agent exists and get process info
    let manager = BunshinManager::new()?;
    if let Some((session_id, window_id, agent)) = manager.find_agent(&agent_id) {
        println!("📊 Agent Info:");
        println!("  ID: {}", agent.id);
        println!("  Name: {}", agent.name);
        println!("  Model: {}", agent.model);
        println!("  State: {}", agent.state);
        println!("  Session: {} / Window: {}", session_id, window_id);
        if let Some(project) = &agent.project {
            println!("  Project: {}", project);
        }
        if let Some(task) = &agent.task_description {
            println!("  Task: {}", task);
        }
        println!("  Uptime: {}", agent.uptime_string());
        println!();
        
        // Create a simple interactive loop
        println!("🎯 Interactive shell connected. Type 'help' for commands, 'quit' to exit.");
        
        use std::io::{self, Write};
        
        let mut process_manager = ProcessManager::new()?;
        
        loop {
            print!("bunshin:{} > ", agent_id);
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input.is_empty() {
                continue;
            }
            
            if input == "quit" || input == "exit" {
                println!("👋 Disconnecting from agent shell...");
                break;
            }
            
            // Try to send command to the agent process
            match process_manager.send_input(&agent_id, input) {
                Ok(()) => {
                    println!("✅ Command sent to agent");
                    
                    // Try to read any output
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    match process_manager.read_output(&agent_id, Some(10)) {
                        Ok(lines) => {
                            for line in lines {
                                println!("📤 {}", line);
                            }
                        }
                        Err(_) => {
                            // No output yet or agent not responding
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to send command: {}", e);
                    println!("💡 Note: Agent process may have terminated. Try respawning it.");
                    break;
                }
            }
        }
    } else {
        println!("❌ Agent {} not found", agent_id);
        println!("📋 Available agents:");
        
        let agents = manager.list_all_agents();
        if agents.is_empty() {
            println!("   No agents currently running");
        } else {
            for (_, _, agent) in agents {
                println!("   - {} ({})", agent.id, agent.name);
            }
        }
    }
    
    Ok(())
}

async fn handle_agent_worktree(agent_id: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("🌳 Opening shell in agent {} worktree...", agent_id);
    
    // Find the agent and get its worktree path
    let manager = BunshinManager::new()?;
    if let Some((session_id, window_id, agent)) = manager.find_agent(&agent_id) {
        if let Some(worktree_path) = &agent.artifacts_path {
            if worktree_path.exists() {
                println!("📊 Agent Info:");
                println!("  ID: {}", agent.id);
                println!("  Name: {}", agent.name);
                println!("  Model: {}", agent.model);
                println!("  Worktree: {}", worktree_path.display());
                if let Some(project) = &agent.project {
                    println!("  Project: {}", project);
                }
                println!();
                
                // Change to the worktree directory and start a shell
                println!("🚀 Starting shell in worktree directory...");
                println!("💡 Claude Code should be available in this environment");
                println!("💡 Type 'exit' to return to bunshin");
                println!();
                
                use std::process::Command;
                use std::os::unix::process::CommandExt;
                
                // Set up environment variables for the shell
                let mut cmd = Command::new("/bin/bash");
                cmd.current_dir(worktree_path);
                
                // Set bunshin environment variables
                cmd.env("BUNSHIN_AGENT_ID", &agent.id);
                cmd.env("BUNSHIN_AGENT_NAME", &agent.name);
                cmd.env("BUNSHIN_SESSION_ID", session_id);
                cmd.env("BUNSHIN_WINDOW_ID", window_id);
                cmd.env("BUNSHIN_MODEL", agent.model.to_string());
                cmd.env("BUNSHIN_WORKTREE_PATH", worktree_path.to_string_lossy().as_ref());
                
                if let Some(project) = &agent.project {
                    cmd.env("BUNSHIN_PROJECT", project);
                }
                
                // Create a custom PS1 prompt to show we're in an agent worktree
                let custom_prompt = format!(
                    "\\[\\033[1;36m\\]bunshin-agent[{}]\\[\\033[0m\\]:\\[\\033[1;34m\\]\\w\\[\\033[0m\\]$ ",
                    agent.name
                );
                cmd.env("PS1", &custom_prompt);
                
                // Replace current process with the shell (like tmux behavior)
                unsafe {
                    let error = cmd.exec();
                    return Err(format!("Failed to exec shell: {}", error).into());
                }
            } else {
                println!("❌ Worktree path does not exist: {}", worktree_path.display());
                println!("💡 The agent's worktree may have been deleted or moved.");
                return Ok(());
            }
        } else {
            println!("❌ Agent {} does not have a worktree path", agent_id);
            println!("💡 This agent was not spawned with a project, so no isolated worktree was created.");
            println!("💡 Use 'bunshin spawn --project <project-name>' to create agents with worktrees.");
            return Ok(());
        }
    } else {
        println!("❌ Agent {} not found", agent_id);
        println!("📋 Available agents:");
        
        let agents = manager.list_all_agents();
        if agents.is_empty() {
            println!("   No agents currently exist");
        } else {
            for (_, _, agent) in agents {
                let worktree_status = if agent.artifacts_path.is_some() {
                    "has worktree"
                } else {
                    "no worktree"
                };
                println!("   - {} ({}) - {}", agent.id, agent.name, worktree_status);
            }
        }
        return Ok(());
    }
    
    Ok(())
}
