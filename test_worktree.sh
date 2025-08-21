#!/bin/bash

# Create a test repository to verify our worktree functionality
TEST_DIR="/tmp/bunshin-test"
TEST_REPO="$TEST_DIR/test-repo"

echo "Setting up test repository..."
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

# Initialize test repo
git init test-repo
cd test-repo
git config user.email "test@example.com"
git config user.name "Test User"

# Create initial commit
echo "# Test Repository" > README.md
git add README.md
git commit -m "Initial commit"

echo "Test repository created at: $TEST_REPO"
echo ""
echo "Now you can test the bunshin application with this repository:"
echo "1. Run: cargo run"
echo "2. Press 'n' to create new session"
echo "3. Use these values:"
echo "   - Session Name: test-session"
echo "   - Repository Path: $TEST_REPO"
echo "   - Branch: feature-test"
echo ""
echo "The worktree should be created at: ~/.bunshin/worktrees/test-session-feature-test"