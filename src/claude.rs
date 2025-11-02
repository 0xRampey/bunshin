use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use crate::session::Session;

pub struct ClaudeCodeManager;

impl ClaudeCodeManager {
    /// Find the claude binary in multiple possible locations (public API)
    pub fn find_claude_binary_public() -> Option<PathBuf> {
        Self::find_claude_binary()
    }

    /// Find the claude binary in multiple possible locations
    fn find_claude_binary() -> Option<PathBuf> {
        // Try multiple locations in order of preference
        let candidates = vec![
            // 1. Check if 'claude' is in PATH
            which::which("claude").ok(),
            // 2. Check homebrew location (macOS)
            Some(PathBuf::from("/opt/homebrew/bin/claude")),
            // 3. Check common homebrew location (Intel Mac)
            Some(PathBuf::from("/usr/local/bin/claude")),
            // 4. Check user's local bin
            std::env::var("HOME").ok().map(|home| PathBuf::from(home).join(".local/bin/claude")),
            // 5. Check legacy location
            std::env::var("HOME").ok().map(|home| PathBuf::from(home).join(".claude/local/claude")),
        ];

        for candidate in candidates.into_iter().flatten() {
            if candidate.exists() {
                return Some(candidate);
            }
        }

        None
    }

    /// Check if Claude Code is available on the system
    pub fn is_available() -> bool {
        Self::find_claude_binary().is_some()
    }

    /// Launch Claude Code as a background process (for TUI)
    pub fn launch_claude_code(session: &mut Session) -> Result<(), Box<dyn std::error::Error>> {
        if session.claude_pid.is_some() {
            return Err("Claude Code is already running for this session".into());
        }

        // Find claude binary
        let claude_path = Self::find_claude_binary()
            .ok_or_else(|| {
                "Claude Code binary not found. Install it with: brew install claude-code\nOr download from: https://claude.ai/download"
            })?;

        // Check if worktree exists
        if !session.worktree_path.exists() {
            return Err("Worktree directory does not exist".into());
        }

        let child = Command::new(&claude_path)
            .current_dir(&session.worktree_path)
            .spawn()?;

        session.claude_pid = Some(child.id());

        Ok(())
    }

    /// Launch Claude Code in interactive mode (replaces current process)
    pub fn launch_claude_code_interactive(worktree_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::process::CommandExt;

        // Find claude binary
        let claude_path = Self::find_claude_binary()
            .ok_or_else(|| {
                "Claude Code binary not found. Install it with: brew install claude-code\nOr download from: https://claude.ai/download"
            })?;

        // Check if worktree exists
        if !worktree_path.exists() {
            return Err("Worktree directory does not exist".into());
        }

        // Exec into Claude Code (this replaces the current process)
        let mut cmd = Command::new(&claude_path);
        cmd.current_dir(worktree_path);

        // Set environment variables to help Claude understand context
        cmd.env("BUNSHIN_SESSION", "true");
        cmd.env("BUNSHIN_WORKTREE", worktree_path.display().to_string());

        // Replace current process with Claude Code
        let error = cmd.exec();

        // If we get here, exec failed
        Err(format!("Failed to launch Claude Code: {}", error).into())
    }

    pub fn is_claude_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            use std::process::Command;
            let output = Command::new("ps")
                .args(["-p", &pid.to_string()])
                .output();
            
            if let Ok(output) = output {
                output.status.success()
            } else {
                false
            }
        }
        
        #[cfg(windows)]
        {
            use std::process::Command;
            let output = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid)])
                .output();
            
            if let Ok(output) = output {
                let output_str = String::from_utf8_lossy(&output.stdout);
                output_str.contains(&pid.to_string())
            } else {
                false
            }
        }
    }

    pub fn kill_claude_code(session: &mut Session) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(pid) = session.claude_pid {
            #[cfg(unix)]
            {
                let output = Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .output()?;
                
                if !output.status.success() {
                    return Err(format!("Failed to kill process {}", pid).into());
                }
            }
            
            #[cfg(windows)]
            {
                let output = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .output()?;
                
                if !output.status.success() {
                    return Err(format!("Failed to kill process {}", pid).into());
                }
            }
            
            session.claude_pid = None;
        }
        Ok(())
    }

    pub fn check_and_update_session_status(session: &mut Session) {
        if let Some(pid) = session.claude_pid {
            if !Self::is_claude_running(pid) {
                session.claude_pid = None;
            }
        }
    }
}