use anyhow::Result;
use std::path::PathBuf;
use tokio::process::Command;

use crate::overlay::OverlayState;

pub struct AbducoSession {
    session_name: String,
    worktree_path: PathBuf,
    branch_name: String,
    socket_path: PathBuf,
    log_path: PathBuf,
    overlay_state: OverlayState,
}

impl AbducoSession {
    pub fn new(session_name: String, worktree_path: PathBuf, branch_name: String) -> Self {
        let base_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".bunshin");

        // Create socket path for abduco
        let socket_dir = base_dir.join("sessions");
        std::fs::create_dir_all(&socket_dir).ok();
        let socket_path = socket_dir.join(format!("{}.sock", session_name));

        // Create log path
        let log_dir = base_dir.join("logs");
        std::fs::create_dir_all(&log_dir).ok();
        let log_path = log_dir.join(format!("{}.log", session_name));

        let overlay_state = OverlayState::new(
            session_name.clone(),
            worktree_path.display().to_string(),
            branch_name.clone(),
        );

        Self {
            session_name,
            worktree_path,
            branch_name,
            socket_path,
            log_path,
            overlay_state,
        }
    }

    /// Check if abduco is installed
    pub fn check_abduco_installed() -> Result<PathBuf> {
        which::which("abduco").map_err(|_| {
            anyhow::anyhow!(
                "abduco not found. Please install it:\n\
                 \n\
                 macOS:   brew install abduco\n\
                 Linux:   sudo apt install abduco  (or yum, pacman, etc.)\n\
                 \n\
                 abduco provides lightweight session persistence for Bunshin."
            )
        })
    }

    /// Create a new abduco session (detached)
    pub async fn create(&self, claude_binary: PathBuf) -> Result<()> {
        Self::check_abduco_installed()?;

        println!("🚀 Creating persistent Bunshin session...");
        println!("📁 Session: {} | Branch: {}", self.session_name, self.branch_name);
        println!("📝 Logging to: {}", self.log_path.display());

        // Wrap Claude Code with script for logging, then run in abduco
        // macOS script syntax: script [-q] [-F] [file] [command]
        // -q: quiet mode (no "Script started" messages)
        // -F: flush output immediately (macOS uses -F, not -f)

        // Create detached abduco session running script+Claude Code
        // Use -n to create without attaching (session runs as daemon)
        let status = tokio::process::Command::new("abduco")
            .args([
                "-n",
                self.socket_path.to_str().unwrap(),
                "script",
                "-q",
                "-F",
                self.log_path.to_str().unwrap(),
                claude_binary.to_str().unwrap(),
            ])
            .current_dir(&self.worktree_path)
            .env("BUNSHIN_SESSION", &self.session_name)
            .env("BUNSHIN_WORKTREE", self.worktree_path.display().to_string())
            .env("BUNSHIN_BRANCH", &self.branch_name)
            // Disable Claude Code's built-in shpool integration
            .env_remove("SHPOOL_SESSION_NAME")
            .env_remove("SHPOOL_SOCKET")
            .env("SHPOOL_ENABLED", "false")
            .status()
            .await?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to create abduco session"));
        }

        // Wait for session to fully initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok(())
    }

    /// Attach to existing abduco session with overlay support
    pub async fn attach(&mut self) -> Result<()> {
        Self::check_abduco_installed()?;

        // Check if session exists
        if !self.socket_path.exists() {
            return Err(anyhow::anyhow!(
                "Session '{}' not found. Socket: {}",
                self.session_name,
                self.socket_path.display()
            ));
        }

        println!("🔗 Attaching to Bunshin session...");
        println!("📁 Session: {} | Branch: {}", self.session_name, self.branch_name);
        println!();

        // Display recent history from log file
        if self.log_path.exists() {
            self.display_recent_history(50).await?;
        } else {
            println!("📝 No previous history available");
        }

        println!();
        println!("💡 Press Ctrl-\\ to detach (session continues running)");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();

        // Attach to abduco session directly (let it handle the terminal)
        let status = Command::new("abduco")
            .args([
                "-a",
                self.socket_path.to_str().unwrap(),
            ])
            .status()
            .await?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to attach to abduco session"));
        }

        println!("✅ Detached from session");
        Ok(())
    }

    /// Display recent lines from the log file
    async fn display_recent_history(&self, lines: usize) -> Result<()> {
        use std::io::{BufRead, BufReader};
        use std::fs::File;

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        // Collect all lines
        let all_lines: Vec<String> = reader.lines()
            .filter_map(|line| line.ok())
            .collect();

        if all_lines.is_empty() {
            println!("📝 No history yet");
            return Ok(());
        }

        // Display header
        println!("📜 Previous session history (last {} lines):", lines);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Get last N lines
        let start_idx = if all_lines.len() > lines {
            all_lines.len() - lines
        } else {
            0
        };

        for line in &all_lines[start_idx..] {
            println!("{}", line);
        }

        Ok(())
    }

    /// List all abduco sessions
    pub fn list_sessions() -> Result<Vec<String>> {
        Self::check_abduco_installed()?;

        let socket_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".bunshin")
            .join("sessions");

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&socket_dir).ok();

        // List all .sock files in the directory
        let sessions: Vec<String> = std::fs::read_dir(&socket_dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();

                // Only include .sock files
                if path.extension()?.to_str()? == "sock" {
                    // Extract session name (remove .sock extension)
                    path.file_stem()?.to_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(sessions)
    }

    /// Kill an abduco session
    pub fn kill_session(session_name: &str) -> Result<()> {
        Self::check_abduco_installed()?;

        let socket_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".bunshin")
            .join("sessions");

        let socket_path = socket_dir.join(format!("{}.sock", session_name));

        if !socket_path.exists() {
            return Err(anyhow::anyhow!("Session '{}' not found", session_name));
        }

        // Kill by removing the socket (abduco will detect and terminate)
        std::fs::remove_file(&socket_path)?;

        Ok(())
    }

}
