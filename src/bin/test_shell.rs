use bunshin::git::GitWorktree;
use bunshin::shell::ShellManager;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Test shell functionality with the existing test worktree
    let worktree_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".bunshin")
        .join("worktrees")
        .join("test-session-feature-test");

    println!("=== Testing Shell Functionality ===");
    println!("Worktree path: {:?}", worktree_path);

    if !worktree_path.exists() {
        println!("❌ Worktree does not exist. Create a session first using the main application.");
        println!("Run: cargo run --bin test_worktree");
        return Ok(());
    }

    println!("✅ Worktree exists");

    // Test shell manager
    let mut shell_manager = ShellManager::new();
    println!("\n=== Opening Shell ===");

    match shell_manager.open_shell("feature-test", &worktree_path) {
        Ok(()) => {
            println!("✅ Shell opened successfully!");
            println!("Branch name: feature-test");
            println!("Working directory: {:?}", worktree_path);
            
            // List running shells
            let running_shells = shell_manager.get_running_shells();
            println!("\n=== Running Shells ===");
            for shell in running_shells {
                println!("- Branch: {} (PID: {}, Created: {})", 
                    shell.branch_name, 
                    shell.pid, 
                    shell.created_at.format("%Y-%m-%d %H:%M:%S")
                );
            }
            
            println!("\n=== Instructions ===");
            println!("A terminal window should have opened in the worktree directory.");
            println!("The shell should be in the '{}' directory.", worktree_path.display());
            println!("You can verify by running 'pwd' and 'git branch' in the new terminal.");
            println!("Press Enter to continue and clean up...");
            
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            
            // Cleanup
            println!("\n=== Cleanup ===");
            shell_manager.cleanup_dead_shells();
            println!("✅ Cleaned up dead shells");
        }
        Err(e) => {
            println!("❌ Failed to open shell: {}", e);
            println!("\nThis might be because:");
            println!("- No suitable terminal application found");
            println!("- Permission issues");
            println!("- The worktree directory is not accessible");
        }
    }

    Ok(())
}