use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::path::PathBuf;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::process::Command as AsyncCommand;
use crate::core::{Agent, AgentModel, AgentState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    pub working_directory: PathBuf,
    pub environment_vars: HashMap<String, String>,
    pub max_memory_mb: Option<u64>,
    pub timeout_seconds: Option<u64>,
    pub restart_on_failure: bool,
    pub log_file: Option<PathBuf>,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            environment_vars: HashMap::new(),
            max_memory_mb: Some(2048), // 2GB default limit
            timeout_seconds: Some(3600), // 1 hour default timeout
            restart_on_failure: false,
            log_file: None,
        }
    }
}

#[derive(Debug)]
pub struct ProcessManager {
    processes: HashMap<String, ManagedProcess>,
    logs_dir: PathBuf,
}

#[derive(Debug)]
struct ManagedProcess {
    child: Child,
    config: ProcessConfig,
    started_at: Instant,
    stdin_sender: Option<mpsc::Sender<String>>,
    stdout_receiver: Option<mpsc::Receiver<String>>,
    stderr_receiver: Option<mpsc::Receiver<String>>,
}

impl ProcessManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let logs_dir = dirs::home_dir()
            .ok_or("Could not find home directory")?
            .join(".bunshin")
            .join("logs");
        
        std::fs::create_dir_all(&logs_dir)?;
        
        Ok(Self {
            processes: HashMap::new(),
            logs_dir,
        })
    }
    
    pub fn spawn_agent_process(
        &mut self,
        agent: &mut Agent,
        config: ProcessConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let command_args = self.build_agent_command(agent)?;
        
        // Set up log file
        let log_file = if let Some(log_path) = &config.log_file {
            log_path.clone()
        } else {
            self.logs_dir.join(format!("{}.log", agent.id))
        };
        
        // Determine working directory - use agent's worktree if available
        let working_directory = if let Some(ref worktree_path) = agent.artifacts_path {
            worktree_path.clone()
        } else {
            config.working_directory.clone()
        };
        
        // Create the process command
        let mut cmd = Command::new(&command_args[0]);
        cmd.args(&command_args[1..])
            .current_dir(&working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        // Set environment variables
        for (key, value) in &config.environment_vars {
            cmd.env(key, value);
        }
        
        // Add bunshin-specific environment variables
        cmd.env("BUNSHIN_AGENT_ID", &agent.id)
            .env("BUNSHIN_AGENT_NAME", &agent.name)
            .env("BUNSHIN_SESSION_ID", &agent.session_id)
            .env("BUNSHIN_WINDOW_ID", &agent.window_id)
            .env("BUNSHIN_MODEL", agent.model.to_string());
        
        if let Some(project) = &agent.project {
            cmd.env("BUNSHIN_PROJECT", project);
        }
        
        if let Some(task) = &agent.task_description {
            cmd.env("BUNSHIN_TASK", task);
        }
        
        // Spawn the process
        agent.start();
        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to spawn agent process: {}", e))?;
        
        let pid = child.id();
        agent.set_running(pid);
        
        // Set up communication channels
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        
        let (stdin_sender, stdin_receiver) = mpsc::channel::<String>();
        let (stdout_sender, stdout_receiver) = mpsc::channel::<String>();
        let (stderr_sender, stderr_receiver) = mpsc::channel::<String>();
        
        // Spawn stdin handler thread
        let mut stdin_writer = stdin;
        thread::spawn(move || {
            while let Ok(input) = stdin_receiver.recv() {
                if let Err(_) = writeln!(stdin_writer, "{}", input) {
                    break;
                }
                if let Err(_) = stdin_writer.flush() {
                    break;
                }
            }
        });
        
        // Spawn stdout handler thread
        let log_file_stdout = log_file.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            let mut log_writer = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file_stdout)
                .ok();
            
            for line in reader.lines() {
                if let Ok(line) = line {
                    // Send to channel
                    if stdout_sender.send(line.clone()).is_err() {
                        break;
                    }
                    
                    // Write to log file
                    if let Some(ref mut writer) = log_writer {
                        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
                        let _ = writeln!(writer, "[{}] [STDOUT] {}", timestamp, line);
                        let _ = writer.flush();
                    }
                }
            }
        });
        
        // Spawn stderr handler thread
        let log_file_stderr = log_file.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let mut log_writer = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file_stderr)
                .ok();
            
            for line in reader.lines() {
                if let Ok(line) = line {
                    // Send to channel
                    if stderr_sender.send(line.clone()).is_err() {
                        break;
                    }
                    
                    // Write to log file
                    if let Some(ref mut writer) = log_writer {
                        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
                        let _ = writeln!(writer, "[{}] [STDERR] {}", timestamp, line);
                        let _ = writer.flush();
                    }
                }
            }
        });
        
        // Create managed process
        let managed = ManagedProcess {
            child,
            config,
            started_at: Instant::now(),
            stdin_sender: Some(stdin_sender),
            stdout_receiver: Some(stdout_receiver),
            stderr_receiver: Some(stderr_receiver),
        };
        
        self.processes.insert(agent.id.clone(), managed);
        
        println!("✅ Spawned agent {} (PID: {})", agent.name, pid);
        println!("   Log file: {}", log_file.display());
        
        Ok(())
    }
    
    fn build_agent_command(&self, agent: &Agent) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        match agent.model {
            AgentModel::ClaudeCode => {
                // Launch Claude Code - check if claude command exists, fallback to demo mode
                if std::process::Command::new("claude").arg("--version").output().is_ok() {
                    Ok(vec![
                        "claude".to_string(),
                    ])
                } else {
                    // Fallback to our Python wrapper simulating Claude Code
                    Ok(vec![
                        "python3".to_string(),
                        "/Users/rampey/Documents/bunshin/bunshin_agent.py".to_string(),
                        "--model".to_string(),
                        "claude-code".to_string(),
                    ])
                }
            }
            AgentModel::Claude35Sonnet | AgentModel::Claude35Haiku => {
                // Use a custom wrapper script or API client
                Ok(vec![
                    "python3".to_string(),
                    "-m".to_string(),
                    "bunshin_agent".to_string(),
                    "--model".to_string(),
                    agent.model.to_string(),
                ])
            }
            AgentModel::Gpt4o | AgentModel::Gpt4oMini => {
                // Use OpenAI API wrapper
                Ok(vec![
                    "python3".to_string(),
                    "-m".to_string(),
                    "bunshin_agent".to_string(),
                    "--model".to_string(),
                    agent.model.to_string(),
                ])
            }
            AgentModel::Custom(ref model_name) => {
                // Use custom command specified in model name
                if model_name.starts_with("cmd:") {
                    let cmd = &model_name[4..];
                    let parts: Vec<String> = cmd.split_whitespace()
                        .map(|s| s.to_string())
                        .collect();
                    if parts.is_empty() {
                        return Err("Empty custom command".into());
                    }
                    Ok(parts)
                } else {
                    // Default to python wrapper with custom model
                    Ok(vec![
                        "python3".to_string(),
                        "-m".to_string(),
                        "bunshin_agent".to_string(),
                        "--model".to_string(),
                        model_name.clone(),
                    ])
                }
            }
        }
    }
    
    pub fn send_input(&mut self, agent_id: &str, input: &str) -> Result<(), String> {
        if let Some(process) = self.processes.get_mut(agent_id) {
            if let Some(sender) = &process.stdin_sender {
                sender.send(input.to_string())
                    .map_err(|_| "Failed to send input to agent process".to_string())?;
                Ok(())
            } else {
                Err("Agent process stdin not available".to_string())
            }
        } else {
            Err(format!("Agent process {} not found", agent_id))
        }
    }
    
    pub fn read_output(&mut self, agent_id: &str, max_lines: Option<usize>) -> Result<Vec<String>, String> {
        if let Some(process) = self.processes.get_mut(agent_id) {
            let mut lines = Vec::new();
            let limit = max_lines.unwrap_or(100);
            
            if let Some(receiver) = &process.stdout_receiver {
                while lines.len() < limit {
                    match receiver.try_recv() {
                        Ok(line) => lines.push(line),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => break,
                    }
                }
            }
            
            Ok(lines)
        } else {
            Err(format!("Agent process {} not found", agent_id))
        }
    }
    
    pub fn read_errors(&mut self, agent_id: &str, max_lines: Option<usize>) -> Result<Vec<String>, String> {
        if let Some(process) = self.processes.get_mut(agent_id) {
            let mut lines = Vec::new();
            let limit = max_lines.unwrap_or(100);
            
            if let Some(receiver) = &process.stderr_receiver {
                while lines.len() < limit {
                    match receiver.try_recv() {
                        Ok(line) => lines.push(line),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => break,
                    }
                }
            }
            
            Ok(lines)
        } else {
            Err(format!("Agent process {} not found", agent_id))
        }
    }
    
    pub fn kill_agent(&mut self, agent_id: &str, agent: &mut Agent) -> Result<(), String> {
        if let Some(mut process) = self.processes.remove(agent_id) {
            // Try graceful termination first
            let _ = process.child.kill();
            
            // Wait for process to exit
            match process.child.wait() {
                Ok(exit_status) => {
                    agent.stop();
                    println!("🛑 Agent {} terminated (exit code: {:?})", agent.name, exit_status.code());
                    Ok(())
                }
                Err(e) => {
                    agent.set_error(format!("Failed to wait for process termination: {}", e));
                    Err(format!("Failed to terminate agent process: {}", e))
                }
            }
        } else {
            // Agent might not have a process or already terminated
            agent.stop();
            Ok(())
        }
    }
    
    pub fn is_running(&mut self, agent_id: &str) -> bool {
        if let Some(process) = self.processes.get_mut(agent_id) {
            match process.child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited
                    false
                }
                Ok(None) => {
                    // Process is still running
                    true
                }
                Err(_) => {
                    // Error checking process status
                    false
                }
            }
        } else {
            false
        }
    }
    
    pub fn restart_agent(
        &mut self,
        agent_id: &str,
        agent: &mut Agent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get the config before killing
        let config = if let Some(process) = self.processes.get(agent_id) {
            process.config.clone()
        } else {
            ProcessConfig::default()
        };
        
        // Kill the existing process
        let _ = self.kill_agent(agent_id, agent);
        
        // Small delay before restart
        std::thread::sleep(Duration::from_millis(100));
        
        // Spawn new process
        self.spawn_agent_process(agent, config)
    }
    
    pub fn cleanup_dead_processes(&mut self) -> Vec<String> {
        let mut dead_agents = Vec::new();
        
        self.processes.retain(|agent_id, process| {
            match process.child.try_wait() {
                Ok(Some(exit_status)) => {
                    println!("💀 Agent {} exited (code: {:?})", agent_id, exit_status.code());
                    dead_agents.push(agent_id.clone());
                    false // Remove from map
                }
                Ok(None) => true, // Still running
                Err(_) => {
                    println!("❌ Error checking agent {} status", agent_id);
                    dead_agents.push(agent_id.clone());
                    false // Remove from map
                }
            }
        });
        
        dead_agents
    }
    
    pub fn get_process_stats(&self, agent_id: &str) -> Option<ProcessStats> {
        if let Some(process) = self.processes.get(agent_id) {
            Some(ProcessStats {
                pid: process.child.id(),
                uptime: process.started_at.elapsed(),
                log_file: process.config.log_file.clone(),
            })
        } else {
            None
        }
    }
    
    pub fn list_running_processes(&self) -> Vec<String> {
        self.processes.keys().cloned().collect()
    }
    
    pub fn broadcast_message(&mut self, agent_ids: &[String], message: &str) -> Result<Vec<String>, String> {
        let mut successful = Vec::new();
        
        for agent_id in agent_ids {
            match self.send_input(agent_id, message) {
                Ok(()) => successful.push(agent_id.clone()),
                Err(e) => {
                    println!("Failed to send message to {}: {}", agent_id, e);
                }
            }
        }
        
        Ok(successful)
    }
    
    pub fn tail_logs(&self, agent_id: &str, lines: u32, follow: bool) -> Result<(), String> {
        let process = self.processes.get(agent_id)
            .ok_or_else(|| format!("Agent {} not found or not running", agent_id))?;
        
        let log_file = if let Some(ref path) = process.config.log_file {
            path.clone()
        } else {
            self.logs_dir.join(format!("{}.log", agent_id))
        };
        
        if !log_file.exists() {
            return Err(format!("Log file not found: {}", log_file.display()));
        }
        
        if follow {
            // Use tail -f equivalent
            let mut cmd = Command::new("tail")
                .arg("-f")
                .arg("-n")
                .arg(&lines.to_string())
                .arg(&log_file)
                .spawn()
                .map_err(|e| format!("Failed to tail log file: {}", e))?;
            
            let _ = cmd.wait();
        } else {
            // Read last N lines
            let output = Command::new("tail")
                .arg("-n")
                .arg(&lines.to_string())
                .arg(&log_file)
                .output()
                .map_err(|e| format!("Failed to read log file: {}", e))?;
            
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        
        Ok(())
    }
}

#[derive(Debug)]
pub struct ProcessStats {
    pub pid: u32,
    pub uptime: Duration,
    pub log_file: Option<PathBuf>,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            processes: HashMap::new(),
            logs_dir: PathBuf::from("/tmp/bunshin-logs"),
        })
    }
}