use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Let's test worktree creation manually to see what's failing
    let repo_path = std::env::args().nth(1)
        .ok_or("Usage: debug_worktree <repo_path> [branch_name]")?;
    let branch_name = std::env::args().nth(2).unwrap_or_else(|| "test-branch".to_string());
    
    let repo_path = PathBuf::from(repo_path);
    let worktree_path = repo_path.parent()
        .unwrap_or(&repo_path)
        .join(format!("{}-{}", 
            repo_path.file_name().unwrap().to_string_lossy(),
            branch_name
        ));
    
    println!("Repo path: {:?}", repo_path);
    println!("Worktree path: {:?}", worktree_path);
    println!("Branch: {}", branch_name);
    
    // Check if it's a Git repo
    if !repo_path.join(".git").exists() {
        return Err("Not a Git repository".into());
    }
    
    // Check current directory and branches
    println!("\n=== Git Status ===");
    let output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(&repo_path)
        .output()?;
    println!("Git status: {}", String::from_utf8_lossy(&output.stdout));
    
    println!("\n=== Local Branches ===");
    let output = Command::new("git")
        .args(["branch"])
        .current_dir(&repo_path)
        .output()?;
    println!("Local branches: {}", String::from_utf8_lossy(&output.stdout));
    
    println!("\n=== Checking Branch Existence ===");
    let branch_check = Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &format!("refs/heads/{}", branch_name)])
        .current_dir(&repo_path)
        .output()?;
    println!("Branch '{}' exists locally: {}", branch_name, branch_check.status.success());
    
    // Try to create worktree
    println!("\n=== Creating Worktree ===");
    let worktree_args = if branch_check.status.success() {
        vec!["worktree", "add", worktree_path.to_str().unwrap(), &branch_name]
    } else {
        vec!["worktree", "add", "-b", &branch_name, worktree_path.to_str().unwrap()]
    };
    
    println!("Git command: git {:?}", worktree_args);
    
    let output = Command::new("git")
        .args(&worktree_args)
        .current_dir(&repo_path)
        .output()?;
    
    println!("Exit code: {:?}", output.status.code());
    println!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
    
    if !output.status.success() {
        return Err(format!("Git worktree command failed").into());
    }
    
    println!("\n=== Success! ===");
    println!("Worktree created at: {:?}", worktree_path);
    
    Ok(())
}