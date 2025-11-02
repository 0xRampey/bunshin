# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Bunshin is a multi-agent orchestration system that combines Git worktree management with AI agent lifecycle management. The name "bunshin" (分身) means "clone" or "doppelganger" in Japanese, reflecting the system's ability to spawn isolated agent instances.

**Core Concept**: Bunshin allows spawning multiple AI agents (Claude Code, GPT-4, etc.) in isolated Git worktrees, enabling parallel development across different branches with independent working directories.

## Architecture

### Hierarchical Organization

The system uses a tmux-inspired hierarchy:
- **Sessions** (`BunshinSession`): Top-level containers with unique IDs (e.g., `s-a1b2c3d4`)
- **Windows** (`Window`): Logical groupings within sessions (e.g., `w-e5f6g7h8`)
- **Agents** (`Agent`): Individual AI processes with isolated worktrees (e.g., `a-i9j0`)

### Key Modules

**Core Data Models** (`src/core.rs`):
- `BunshinSession`, `Window`, `Agent`: Hierarchical data structures
- `AgentModel`: Enum for different AI models (ClaudeCode, GPT-4o, etc.)
- `AgentState`: Lifecycle states (Starting, Running, Idle, Stopped, Error)
- `Project`: Project metadata for organizing work

**Manager** (`src/manager.rs`):
- `BunshinManager`: Central orchestrator for sessions, windows, and agents
- Persists state to `~/.bunshin/manager.json`
- Handles agent spawning, worktree creation, and fleet operations

**Process Management** (`src/process.rs`):
- `ProcessManager`: Manages actual OS processes for agents
- Handles stdin/stdout/stderr via channels
- Logs all agent output to `~/.bunshin/logs/{agent_id}.log`
- Supports interactive shells and command broadcasting

**Git Integration** (`src/git.rs`):
- `GitWorktree`: Wrapper around `git worktree` commands
- Creates isolated worktrees in `~/.bunshin/worktrees/`
- Handles branch creation, remote tracking, and cleanup
- Branch naming: `agent-{agent_id}-{timestamp}` for agent-specific branches

**UI** (`src/main.rs`, `src/ui.rs`):
- TUI session manager using ratatui/crossterm
- CLI commands using clap parser

## Building and Testing

### Build Commands
```bash
# Build the project
cargo build

# Build release version
cargo build --release

# Run the application
cargo run

# Run with specific command
cargo run -- ls
cargo run -- spawn --model claude-code --count 2
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_create_worktree_new_branch

# Run integration tests
cargo test --test integration_tests

# Run with output
cargo test -- --nocapture
```

### Helper Scripts
- `test_worktree.sh`: Creates a test Git repository at `/tmp/bunshin-test/test-repo`
- `debug_worktree`: Debug binary for testing worktree creation
- `bunshin_agent.py`: Python mock agent for testing without real AI APIs

## CLI Commands

### Quick Start
```bash
# When run in a git repository, auto-creates a session with Claude Code
bunshin                    # Auto-init from pwd (detects git repo, requires Claude Code)
bunshin init               # Explicitly init from pwd
bunshin init --branch my-feature  # Init with specific branch name
bunshin init --name my-session    # Init with custom session name
```

**IMPORTANT**: Claude Code is REQUIRED. Bunshin will not create sessions without it.

### Session Management
```bash
bunshin manager            # Force TUI session manager
bunshin ls                 # List all sessions
bunshin new session --name my-session --repo /path/to/repo --branch feature-x
```

### Agent Spawning
```bash
# Spawn agents in current window
bunshin spawn --model claude-code --count 3 --project my-project

# Spawn with task description
bunshin spawn --model gpt-4o --task "Fix the authentication bug" --labels bug,auth
```

### Agent Interaction
```bash
bunshin ps                         # List agents in current session
bunshin ps --all                   # List all agents
bunshin shell {agent-id}          # Connect to agent's interactive shell
bunshin worktree {agent-id}       # Shell into agent's worktree directory
bunshin logs {agent-id}           # Tail agent logs
bunshin logs {agent-id} --follow  # Follow log output
```

### Fleet Operations
```bash
# Broadcast to all agents in a project
bunshin broadcast --project my-project "status"

# Broadcast to agents with labels
bunshin broadcast --labels bug,high-priority "analyze the error logs"

# Kill agents
bunshin kill {agent-id}
bunshin kill --all --force
```

### Project Management
```bash
bunshin new project --name my-project --description "Web app refactor"
bunshin project list
bunshin project show my-project
```

## Important Patterns

### Agent Model Support
The system supports multiple agent models via the `AgentModel` enum:
- `claude-code`: Uses actual Claude Code CLI if available, falls back to Python wrapper
- `claude-3.5-sonnet`, `claude-3.5-haiku`: Anthropic models
- `gpt-4o`, `gpt-4o-mini`: OpenAI models
- `Custom("cmd:python3 my_script.py")`: Custom command execution

Agent commands are built in `ProcessManager::build_agent_command()` (src/process.rs:217).

### Worktree Creation Flow
1. User creates session with branch name
2. `GitWorktree::create_worktree()` checks if branch exists locally or remotely
3. Creates worktree in `~/.bunshin/worktrees/{session-name}-{branch-name}`
4. For agents, creates per-agent branches: `agent-{id}-{timestamp}`
5. Sets `agent.artifacts_path` to the worktree directory

