use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct GitWorktree;

impl GitWorktree {
    pub fn list_worktrees(repo_path: &PathBuf) -> Result<Vec<(String, PathBuf)>, Box<dyn std::error::Error>> {
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(repo_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            return Err(format!("Git command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }

        let mut worktrees = Vec::new();
        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut current_path = None;
        let mut current_branch = None;

        for line in output_str.lines() {
            if line.starts_with("worktree ") {
                current_path = Some(PathBuf::from(&line[9..]));
            } else if line.starts_with("branch ") {
                current_branch = Some(line[7..].to_string());
            } else if line.is_empty() {
                if let (Some(path), Some(branch)) = (current_path.take(), current_branch.take()) {
                    worktrees.push((branch, path));
                }
            }
        }

        if let (Some(path), Some(branch)) = (current_path, current_branch) {
            worktrees.push((branch, path));
        }

        Ok(worktrees)
    }

    pub fn create_worktree(
        repo_path: &PathBuf,
        worktree_path: &PathBuf,
        branch: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Validate inputs
        if !repo_path.exists() {
            return Err(format!("Repository path does not exist: {:?}", repo_path).into());
        }
        
        if !Self::is_git_repo(repo_path) {
            return Err(format!("Path is not a Git repository: {:?}", repo_path).into());
        }
        
        if worktree_path.exists() {
            // Try to remove the existing directory if it's empty or just clean it up
            if worktree_path.is_dir() {
                std::fs::remove_dir_all(&worktree_path).map_err(|e| {
                    format!("Worktree path exists and cannot be removed: {:?} ({})", worktree_path, e)
                })?;
            } else {
                return Err(format!("Worktree path exists as a file: {:?}", worktree_path).into());
            }
        }
        
        // Make sure parent directory exists for worktree
        if let Some(parent) = worktree_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create parent directory for worktree: {}", e)
                })?;
            }
        }
        
        // Check if branch exists locally
        let branch_exists_locally = Self::branch_exists_locally(repo_path, branch)?;
        
        // Check if branch exists on remote
        let branch_exists_on_remote = Self::branch_exists_on_remote(repo_path, branch)?;
        
        let remote_branch = format!("origin/{}", branch);
        let worktree_path_str = worktree_path.to_str().unwrap();
        
        let worktree_args = if branch_exists_locally {
            // Branch exists locally, use it directly
            vec!["worktree", "add", worktree_path_str, branch]
        } else if branch_exists_on_remote {
            // Branch exists on remote, create local tracking branch
            vec!["worktree", "add", "--track", "-b", branch, worktree_path_str, &remote_branch]
        } else {
            // Branch doesn't exist anywhere, create new branch from current HEAD
            vec!["worktree", "add", "-b", branch, worktree_path_str]
        };

        let output = Command::new("git")
            .args(&worktree_args)
            .current_dir(repo_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            // Handle common Git worktree errors
            if stderr.contains("already exists") || stderr.contains("already checked out") {
                return Err(format!(
                    "Branch '{}' is already checked out in another worktree. Use a different branch name or remove the existing worktree.",
                    branch
                ).into());
            }
            
            if stderr.contains("not a valid object") {
                return Err(format!(
                    "Branch '{}' does not exist on remote 'origin'. The branch will be created as a new branch.",
                    branch
                ).into());
            }
            
            if stderr.contains("refusing to create") {
                return Err(format!(
                    "Git refused to create worktree. The target directory may already exist or be in use."
                ).into());
            }
            
            return Err(format!(
                "Git worktree creation failed.\nCommand: git {}\nExit code: {:?}\nError: {}",
                worktree_args.join(" "), output.status.code(), stderr.trim()
            ).into());
        }

