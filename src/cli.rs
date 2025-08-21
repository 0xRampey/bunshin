use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;
use crate::core::AgentModel;

#[derive(Parser)]
#[command(name = "bunshin")]
#[command(about = "Multi-agent orchestration and git worktree session manager")]
#[command(long_about = "Bunshin manages sessions, windows, and agents across git worktrees with Claude Code and other AI models")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all sessions
    Ls {
        /// Show all sessions (default shows only active)
        #[arg(short, long)]
        all: bool,
        
        /// Filter by project
        #[arg(short, long)]
        project: Option<String>,
        
        /// Output format (table, json, compact)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    
    /// List agents in current or specified session
    Ps {
        /// Session ID to list agents from
        #[arg(short, long)]
        session: Option<String>,
        
        /// Show all agents across all sessions
        #[arg(short, long)]
        all: bool,
        
        /// Output format (table, json, compact)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    
    /// Spawn new agents
    Spawn {
        /// AI model to use
        #[arg(short, long, default_value = "claude-code")]
        model: String,
        
        /// Number of agents to spawn
        #[arg(short, long, default_value = "1")]
        count: u32,
        
        /// Project tag for organization
        #[arg(short, long)]
        project: Option<String>,
        
        /// Labels for categorization
        #[arg(short, long)]
        labels: Vec<String>,
        
        /// Window to spawn agents in (defaults to current)
        #[arg(short, long)]
        window: Option<String>,
        
        /// Task description for the agents
        #[arg(short, long)]
        task: Option<String>,
        
        /// Tools to enable for agents
        #[arg(long)]
        tools: Vec<String>,
    },
    
    /// Clone existing agent configuration
    Clone {
        /// Agent ID to clone from
        agent_id: String,
        
        /// Number of clones to create
        #[arg(short, long, default_value = "1")]
        count: u32,
        
        /// New project tag (inherits from original if not specified)
        #[arg(short, long)]
        project: Option<String>,
    },
    
    /// Attach to specific session, window, or agent
    Attach {
        /// Target to attach to (session-id, window-id, or agent-id)
        target: String,
    },
    
    /// Connect to agent's interactive shell
    Shell {
        /// Agent ID to connect to
        agent_id: String,
    },
    
    /// Shell into agent's worktree directory
    Worktree {
        /// Agent ID to shell into worktree
        agent_id: String,
    },
    
    /// Kill agents or sessions
    Kill {
        /// Targets to kill (agent-id, window-id, or session-id)
        targets: Vec<String>,
        
        /// Kill all agents in current session
        #[arg(long)]
        all: bool,
        
        /// Force kill without confirmation
        #[arg(short, long)]
        force: bool,
    },
    
    /// Broadcast command to multiple agents
    Broadcast {
        /// Target scope (session, window, project, or specific agents)
        #[arg(short, long)]
        scope: Option<String>,
        
        /// Project to broadcast to
        #[arg(short, long)]
        project: Option<String>,
        
        /// Window to broadcast to
        #[arg(short, long)]
        window: Option<String>,
        
        /// Labels to filter agents
        #[arg(short, long)]
        labels: Vec<String>,
        
        /// The command/message to broadcast
        message: String,
    },
    
    /// Create new session, window, or project
    New {
        #[command(subcommand)]
        entity: NewEntity,
    },
    
    /// Show detailed information about entities
    Info {
        /// Entity ID to show info for
        target: String,
    },
    
    /// Manage projects
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    
    /// Show session manager TUI (legacy compatibility)
    Manager,
    
    /// Tail logs from agents
    Logs {
        /// Agent ID to tail logs from
        agent_id: String,
        
        /// Number of lines to show
        #[arg(short, long, default_value = "50")]
        lines: u32,
        
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },
    
    /// Export session data
    Export {
        /// Session ID to export
        session_id: String,
        
        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Export format (json, yaml, tar)
        #[arg(short, long, default_value = "json")]
        format: String,
    },
    
    /// Import session data
    Import {
        /// Input file path
        input: PathBuf,
        
        /// Merge with existing sessions
        #[arg(short, long)]
        merge: bool,
    },
}

#[derive(Subcommand)]
pub enum NewEntity {
    /// Create new session
    Session {
        /// Session name
        name: String,
        
        /// Git repository path
        #[arg(short, long)]
        repo: PathBuf,
        
        /// Git branch (creates if doesn't exist)
        #[arg(short, long)]
        branch: String,
        
        /// Project tag
        #[arg(short, long)]
        project: Option<String>,
    },
    
    /// Create new window in current session
    Window {
        /// Window name
        name: String,
        
        /// Session ID (defaults to current)
        #[arg(short, long)]
        session: Option<String>,
        
        /// Project tag
        #[arg(short, long)]
        project: Option<String>,
        
        /// Working directory override
        #[arg(short, long)]
        dir: Option<PathBuf>,
    },
    
    /// Create new project
    Project {
        /// Project name
        name: String,
        
        /// Project description
        #[arg(short, long)]
        description: Option<String>,
        
        /// Git repository URL
        #[arg(short, long)]
        repo: Option<String>,
        
        /// Labels for categorization
        #[arg(short, long)]
        labels: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum ProjectAction {
    /// List all projects
    List,
    
    /// Show project details
    Show {
        /// Project name
        name: String,
    },
    
    /// Update project information
    Update {
        /// Project name
        name: String,
        
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        
        /// New repository URL
        #[arg(short, long)]
        repo: Option<String>,
        
        /// Add labels
        #[arg(long)]
        add_labels: Vec<String>,
        
        /// Remove labels
        #[arg(long)]
        remove_labels: Vec<String>,
    },
    
    /// Delete project
    Delete {
        /// Project name
        name: String,
        
        /// Force delete without confirmation
        #[arg(short, long)]
        force: bool,
    },
}

impl From<String> for AgentModel {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(AgentModel::Custom(s))
    }
}