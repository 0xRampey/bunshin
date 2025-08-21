use bunshin::session_shell::SessionShell;
use std::path::PathBuf;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Tmux-like Workflow ===");
    
    // Check if we have a test worktree
    let worktree_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".bunshin")
        .join("worktrees")
        .join("test-session-feature-test");

    if !worktree_path.exists() {
        println!("❌ Test worktree not found at: {:?}", worktree_path);
        println!("Run this first: cargo run --bin test_worktree");
        return Ok(());
    }

    println!("✅ Found test worktree at: {:?}", worktree_path);
    
    // Test session detection
    println!("\n=== Session Detection Test ===");
    if SessionShell::in_session() {
        if let Some((branch, path)) = SessionShell::current_session_info() {
            println!("✅ Already in session: {} at {}", branch, path.display());
        }
    } else {
        println!("✅ Not currently in a session");
    }
    
    println!("\n=== How the Tmux-like Workflow Works ===");
    println!("1. Run 'bunshin' - Opens TUI session manager");
    println!("2. Press Enter on a session - TUI disappears, drops into shell");
    println!("3. In session shell - Run 'bunshin' to return to manager");
    println!("4. Alternative - Run 'bunshin attach <session-name>' to attach directly");
    
    println!("\n=== Test Commands ===");
    println!("Try these commands:");
    println!("  cargo run                           # Open session manager TUI");
    println!("  cargo run -- attach test-session   # Attach directly to session");
    println!("  cargo run -- --help                # Show help");
    
    println!("\n=== Expected Behavior ===");
    println!("• TUI shows sessions with indicators:");
    println!("  - ● = Claude Code running (green/gray)");
    println!("  - No more shell indicators (shells are session-based now)");
    println!("• Press Enter = TUI disappears, you're in the session shell");
    println!("• Press 'c' = Launch Claude Code for session");  
    println!("• Press 'n' = Create new session");
    println!("• Press 'q' = Quit to current shell");
    
    println!("\n=== Session Shell Features ===");
    println!("When attached to a session:");
    println!("• Custom prompt shows session name");
    println!("• Working directory is the worktree");
    println!("• Environment variables set (BUNSHIN_SESSION_BRANCH, etc.)");
    println!("• 'bunshin' command returns to manager");
    println!("• 'exit' or Ctrl+D closes session");
    
    Ok(())
}