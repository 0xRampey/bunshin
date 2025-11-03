use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::os::unix::process::CommandExt;

pub struct SessionShell;

impl SessionShell {
    /// Launch a shell directly in the terminal (like tmux session attach)
    /// This will take over the current terminal completely
    pub fn launch_session_shell(worktree_path: &PathBuf, branch_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Restore terminal to normal mode first
        crossterm::terminal::disable_raw_mode()?;
        
        // Change to the worktree directory
        env::set_current_dir(worktree_path)?;
        
        // Set up environment variables
        unsafe {
            env::set_var("BUNSHIN_SESSION_BRANCH", branch_name);
            env::set_var("BUNSHIN_SESSION_PATH", worktree_path.to_string_lossy().to_string());
            
            // Create a custom prompt that shows session info
            let ps1 = format!(
                "\\[\\033[36m\\][bunshin:{}]\\[\\033[0m\\] \\[\\033[32m\\]\\u@\\h\\[\\033[0m\\]:\\[\\033[34m\\]\\w\\[\\033[0m\\]$ ",
                branch_name
            );
            env::set_var("PS1", ps1);
        }
        
        // Get the user's preferred shell
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        
        // Print session info
        println!("🚀 Bunshin Session: {} ({})", branch_name, worktree_path.display());
        println!("📁 Working directory: {}", worktree_path.display());
        println!("🌿 Git branch: {}", branch_name);
        println!("⌨️  Press Ctrl+D or type 'exit' to close session");
        println!("⌨️  Run 'bunshin' to return to session manager");
        println!();
        
        // Execute shell with custom configuration
        let mut cmd = Command::new(&shell);
        
        // For bash, use --rcfile to load custom configuration
        if shell.contains("bash") {
            // Create a temporary rc file that sources the user's bashrc and adds bunshin config
            let temp_rc = create_temp_bashrc(branch_name)?;
            cmd.arg("--rcfile").arg(&temp_rc);
        }
        // For zsh, set up ZDOTDIR
        else if shell.contains("zsh") {
            setup_zsh_config(branch_name)?;
        }
        
        // Replace current process with shell (exec)
        let error = cmd.exec();
        
        // If we get here, exec failed
        Err(format!("Failed to execute shell: {}", error).into())
    }
    
    /// Check if we're currently in a bunshin session
    pub fn in_session() -> bool {
        env::var("BUNSHIN_SESSION_BRANCH").is_ok()
    }
    
    /// Get current session info if in a session
    pub fn current_session_info() -> Option<(String, PathBuf)> {
        if let (Ok(branch), Ok(path)) = (
            env::var("BUNSHIN_SESSION_BRANCH"),
            env::var("BUNSHIN_SESSION_PATH")
        ) {
            Some((branch, PathBuf::from(path)))
        } else {
            None
        }
    }
}

fn create_temp_bashrc(branch_name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::fs;
    use std::io::Write;
    
    let temp_dir = env::temp_dir();
    let temp_rc = temp_dir.join(format!("bunshin-bashrc-{}", branch_name));
    
    let mut rc_content = String::new();
    
    // Source user's existing bashrc if it exists
    if let Ok(home) = env::var("HOME") {
        let bashrc = PathBuf::from(&home).join(".bashrc");
        if bashrc.exists() {
            rc_content.push_str(&format!("source {}\n", bashrc.display()));
        }
        
        let bash_profile = PathBuf::from(&home).join(".bash_profile");
        if bash_profile.exists() {
            rc_content.push_str(&format!("source {}\n", bash_profile.display()));
        }
    }
    
    // Add bunshin-specific configuration
    rc_content.push_str(&format!(r#"
# Bunshin session configuration
export BUNSHIN_SESSION_BRANCH="{}"
export BUNSHIN_SESSION_PATH="{}"

# Custom bunshin command to return to session manager
bunshin() {{
    if [ "$1" = "manager" ] || [ "$1" = "sessions" ] || [ -z "$1" ]; then
        exec bunshin
    else
        command bunshin "$@"
    fi
}}

# Show git status on cd
cd() {{
    builtin cd "$@" && git status --porcelain 2>/dev/null | head -10
}}

echo "Bunshin session active: {}"
echo "Run 'bunshin' to return to session manager"
echo ""
"#, branch_name, env::current_dir()?.display(), branch_name));
    
    let mut file = fs::File::create(&temp_rc)?;
    file.write_all(rc_content.as_bytes())?;
    
    Ok(temp_rc)
}

fn setup_zsh_config(branch_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::io::Write;
    
    let temp_dir = env::temp_dir();
    let zsh_dir = temp_dir.join(format!("bunshin-zsh-{}", branch_name));
    fs::create_dir_all(&zsh_dir)?;
    
    unsafe { env::set_var("ZDOTDIR", &zsh_dir); }
    
    let zshrc_path = zsh_dir.join(".zshrc");
    let mut zshrc_content = String::new();
    
    // Source user's existing zshrc if it exists
    if let Ok(home) = env::var("HOME") {
        let user_zshrc = PathBuf::from(&home).join(".zshrc");
        if user_zshrc.exists() {
            zshrc_content.push_str(&format!("source {}\n", user_zshrc.display()));
        }
    }
    
    // Add bunshin-specific configuration
    zshrc_content.push_str(&format!(r#"
# Bunshin session configuration
export BUNSHIN_SESSION_BRANCH="{}"
export BUNSHIN_SESSION_PATH="{}"

# Custom bunshin command
bunshin() {{
    if [[ "$1" = "manager" || "$1" = "sessions" || -z "$1" ]]; then
        exec bunshin
    else
        command bunshin "$@"
    fi
}}

# Show git status on cd
cd() {{
    builtin cd "$@" && git status --porcelain 2>/dev/null | head -10
}}

echo "Bunshin session active: {}"
echo "Run 'bunshin' to return to session manager"
echo ""
"#, branch_name, env::current_dir()?.display(), branch_name));
    
    let mut file = fs::File::create(&zshrc_path)?;
    file.write_all(zshrc_content.as_bytes())?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_detection() {
        // Should not be in session initially
        assert!(!SessionShell::in_session());
        assert!(SessionShell::current_session_info().is_none());
        
        // Set session environment variables
        unsafe {
            env::set_var("BUNSHIN_SESSION_BRANCH", "test-branch");
            env::set_var("BUNSHIN_SESSION_PATH", "/tmp/test-path");
        }
        
        // Should now detect session
        assert!(SessionShell::in_session());
        let (branch, path) = SessionShell::current_session_info().unwrap();
        assert_eq!(branch, "test-branch");
        assert_eq!(path, PathBuf::from("/tmp/test-path"));
        
        // Clean up
        unsafe {
            env::remove_var("BUNSHIN_SESSION_BRANCH");
            env::remove_var("BUNSHIN_SESSION_PATH");
        }
    }
    
    #[test]
    fn test_temp_bashrc_creation() {
        let temp_rc = create_temp_bashrc("test-branch").unwrap();
        assert!(temp_rc.exists());
        
        let content = std::fs::read_to_string(&temp_rc).unwrap();
        assert!(content.contains("BUNSHIN_SESSION_BRANCH=\"test-branch\""));
        assert!(content.contains("bunshin() {"));
        
        // Clean up
        std::fs::remove_file(&temp_rc).ok();
    }
}