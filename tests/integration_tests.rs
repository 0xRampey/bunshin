use bunshin::session::{Session, SessionManager};
use bunshin::git::GitWorktree;
use tempfile::TempDir;
use std::path::PathBuf;

fn setup_test_repo() -> Result<(TempDir, PathBuf), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path().to_path_buf();
    GitWorktree::init_test_repo(&repo_path)?;
    Ok((temp_dir, repo_path))
}

#[test]
fn test_full_session_workflow() {
    let (_temp_dir, repo_path) = setup_test_repo().unwrap();
    let session_temp = TempDir::new().unwrap();
    let sessions_file = session_temp.path().join("sessions.json");
    
    // Create session manager
    let mut session_manager = SessionManager::new();
    
    // Create a session with a new branch
    let worktree_path = session_temp.path().join("test-repo-feature");
    let session = Session::new(
        "feature-session".to_string(),
        worktree_path.clone(),
        "feature-branch".to_string(),
        repo_path.clone()
    );
    
    session_manager.add_session(session);
    
    // Save sessions
    assert!(session_manager.save_to_file(&sessions_file).is_ok());
    
    // Load sessions from file
    let loaded_manager = SessionManager::load_from_file(&sessions_file).unwrap();
    assert_eq!(loaded_manager.sessions.len(), 1);
    assert_eq!(loaded_manager.sessions[0].name, "feature-session");
    assert_eq!(loaded_manager.sessions[0].branch, "feature-branch");
    
    // Create the actual worktree
    let session = &loaded_manager.sessions[0];
    let result = GitWorktree::create_worktree(&session.repo_path, &session.worktree_path, &session.branch);
    assert!(result.is_ok(), "Failed to create worktree: {:?}", result.err());
    
    // Verify worktree exists
    assert!(session.worktree_path.exists());
    
    // Clean up worktree
    let result = GitWorktree::remove_worktree(&session.repo_path, &session.worktree_path);
    assert!(result.is_ok(), "Failed to remove worktree: {:?}", result.err());
}

#[test]
fn test_session_manager_operations() {
    let mut manager = SessionManager::new();
    
    let session1 = Session::new(
        "session1".to_string(),
        PathBuf::from("/tmp/session1"),
        "main".to_string(),
        PathBuf::from("/tmp/repo")
    );
    
    let session2 = Session::new(
        "session2".to_string(),
        PathBuf::from("/tmp/session2"),
        "develop".to_string(),
        PathBuf::from("/tmp/repo")
    );
    
    // Add sessions
    manager.add_session(session1);
    manager.add_session(session2);
    assert_eq!(manager.sessions.len(), 2);
    
    // Get session
    let session = manager.get_session("session1");
    assert!(session.is_some());
    assert_eq!(session.unwrap().name, "session1");
    
    // Get non-existent session
    assert!(manager.get_session("nonexistent").is_none());
    
    // Remove session
    manager.remove_session("session1");
    assert_eq!(manager.sessions.len(), 1);
    assert!(manager.get_session("session1").is_none());
    assert!(manager.get_session("session2").is_some());
}

#[test]
fn test_worktree_creation_scenarios() {
    let (_temp_dir, repo_path) = setup_test_repo().unwrap();
    
    // Test 1: Create worktree with new branch
    let worktree_temp1 = TempDir::new().unwrap();
    let worktree_path1 = worktree_temp1.path().join("new-branch-worktree");
    
    let result = GitWorktree::create_worktree(&repo_path, &worktree_path1, "new-feature");
    assert!(result.is_ok(), "Failed to create worktree with new branch: {:?}", result.err());
    assert!(worktree_path1.exists());
    
    // Test 2: Create worktree with existing branch
    GitWorktree::create_test_branch(&repo_path, "existing-feature").unwrap();
    
    let worktree_temp2 = TempDir::new().unwrap();
    let worktree_path2 = worktree_temp2.path().join("existing-branch-worktree");
    
    let result = GitWorktree::create_worktree(&repo_path, &worktree_path2, "existing-feature");
    assert!(result.is_ok(), "Failed to create worktree with existing branch: {:?}", result.err());
    assert!(worktree_path2.exists());
    
    // Clean up
    GitWorktree::remove_worktree(&repo_path, &worktree_path1).unwrap();
    GitWorktree::remove_worktree(&repo_path, &worktree_path2).unwrap();
}

#[test]
fn test_session_persistence() {
    let session_temp = TempDir::new().unwrap();
    let sessions_file = session_temp.path().join("test_sessions.json");
    
    // Create and save sessions
    {
        let mut manager = SessionManager::new();
        
        let session = Session::new(
            "persistent-session".to_string(),
            PathBuf::from("/tmp/persistent-worktree"),
            "persistent-branch".to_string(),
            PathBuf::from("/tmp/persistent-repo")
        );
        
        manager.add_session(session);
        manager.save_to_file(&sessions_file).unwrap();
    }
    
    // Load and verify sessions
    {
        let manager = SessionManager::load_from_file(&sessions_file).unwrap();
        assert_eq!(manager.sessions.len(), 1);
        
        let session = &manager.sessions[0];
        assert_eq!(session.name, "persistent-session");
        assert_eq!(session.branch, "persistent-branch");
        assert_eq!(session.worktree_path, PathBuf::from("/tmp/persistent-worktree"));
        assert_eq!(session.repo_path, PathBuf::from("/tmp/persistent-repo"));
    }
}

#[test]
fn test_error_conditions() {
    let (_temp_dir, repo_path) = setup_test_repo().unwrap();
    
    // Test: Try to create worktree with existing directory
    let worktree_temp = TempDir::new().unwrap();
    let existing_dir = worktree_temp.path().join("existing-directory");
    std::fs::create_dir(&existing_dir).unwrap();
    
    let result = GitWorktree::create_worktree(&repo_path, &existing_dir, "test-branch");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Worktree path already exists"));
    
    // Test: Try to create worktree with invalid repo
    let invalid_repo = PathBuf::from("/tmp/nonexistent-repo");
    let worktree_path = worktree_temp.path().join("test-worktree");
    
    let result = GitWorktree::create_worktree(&invalid_repo, &worktree_path, "test-branch");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Repository path does not exist"));
}