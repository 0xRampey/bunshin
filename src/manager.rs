use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::core::{BunshinSession, Window, Agent, Project, AgentModel, AgentState};
use dirs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BunshinManager {
    pub sessions: HashMap<String, BunshinSession>,
    pub projects: HashMap<String, Project>,
    pub config_path: PathBuf,
    pub current_session_id: Option<String>,
    pub current_window_id: Option<String>,
}

impl BunshinManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path()?;
        let mut manager = Self {
            sessions: HashMap::new(),
            projects: HashMap::new(),
            config_path,
            current_session_id: None,
            current_window_id: None,
        };
        manager.load_from_disk()?;
        Ok(manager)
    }
    
    fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::home_dir()
            .ok_or("Could not find home directory")?
            .join(".bunshin");
        
        std::fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("manager.json"))
    }
    
    pub fn save_to_disk(&self) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.config_path, json)?;
        Ok(())
    }
    
    pub fn load_from_disk(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.config_path.exists() {
            let contents = std::fs::read_to_string(&self.config_path)?;
            let data: BunshinManager = serde_json::from_str(&contents)?;
            self.sessions = data.sessions;
            self.projects = data.projects;
        }
        Ok(())
    }
    
    // Session Management
    pub fn create_session(&mut self, name: String, worktree_path: PathBuf) -> String {
        let mut session = BunshinSession::new(name.clone());
        
        // Create a default window
        let window_id = session.add_window("main".to_string());
        let session_id = session.id.clone();
        
        self.sessions.insert(session_id.clone(), session);
        self.current_session_id = Some(session_id.clone());
        self.current_window_id = Some(window_id);
        
        session_id
    }
    
    pub fn get_session(&self, session_id: &str) -> Option<&BunshinSession> {
        self.sessions.get(session_id)
    }
    
    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut BunshinSession> {
        self.sessions.get_mut(session_id)
    }
    
    pub fn remove_session(&mut self, session_id: &str) -> Option<BunshinSession> {
        if Some(session_id) == self.current_session_id.as_deref() {
            self.current_session_id = None;
            self.current_window_id = None;
        }
        self.sessions.remove(session_id)
    }
    
    pub fn list_sessions(&self) -> Vec<&BunshinSession> {
        self.sessions.values().collect()
    }
    
    // Window Management
    pub fn create_window(&mut self, session_id: &str, name: String) -> Result<String, String> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            Ok(session.add_window(name))
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }
    
    pub fn get_window(&self, session_id: &str, window_id: &str) -> Option<&Window> {
        self.sessions.get(session_id)?.windows.get(window_id)
    }
    
    pub fn get_window_mut(&mut self, session_id: &str, window_id: &str) -> Option<&mut Window> {
        self.sessions.get_mut(session_id)?.windows.get_mut(window_id)
    }
    
    // Agent Management
    pub fn spawn_agent(
        &mut self, 
        session_id: &str, 
        window_id: &str, 
        name: String, 
        model: AgentModel
    ) -> Result<String, String> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            if let Some(window) = session.windows.get_mut(window_id) {
                Ok(window.add_agent(name, model))
            } else {
                Err(format!("Window {} not found in session {}", window_id, session_id))
            }
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }
    
    pub fn spawn_agents_in_current_window(
        &mut self,
        count: u32,
        model: AgentModel,
        project: Option<String>,
        labels: Vec<String>,
        task: Option<String>,
        tools: Vec<String>
    ) -> Result<Vec<(String, Option<PathBuf>)>, String> {
        let session_id = if let Some(id) = self.current_session_id.clone() {
            id
        } else {
            // Create a default session if none exists
            let default_session_id = self.create_session("default".to_string(), std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
            default_session_id
        };
        let window_id = self.current_window_id.clone()
            .ok_or("No current window")?;
        
        let mut agent_data = Vec::new();
        
        for i in 0..count {
            let name = format!("agent-{}", i + 1);
            let agent_id = self.spawn_agent(&session_id, &window_id, name, model.clone())?;
            
            // Create isolated worktree for this agent if project is specified
            let worktree_path = if let Some(ref project_name) = project {
                match self.create_agent_worktree(&agent_id, project_name) {
                    Ok(path) => {
                        println!("✅ Created worktree for agent {}: {}", agent_id, path.display());
                        Some(path)
                    }
                    Err(e) => {
                        println!("⚠️  Failed to create worktree for agent {}: {}", agent_id, e);
                        None
                    }
                }
            } else {
                None
            };
            
            // Configure the agent
            if let Some(agent) = self.get_agent_mut(&session_id, &window_id, &agent_id) {
                agent.project = project.clone();
                agent.labels = labels.clone();
                agent.task_description = task.clone();
                agent.tools = tools.clone();
                
                // Set the worktree path as the working directory
                if let Some(ref path) = worktree_path {
                    agent.artifacts_path = Some(path.clone());
                }
            }
            
            agent_data.push((agent_id, worktree_path));
        }
        
        Ok(agent_data)
    }
    
    fn create_agent_worktree(&self, agent_id: &str, project_name: &str) -> Result<PathBuf, String> {
        // Get project info to find the repository
        let project = self.get_project(project_name)
            .ok_or_else(|| format!("Project '{}' not found", project_name))?;
        
        // Determine repository path - for now, use current directory or specified repo
        let repo_path = if let Some(repo_url) = &project.repository {
            // For HTTP repos, we'd need to clone first - simplified for demo
            std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?
        } else {
            // Assume current directory is a git repo
            std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?
        };
        
        // Generate unique branch name for this agent
        let branch_name = format!("agent-{}-{}", agent_id, chrono::Utc::now().format("%Y%m%d-%H%M%S"));
        
        // Create worktree directory
        let worktree_base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".bunshin")
            .join("worktrees");
        
        std::fs::create_dir_all(&worktree_base)
            .map_err(|e| format!("Failed to create worktree base directory: {}", e))?;
            
        let worktree_path = worktree_base.join(&branch_name);
        
        // Create the worktree with new branch using our git utilities
        use crate::git::GitWorktree;
        GitWorktree::create_worktree(&repo_path, &worktree_path, &branch_name)
            .map_err(|e| format!("Failed to create git worktree: {}", e))?;
        
        Ok(worktree_path)
    }
    
    pub fn get_agent(&self, session_id: &str, window_id: &str, agent_id: &str) -> Option<&Agent> {
        self.sessions.get(session_id)?
            .windows.get(window_id)?
            .agents.get(agent_id)
    }
    
    pub fn get_agent_mut(&mut self, session_id: &str, window_id: &str, agent_id: &str) -> Option<&mut Agent> {
        self.sessions.get_mut(session_id)?
            .windows.get_mut(window_id)?
            .agents.get_mut(agent_id)
    }
    
    pub fn find_agent(&self, agent_id: &str) -> Option<(&str, &str, &Agent)> {
        for (session_id, session) in &self.sessions {
            for (window_id, window) in &session.windows {
                if let Some(agent) = window.agents.get(agent_id) {
                    return Some((session_id, window_id, agent));
                }
            }
        }
        None
    }
    
    pub fn find_agent_mut(&mut self, agent_id: &str) -> Option<(&str, &str, &mut Agent)> {
        for (session_id, session) in &mut self.sessions {
            for (window_id, window) in &mut session.windows {
                if let Some(agent) = window.agents.get_mut(agent_id) {
                    return Some((session_id, window_id, agent));
                }
            }
        }
        None
    }
    
    pub fn list_agents_in_session(&self, session_id: &str) -> Vec<&Agent> {
        if let Some(session) = self.sessions.get(session_id) {
            session.windows.values()
                .flat_map(|w| w.agents.values())
                .collect()
        } else {
            Vec::new()
        }
    }
    
    pub fn list_all_agents(&self) -> Vec<(&str, &str, &Agent)> {
        let mut agents = Vec::new();
        for (session_id, session) in &self.sessions {
            for (window_id, window) in &session.windows {
                for (_, agent) in &window.agents {
                    agents.push((session_id.as_str(), window_id.as_str(), agent));
                }
            }
        }
        agents
    }
    
    pub fn kill_agent(&mut self, agent_id: &str) -> Result<(), String> {
        if let Some((session_id, window_id, agent)) = self.find_agent_mut(agent_id) {
            if let Some(pid) = agent.pid {
                // Kill the actual process
                #[cfg(unix)]
                {
                    use std::process::Command;
                    Command::new("kill")
                        .arg("-TERM")
                        .arg(&pid.to_string())
                        .output()
                        .map_err(|e| format!("Failed to kill process {}: {}", pid, e))?;
                }
            }
            
            agent.stop();
            Ok(())
        } else {
            Err(format!("Agent {} not found", agent_id))
        }
    }
    
    // Project Management
    pub fn create_project(&mut self, project: Project) -> Result<(), String> {
        if self.projects.contains_key(&project.name) {
            return Err(format!("Project '{}' already exists", project.name));
        }
        self.projects.insert(project.name.clone(), project);
        Ok(())
    }
    
    pub fn get_project(&self, name: &str) -> Option<&Project> {
        self.projects.get(name)
    }
    
    pub fn list_projects(&self) -> Vec<&Project> {
        self.projects.values().collect()
    }
    
    pub fn update_project(&mut self, name: &str, description: Option<String>, repository: Option<String>, add_labels: Vec<String>, remove_labels: Vec<String>) -> Result<(), String> {
        if let Some(project) = self.projects.get_mut(name) {
            if let Some(desc) = description {
                project.description = Some(desc);
            }
            if let Some(repo) = repository {
                project.repository = Some(repo);
            }
            for label in add_labels {
                if !project.labels.contains(&label) {
                    project.labels.push(label);
                }
            }
            project.labels.retain(|l| !remove_labels.contains(l));
            Ok(())
        } else {
            Err(format!("Project '{}' not found", name))
        }
    }
    
    pub fn delete_project(&mut self, name: &str) -> Result<(), String> {
        if self.projects.remove(name).is_some() {
            Ok(())
        } else {
            Err(format!("Project '{}' not found", name))
        }
    }
    
    // Context Management
    pub fn set_current_context(&mut self, session_id: Option<String>, window_id: Option<String>) {
        self.current_session_id = session_id;
        self.current_window_id = window_id;
    }
    
    pub fn get_current_context(&self) -> (Option<&str>, Option<&str>) {
        (
            self.current_session_id.as_deref(),
            self.current_window_id.as_deref()
        )
    }
    
    // Fleet Operations
    pub fn broadcast_to_project(&mut self, project_name: &str, message: &str) -> Result<Vec<String>, String> {
        let mut agent_ids = Vec::new();
        
        for (_, session) in &mut self.sessions {
            for (_, window) in &mut session.windows {
                for (agent_id, agent) in &mut window.agents {
                    if agent.project.as_deref() == Some(project_name) {
                        // TODO: Actually send message to agent process
                        agent.last_activity = Utc::now();
                        agent_ids.push(agent_id.clone());
                    }
                }
            }
        }
        
        if agent_ids.is_empty() {
            Err(format!("No agents found in project '{}'", project_name))
        } else {
            Ok(agent_ids)
        }
    }
    
    pub fn broadcast_to_labels(&mut self, labels: &[String], message: &str) -> Result<Vec<String>, String> {
        let mut agent_ids = Vec::new();
        
        for (_, session) in &mut self.sessions {
            for (_, window) in &mut session.windows {
                for (agent_id, agent) in &mut window.agents {
                    if labels.iter().any(|label| agent.labels.contains(label)) {
                        // TODO: Actually send message to agent process
                        agent.last_activity = Utc::now();
                        agent_ids.push(agent_id.clone());
                    }
                }
            }
        }
        
        if agent_ids.is_empty() {
            Err("No agents found with matching labels".to_string())
        } else {
            Ok(agent_ids)
        }
    }
}

impl Default for BunshinManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            sessions: HashMap::new(),
            projects: HashMap::new(),
            config_path: PathBuf::from(".bunshin-manager.json"),
            current_session_id: None,
            current_window_id: None,
        })
    }
}