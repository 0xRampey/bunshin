use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    pub worktree_path: PathBuf,
    pub branch: String,
    pub repo_path: PathBuf,
    pub claude_pid: Option<u32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Session {
    pub fn new(name: String, worktree_path: PathBuf, branch: String, repo_path: PathBuf) -> Self {
        Self {
            name,
            worktree_path,
            branch,
            repo_path,
            claude_pid: None,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.claude_pid.is_some()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SessionManager {
    pub sessions: Vec<Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_session(&mut self, session: Session) {
        self.sessions.push(session);
    }

    pub fn remove_session(&mut self, name: &str) {
        self.sessions.retain(|s| s.name != name);
    }

    pub fn get_session(&self, name: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.name == name)
    }

    pub fn get_session_mut(&mut self, name: &str) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.name == name)
    }

    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let manager = serde_json::from_str(&content)?;
            Ok(manager)
        } else {
            Ok(Self::new())
        }
    }
}