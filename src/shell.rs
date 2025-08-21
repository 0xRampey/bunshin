use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSession {
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub pid: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl ShellSession {
    pub fn new(branch_name: String, worktree_path: PathBuf, pid: u32) -> Self {
        Self {
            branch_name,
            worktree_path,
            pid,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn is_running(&self) -> bool {
        ShellManager::is_process_running(self.pid)
    }
}

#[derive(Debug, Default)]
pub struct ShellManager {
    pub shells: HashMap<String, ShellSession>,
}

impl ShellManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_shell(&mut self, branch_name: &str, worktree_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        // Close existing shell for this branch if it exists
        let _ = self.close_shell(branch_name);

        // Determine terminal application and command
        let (terminal_app, args) = Self::get_terminal_command();
        
        // Create the shell command that will run in the terminal
        let shell_cmd = format!(
            "cd '{}' && echo 'Opened shell in worktree: {}' && echo 'Branch: {}' && exec $SHELL",
            worktree_path.display(),
            worktree_path.display(),
            branch_name
        );

        // Launch the terminal with the shell command
        let mut cmd = Command::new(&terminal_app);
        for arg in args {
            if arg.contains("SHELL_CMD") {
                cmd.arg(arg.replace("SHELL_CMD", &shell_cmd));
            } else {
                cmd.arg(arg);
            }
        }

        let child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let shell_session = ShellSession::new(
            branch_name.to_string(),
            worktree_path.clone(),
            child.id()
        );

        self.shells.insert(branch_name.to_string(), shell_session);

        Ok(())
    }

    pub fn close_shell(&mut self, branch_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(shell) = self.shells.remove(branch_name) {
            Self::kill_process(shell.pid)?;
        }
        Ok(())
    }

    pub fn get_running_shells(&self) -> Vec<&ShellSession> {
        self.shells.values()
            .filter(|shell| shell.is_running())
            .collect()
    }

    pub fn cleanup_dead_shells(&mut self) {
        let dead_shells: Vec<String> = self.shells
            .iter()
            .filter(|(_, shell)| !shell.is_running())
            .map(|(name, _)| name.clone())
            .collect();

        for shell_name in dead_shells {
            self.shells.remove(&shell_name);
        }
    }

    fn get_terminal_command() -> (String, Vec<String>) {
        #[cfg(target_os = "macos")]
        {
            // Try different terminal applications in order of preference
            if Self::command_exists("wezterm") {
                return ("wezterm".to_string(), vec!["start".to_string(), "--".to_string(), "sh".to_string(), "-c".to_string(), "SHELL_CMD".to_string()]);
            }
            if Self::command_exists("alacritty") {
                return ("alacritty".to_string(), vec!["-e".to_string(), "sh".to_string(), "-c".to_string(), "SHELL_CMD".to_string()]);
            }
            if Self::command_exists("kitty") {
                return ("kitty".to_string(), vec!["sh".to_string(), "-c".to_string(), "SHELL_CMD".to_string()]);
            }
            // Fallback to macOS Terminal
            return ("osascript".to_string(), vec![
                "-e".to_string(),
                format!("tell application \"Terminal\" to do script \"SHELL_CMD\"")
            ]);
        }

        #[cfg(target_os = "linux")]
        {
            // Try different terminal applications
            if Self::command_exists("gnome-terminal") {
                return ("gnome-terminal".to_string(), vec!["--".to_string(), "sh".to_string(), "-c".to_string(), "SHELL_CMD".to_string()]);
            }
            if Self::command_exists("konsole") {
                return ("konsole".to_string(), vec!["-e".to_string(), "sh".to_string(), "-c".to_string(), "SHELL_CMD".to_string()]);
            }
            if Self::command_exists("xterm") {
                return ("xterm".to_string(), vec!["-e".to_string(), "sh".to_string(), "-c".to_string(), "SHELL_CMD".to_string()]);
            }
            // Fallback
            return ("x-terminal-emulator".to_string(), vec!["-e".to_string(), "sh".to_string(), "-c".to_string(), "SHELL_CMD".to_string()]);
        }

        #[cfg(target_os = "windows")]
        {
            return ("cmd".to_string(), vec!["/c".to_string(), "start".to_string(), "cmd".to_string(), "/k".to_string(), "SHELL_CMD".to_string()]);
        }
    }

    fn command_exists(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    pub fn is_process_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            Command::new("kill")
                .args(["-0", &pid.to_string()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }

        #[cfg(windows)]
        {
            Command::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid)])
                .output()
                .map(|output| {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    output_str.contains(&pid.to_string())
                })
                .unwrap_or(false)
        }
    }

    fn kill_process(pid: u32) -> Result<(), Box<dyn std::error::Error>> {
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

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_shell_session_creation() {
        let temp_dir = TempDir::new().unwrap();
        let worktree_path = temp_dir.path().to_path_buf();
        
        let shell = ShellSession::new("test-branch".to_string(), worktree_path.clone(), 12345);
        
        assert_eq!(shell.branch_name, "test-branch");
        assert_eq!(shell.worktree_path, worktree_path);
        assert_eq!(shell.pid, 12345);
    }

    #[test]
    fn test_shell_manager_operations() {
        let mut manager = ShellManager::new();
        assert_eq!(manager.shells.len(), 0);
        
        // Add a mock shell session
        let temp_dir = TempDir::new().unwrap();
        let shell = ShellSession::new("test-branch".to_string(), temp_dir.path().to_path_buf(), 99999);
        manager.shells.insert("test-branch".to_string(), shell);
        
        assert_eq!(manager.shells.len(), 1);
        assert!(manager.shells.contains_key("test-branch"));
        
        // Test cleanup (this will remove the shell since PID 99999 doesn't exist)
        manager.cleanup_dead_shells();
        assert_eq!(manager.shells.len(), 0);
    }

    #[test]
    fn test_command_exists() {
        // Test with a command that should exist on most systems
        assert!(ShellManager::command_exists("sh"));
        
        // Test with a command that shouldn't exist
        assert!(!ShellManager::command_exists("definitely_not_a_real_command_12345"));
    }
}