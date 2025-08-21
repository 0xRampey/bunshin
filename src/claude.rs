use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use crate::session::Session;

pub struct ClaudeCodeManager;

impl ClaudeCodeManager {
    pub fn launch_claude_code(session: &mut Session) -> Result<(), Box<dyn std::error::Error>> {
        if session.claude_pid.is_some() {
            return Err("Claude Code is already running for this session".into());
        }

        // Use the full path to claude command
        let claude_path = std::env::var("HOME").unwrap_or_default() + "/.claude/local/claude";
        
        // Check if claude binary exists
        if !std::path::Path::new(&claude_path).exists() {
            return Err("Claude binary not found. Make sure Claude Code is installed.".into());
        }

        // Check if worktree exists
        if !session.worktree_path.exists() {
            return Err("Worktree directory does not exist".into());
        }

        let child = Command::new(&claude_path)
            .arg("code")
            .current_dir(&session.worktree_path)
            .spawn()?;

        session.claude_pid = Some(child.id());
        
        Ok(())
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