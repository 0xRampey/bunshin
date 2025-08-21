use bunshin::git::GitWorktree;
use bunshin::session::{Session, SessionManager};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Test the exact scenario that's failing
    let repo_path = PathBuf::from("/tmp/bunshin-test/test-repo");
    let session_name = "test-session";
    let branch_name = "feature-test";
    
    println!("=== Testing Worktree Creation ===");
    println!("Repo path: {:?}", repo_path);
    println!("Session name: {}", session_name);
    println!("Branch name: {}", branch_name);
    
    // Check if repo exists and is valid
    if !repo_path.exists() {
        eprintln!("Repository path does not exist! Run the test setup script first.");
        return Ok(());
    }
    
    if !GitWorktree::is_git_repo(&repo_path) {
        eprintln!("Path is not a Git repository!");
        return Ok(());
    }
    
    // Create worktree path like the main app does
    let worktree_base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".bunshin")
        .join("worktrees");
    
    std::fs::create_dir_all(&worktree_base)?;
    
    let worktree_path = worktree_base.join(format!("{}-{}", session_name, branch_name));
    
    println!("Worktree path: {:?}", worktree_path);
    
    // Try to create the worktree
    println!("\n=== Attempting Worktree Creation ===");
    match GitWorktree::create_worktree(&repo_path, &worktree_path, branch_name) {
        Ok(()) => {
            println!("✅ SUCCESS: Worktree created successfully!");
            println!("Worktree location: {:?}", worktree_path);
            
            // Verify it exists
            if worktree_path.exists() {
                println!("✅ Worktree directory exists");
            } else {
                println!("❌ Worktree directory does not exist");
            }
            
            // Test session creation
            println!("\n=== Testing Session Creation ===");
            let mut session_manager = SessionManager::new();
            let session = Session::new(
                session_name.to_string(),
                worktree_path.clone(),
                branch_name.to_string(),
                repo_path.clone()
            );
            
            session_manager.add_session(session);
            println!("✅ Session created successfully");
            
        }
        Err(e) => {
            println!("❌ FAILED: {}", e);
            eprintln!("Error details: {}", e);
            
            // Let's try some debugging
            println!("\n=== Debug Information ===");
            
            // Check git status in repo
            let output = std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&repo_path)
                .output()?;
            println!("Git status output: {}", String::from_utf8_lossy(&output.stdout));
            
            // Check existing branches
            let output = std::process::Command::new("git")
                .args(["branch", "-a"])
                .current_dir(&repo_path)
                .output()?;
            println!("Git branches: {}", String::from_utf8_lossy(&output.stdout));
            
            // Check existing worktrees
            let output = std::process::Command::new("git")
                .args(["worktree", "list"])
                .current_dir(&repo_path)
                .output()?;
            println!("Existing worktrees: {}", String::from_utf8_lossy(&output.stdout));
        }
    }
    
    Ok(())
}