        Ok(())
    }

    fn branch_exists_locally(repo_path: &PathBuf, branch: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let output = Command::new("git")
            .args(["show-ref", "--verify", "--quiet", &format!("refs/heads/{}", branch)])
            .current_dir(repo_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        Ok(output.status.success())
    }

    fn branch_exists_on_remote(repo_path: &PathBuf, branch: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let output = Command::new("git")
            .args(["show-ref", "--verify", "--quiet", &format!("refs/remotes/origin/{}", branch)])
            .current_dir(repo_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        Ok(output.status.success())
    }

    pub fn remove_worktree(
        repo_path: &PathBuf,
        worktree_path: &PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output = Command::new("git")
            .args(["worktree", "remove", worktree_path.to_str().unwrap()])
            .current_dir(repo_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "Failed to remove worktree: {}",
                String::from_utf8_lossy(&output.stderr)
            ).into());
        }

        Ok(())
    }

    pub fn list_branches(repo_path: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let output = Command::new("git")
            .args(["branch", "-a", "--format=%(refname:short)"])
            .current_dir(repo_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            return Err(format!("Git command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }

        let branches = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|branch| !branch.is_empty())
            .collect();

        Ok(branches)
    }

    pub fn is_git_repo(path: &PathBuf) -> bool {
        let git_dir = path.join(".git");
        git_dir.exists()
    }

    // Test helper functions
    pub fn init_test_repo(repo_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(repo_path)?;
        
        let output = Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()?;
        
        if !output.status.success() {
            return Err("Failed to init git repo".into());
        }

        // Configure git for testing
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()?;
        
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()?;

        // Create initial commit
        std::fs::write(repo_path.join("README.md"), "# Test Repo")?;
        
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(repo_path)
            .output()?;
        
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()?;

        Ok(())
    }

    pub fn create_test_branch(repo_path: &PathBuf, branch_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let output = Command::new("git")
            .args(["checkout", "-b", branch_name])
            .current_dir(repo_path)
            .output()?;
        
        if !output.status.success() {
            return Err("Failed to create test branch".into());
        }

        // Switch back to main
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(repo_path)
            .output()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_repo() -> Result<(TempDir, PathBuf), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path().to_path_buf();
        GitWorktree::init_test_repo(&repo_path)?;
        Ok((temp_dir, repo_path))
    }

    #[test]
    fn test_is_git_repo() {
        let (_temp_dir, repo_path) = setup_test_repo().unwrap();
        assert!(GitWorktree::is_git_repo(&repo_path));
        
        let non_repo = PathBuf::from("/tmp/not-a-repo");
        assert!(!GitWorktree::is_git_repo(&non_repo));
    }

    #[test]
    fn test_branch_exists_locally() {
        let (_temp_dir, repo_path) = setup_test_repo().unwrap();
        
        // main branch should exist (created during init)
        assert!(GitWorktree::branch_exists_locally(&repo_path, "main").unwrap());
        
        // non-existent branch
        assert!(!GitWorktree::branch_exists_locally(&repo_path, "nonexistent").unwrap());
        
        // Create a test branch
        GitWorktree::create_test_branch(&repo_path, "test-branch").unwrap();
        assert!(GitWorktree::branch_exists_locally(&repo_path, "test-branch").unwrap());
    }

    #[test]
    fn test_create_worktree_new_branch() {
        let (_temp_dir, repo_path) = setup_test_repo().unwrap();
        let worktree_temp = TempDir::new().unwrap();
        let worktree_path = worktree_temp.path().join("new-feature-worktree");
        
        // Create worktree with new branch
        let result = GitWorktree::create_worktree(&repo_path, &worktree_path, "new-feature");
        assert!(result.is_ok(), "Failed to create worktree: {:?}", result.err());
        
        // Verify worktree directory exists
        assert!(worktree_path.exists());
        
        // Verify the branch was created locally
        assert!(GitWorktree::branch_exists_locally(&repo_path, "new-feature").unwrap());
    }

    #[test]
    fn test_create_worktree_existing_branch() {
        let (_temp_dir, repo_path) = setup_test_repo().unwrap();
        let worktree_temp = TempDir::new().unwrap();
        let worktree_path = worktree_temp.path().join("existing-branch-worktree");
        
        // Create a test branch first
        GitWorktree::create_test_branch(&repo_path, "existing-test-branch").unwrap();
        
        // Create worktree with the existing branch (not main, since main is already checked out)
        let result = GitWorktree::create_worktree(&repo_path, &worktree_path, "existing-test-branch");
        assert!(result.is_ok(), "Failed to create worktree: {:?}", result.err());
        
        // Verify worktree directory exists
        assert!(worktree_path.exists());
    }

    #[test]
    fn test_create_worktree_invalid_repo() {
        let invalid_repo = PathBuf::from("/tmp/invalid-repo");
        let worktree_temp = TempDir::new().unwrap();
        let worktree_path = worktree_temp.path().join("test-worktree");
        
        let result = GitWorktree::create_worktree(&invalid_repo, &worktree_path, "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Repository path does not exist"));
    }

    #[test]
    fn test_create_worktree_existing_path() {
        let (_temp_dir, repo_path) = setup_test_repo().unwrap();
        let worktree_temp = TempDir::new().unwrap();
        let worktree_path = worktree_temp.path().join("existing-dir");
        
        // Create the directory first
        std::fs::create_dir(&worktree_path).unwrap();
        
        // Should now succeed because we auto-cleanup existing directories
        let result = GitWorktree::create_worktree(&repo_path, &worktree_path, "test-existing");
        assert!(result.is_ok(), "Should succeed after cleaning up existing directory: {:?}", result.err());
        
        // Verify the worktree was created
        assert!(worktree_path.exists());
    }

    #[test]
    fn test_remove_worktree() {
        let (_temp_dir, repo_path) = setup_test_repo().unwrap();
        let worktree_temp = TempDir::new().unwrap();
        let worktree_path = worktree_temp.path().join("remove-test-worktree");
        
        // Create worktree
        GitWorktree::create_worktree(&repo_path, &worktree_path, "remove-test").unwrap();
        assert!(worktree_path.exists());
        
        // Remove worktree
        let result = GitWorktree::remove_worktree(&repo_path, &worktree_path);
        assert!(result.is_ok(), "Failed to remove worktree: {:?}", result.err());
        
        // Verify worktree directory is removed
        assert!(!worktree_path.exists());
    }

    #[test]
    fn test_list_branches() {
        let (_temp_dir, repo_path) = setup_test_repo().unwrap();
        
        // Create a few test branches
        GitWorktree::create_test_branch(&repo_path, "feature-1").unwrap();
        GitWorktree::create_test_branch(&repo_path, "feature-2").unwrap();
        
        let branches = GitWorktree::list_branches(&repo_path).unwrap();
        
        // Should contain at least main and our test branches
        assert!(branches.contains(&"main".to_string()));
        assert!(branches.contains(&"feature-1".to_string()));
        assert!(branches.contains(&"feature-2".to_string()));
    }

    #[test]
    fn test_list_worktrees() {
        let (_temp_dir, repo_path) = setup_test_repo().unwrap();
        let worktree_temp = TempDir::new().unwrap();
        let worktree_path = worktree_temp.path().join("list-test-worktree");
        
        // Create a worktree
        GitWorktree::create_worktree(&repo_path, &worktree_path, "list-test").unwrap();
        
        let worktrees = GitWorktree::list_worktrees(&repo_path).unwrap();
        
        // Should contain at least the main worktree and our test worktree
        assert!(worktrees.len() >= 2);
        
        // Check if our test worktree is in the list
        // Note: branch names in worktree list might have refs/heads/ prefix
        let test_worktree_found = worktrees.iter()
            .any(|(branch, path)| {
                (branch == "list-test" || branch == "refs/heads/list-test") && 
                path.file_name() == worktree_path.file_name()
            });
        
        if !test_worktree_found {
            eprintln!("Expected worktree path: {:?}", worktree_path);
            eprintln!("Actual worktrees: {:?}", worktrees);
        }
        
        assert!(test_worktree_found, "Test worktree not found in list: {:?}", worktrees);
    }
}