See `src/git.rs:43` for the worktree creation logic.

### Environment Variables
Agents receive these environment variables:
- `BUNSHIN_AGENT_ID`, `BUNSHIN_AGENT_NAME`
- `BUNSHIN_SESSION_ID`, `BUNSHIN_WINDOW_ID`
- `BUNSHIN_MODEL`: The model type (e.g., "claude-code")
- `BUNSHIN_PROJECT`: Project name (if applicable)
- `BUNSHIN_TASK`: Task description (if provided)
- `BUNSHIN_WORKTREE_PATH`: Path to isolated worktree

Set in `src/process.rs:100`.

### State Persistence

**Session Storage** (Legacy TUI System):
- Location: `~/.config/bunshin/sessions.json` (or XDG config dir on Linux)
- Type: `SessionManager` storing `Vec<Session>`
- Used by: `bunshin` (TUI), `bunshin init`, `bunshin ls`, `bunshin manager`
- Fields: name, worktree_path, branch, repo_path, claude_pid, created_at

**Agent Management** (New CLI System):
- Location: `~/.bunshin/manager.json`
- Type: `BunshinManager` storing hierarchical sessions/windows/agents
- Used by: `bunshin spawn`, `bunshin ps`, `bunshin kill`, etc.
- Note: Currently separate from legacy session storage

**Other Storage**:
- Agent logs: `~/.bunshin/logs/{agent_id}.log`
- Worktrees: `~/.bunshin/worktrees/`

The legacy `SessionManager` (src/session.rs) is used for simple session tracking (worktrees + Claude Code), while `BunshinManager` (src/manager.rs) provides the full hierarchical multi-agent system.

### Process Communication
Agents use stdin/stdout/stderr channels with background threads:
- `stdin_sender`: Send commands to agent
- `stdout_receiver`: Read agent output
- `stderr_receiver`: Read agent errors
- All output is logged to per-agent log files

See `src/process.rs:132` for channel setup.

## Common Development Tasks

### Adding a New Agent Model
1. Add variant to `AgentModel` enum in `src/core.rs:51`
2. Update `Display` and `FromStr` implementations
3. Add command builder case in `src/process.rs:217`
4. Update CLI help text in `src/cli.rs:48`

### Adding a New CLI Command
1. Add command to `Commands` enum in `src/cli.rs:14`
2. Implement handler function in `src/main.rs` (e.g., `handle_new_command()`)
3. Add command routing in `main()` match statement (src/main.rs:45)

### Modifying Worktree Behavior
Key function: `GitWorktree::create_worktree()` in `src/git.rs:43`
- Handles branch existence checks (local vs remote)
- Determines appropriate git worktree command
- Includes error handling for common git failures

### Testing Worktree Functionality
```bash
# Create test repo
./test_worktree.sh

# Run debug tool
./debug_worktree /tmp/bunshin-test/test-repo my-feature-branch

# Run relevant tests
cargo test test_create_worktree
```

## Key Workflows

### Quick Session Initialization
When you run `bunshin` in a git repository directory, it automatically:
1. Detects that pwd is a git repo
2. Creates a session named after the directory
3. Auto-generates a branch name: `bunshin-YYYYMMDD-HHMMSS`
4. Creates an isolated worktree in `~/.bunshin/worktrees/{session}-{branch}`
5. Launches Claude Code in the worktree
6. Attaches you to the session shell

This is implemented in `handle_init_session()` (src/main.rs:855).

### Manual Session Creation
If not in a git repo, `bunshin` falls back to the TUI session manager where you can manually configure sessions.

## Important Notes

### Claude Code Detection
The system automatically detects Claude Code in these locations (in order):
1. `claude` command in PATH (checks via `which`)
2. `/opt/homebrew/bin/claude` (Homebrew on Apple Silicon)
3. `/usr/local/bin/claude` (Homebrew on Intel Mac)
4. `~/.local/bin/claude` (User local install)
5. `~/.claude/local/claude` (Legacy location)

If Claude Code is not found, `bunshin` will:
- Show an error message with installation instructions
- Exit immediately without creating any session or worktree
- **Refuse to proceed** - Claude Code is mandatory for all Bunshin operations

See `ClaudeCodeManager::find_claude_binary()` in src/claude.rs:9.

- **Python Agent Path**: The Python agent wrapper path is hardcoded in `src/process.rs:229`. Update this to match your installation path or make it configurable.
- **Platform Compatibility**: Shell launching uses Unix-specific APIs (`CommandExt::exec()` at src/main.rs:998). Windows support would require conditional compilation.
- **Git Requirements**: Requires git 2.5+ for worktree support. System must have git in PATH.
- **Agent Lifecycle**: Agents can be in Starting, Running, Idle, Stopping, Stopped, or Error states. Check `is_running()` before operations.
- **Concurrency**: Process channels use mpsc (multi-producer, single-consumer). The stdout/stderr handlers run in background threads.

## Testing Strategy

The codebase includes:
- Unit tests in module files (e.g., `src/core.rs:287`, `src/git.rs:267`)
- Integration tests in `tests/integration_tests.rs`
- Test helpers: `GitWorktree::init_test_repo()`, `GitWorktree::create_test_branch()`

Use `tempfile::TempDir` for isolated test environments to avoid polluting the file system.
