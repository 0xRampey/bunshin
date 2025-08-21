use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BunshinSession {
    pub id: String,
    pub name: String,
    pub windows: HashMap<String, Window>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub id: String,
    pub name: String,
    pub session_id: String,
    pub agents: HashMap<String, Agent>,
    pub project: Option<String>,
    pub labels: Vec<String>,
    pub worktree_path: Option<PathBuf>,
    pub branch: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub window_id: String,
    pub session_id: String,
    pub model: AgentModel,
    pub state: AgentState,
    pub project: Option<String>,
    pub labels: Vec<String>,
    pub pid: Option<u32>,
    pub tokens_used: u64,
    pub estimated_cost: f64,
    pub uptime_start: Option<chrono::DateTime<chrono::Utc>>,
    pub task_description: Option<String>,
    pub tools: Vec<String>,
    pub artifacts_path: Option<PathBuf>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentModel {
    ClaudeCode,
    Claude35Sonnet,
    Claude35Haiku,
    Gpt4o,
    Gpt4oMini,
    Custom(String),
}

impl std::fmt::Display for AgentModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentModel::ClaudeCode => write!(f, "claude-code"),
            AgentModel::Claude35Sonnet => write!(f, "claude-3.5-sonnet"),
            AgentModel::Claude35Haiku => write!(f, "claude-3.5-haiku"),
            AgentModel::Gpt4o => write!(f, "gpt-4o"),
            AgentModel::Gpt4oMini => write!(f, "gpt-4o-mini"),
            AgentModel::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl std::str::FromStr for AgentModel {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "claude-code" => Ok(AgentModel::ClaudeCode),
            "claude-3.5-sonnet" | "claude-3.5" => Ok(AgentModel::Claude35Sonnet),
            "claude-3.5-haiku" => Ok(AgentModel::Claude35Haiku),
            "gpt-4o" => Ok(AgentModel::Gpt4o),
            "gpt-4o-mini" => Ok(AgentModel::Gpt4oMini),
            custom => Ok(AgentModel::Custom(custom.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentState {
    Starting,
    Running,
    Idle,
    Stopping,
    Stopped,
    Error(String),
}

impl std::fmt::Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentState::Starting => write!(f, "Starting"),
            AgentState::Running => write!(f, "Running"),
            AgentState::Idle => write!(f, "Idle"),
            AgentState::Stopping => write!(f, "Stopping"),
            AgentState::Stopped => write!(f, "Stopped"),
            AgentState::Error(err) => write!(f, "Error: {}", err),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub description: Option<String>,
    pub repository: Option<String>,
    pub labels: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCap {
    pub session_id: String,
    pub max_cost: Option<f64>,
    pub max_tokens: Option<u64>,
    pub current_cost: f64,
    pub current_tokens: u64,
}

impl BunshinSession {
    pub fn new(name: String) -> Self {
        let id = format!("s-{}", Uuid::new_v4().simple().to_string()[..8].to_lowercase());
        Self {
            id,
            name,
            windows: HashMap::new(),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            is_active: false,
        }
    }

    pub fn add_window(&mut self, name: String) -> String {
        let window = Window::new(name, self.id.clone());
        let window_id = window.id.clone();
        self.windows.insert(window_id.clone(), window);
        window_id
    }

    pub fn get_window_mut(&mut self, window_id: &str) -> Option<&mut Window> {
        self.windows.get_mut(window_id)
    }

    pub fn total_agents(&self) -> usize {
        self.windows.values().map(|w| w.agents.len()).sum()
    }

    pub fn total_cost(&self) -> f64 {
        self.windows.values()
            .flat_map(|w| w.agents.values())
            .map(|a| a.estimated_cost)
            .sum()
    }

    pub fn total_tokens(&self) -> u64 {
        self.windows.values()
            .flat_map(|w| w.agents.values())
            .map(|a| a.tokens_used)
            .sum()
    }
}

impl Window {
    pub fn new(name: String, session_id: String) -> Self {
        let id = format!("w-{}", Uuid::new_v4().simple().to_string()[..8].to_lowercase());
        Self {
            id,
            name,
            session_id,
            agents: HashMap::new(),
            project: None,
            labels: Vec::new(),
            worktree_path: None,
            branch: None,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn add_agent(&mut self, name: String, model: AgentModel) -> String {
        let agent = Agent::new(name, self.id.clone(), self.session_id.clone(), model);
        let agent_id = agent.id.clone();
        self.agents.insert(agent_id.clone(), agent);
        agent_id
    }

    pub fn get_agent_mut(&mut self, agent_id: &str) -> Option<&mut Agent> {
        self.agents.get_mut(agent_id)
    }
}

impl Agent {
    pub fn new(name: String, window_id: String, session_id: String, model: AgentModel) -> Self {
        let id = format!("a-{}", Uuid::new_v4().simple().to_string()[..4].to_lowercase());
        Self {
            id,
            name,
            window_id,
            session_id,
            model,
            state: AgentState::Stopped,
            project: None,
            labels: Vec::new(),
            pid: None,
            tokens_used: 0,
            estimated_cost: 0.0,
            uptime_start: None,
            task_description: None,
            tools: Vec::new(),
            artifacts_path: None,
            created_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
        }
    }

    pub fn uptime(&self) -> Option<chrono::Duration> {
        self.uptime_start.map(|start| chrono::Utc::now() - start)
    }

    pub fn uptime_string(&self) -> String {
        self.uptime()
            .map(|duration| {
                let hours = duration.num_hours();
                let minutes = duration.num_minutes() % 60;
                let seconds = duration.num_seconds() % 60;
                format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
            })
            .unwrap_or_else(|| "00:00:00".to_string())
    }

    pub fn start(&mut self) {
        self.state = AgentState::Starting;
        self.uptime_start = Some(chrono::Utc::now());
        self.last_activity = chrono::Utc::now();
    }

    pub fn set_running(&mut self, pid: u32) {
        self.state = AgentState::Running;
        self.pid = Some(pid);
        self.last_activity = chrono::Utc::now();
    }

    pub fn set_idle(&mut self) {
        self.state = AgentState::Idle;
        self.last_activity = chrono::Utc::now();
    }

    pub fn stop(&mut self) {
        self.state = AgentState::Stopped;
        self.pid = None;
        self.uptime_start = None;
    }

    pub fn set_error(&mut self, error: String) {
        self.state = AgentState::Error(error);
        self.last_activity = chrono::Utc::now();
    }

    pub fn add_tokens(&mut self, tokens: u64) {
        self.tokens_used += tokens;
        self.estimated_cost += self.calculate_token_cost(tokens);
        self.last_activity = chrono::Utc::now();
    }

    fn calculate_token_cost(&self, tokens: u64) -> f64 {
        let cost_per_1k_tokens = match self.model {
            AgentModel::ClaudeCode => 0.003,        // Estimated
            AgentModel::Claude35Sonnet => 0.003,    // $3/1M tokens
            AgentModel::Claude35Haiku => 0.00025,   // $0.25/1M tokens
            AgentModel::Gpt4o => 0.015,             // $15/1M tokens (input)
            AgentModel::Gpt4oMini => 0.00015,       // $0.15/1M tokens
            AgentModel::Custom(_) => 0.002,         // Default estimate
        };
        
        (tokens as f64 / 1000.0) * cost_per_1k_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let mut session = BunshinSession::new("test-session".to_string());
        assert_eq!(session.name, "test-session");
        assert!(session.id.starts_with("s-"));
        assert_eq!(session.windows.len(), 0);
        
        let window_id = session.add_window("main".to_string());
        assert_eq!(session.windows.len(), 1);
        assert!(window_id.starts_with("w-"));
    }

    #[test]
    fn test_window_creation() {
        let mut window = Window::new("test-window".to_string(), "s-test".to_string());
        assert_eq!(window.name, "test-window");
        assert!(window.id.starts_with("w-"));
        
        let agent_id = window.add_agent("agent1".to_string(), AgentModel::ClaudeCode);
        assert_eq!(window.agents.len(), 1);
        assert!(agent_id.starts_with("a-"));
    }

    #[test]
    fn test_agent_lifecycle() {
        let mut agent = Agent::new(
            "test-agent".to_string(),
            "w-test".to_string(),
            "s-test".to_string(),
            AgentModel::ClaudeCode
        );
        
        assert!(matches!(agent.state, AgentState::Stopped));
        assert!(agent.uptime().is_none());
        
        agent.start();
        assert!(matches!(agent.state, AgentState::Starting));
        assert!(agent.uptime().is_some());
        
        agent.set_running(12345);
        assert!(matches!(agent.state, AgentState::Running));
        assert_eq!(agent.pid, Some(12345));
        
        agent.add_tokens(1000);
        assert_eq!(agent.tokens_used, 1000);
        assert!(agent.estimated_cost > 0.0);
    }

    #[test]
    fn test_agent_model_parsing() {
        assert!(matches!("claude-code".parse::<AgentModel>().unwrap(), AgentModel::ClaudeCode));
        assert!(matches!("gpt-4o".parse::<AgentModel>().unwrap(), AgentModel::Gpt4o));
        assert!(matches!("custom-model".parse::<AgentModel>().unwrap(), AgentModel::Custom(_)));
    }
}