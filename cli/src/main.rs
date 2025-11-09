use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const PLUGIN_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bunshin.wasm"));
const ZELLIJ_VERSION: &str = "0.43.1";

const CLAUDE_FORK_SCRIPT: &str = r#"
# Claude conversation forking wrapper for bunshin
# Works with SessionStart hook for instant session ID capture

set -euo pipefail

# State directory for tracking session IDs
STATE_DIR="${HOME}/.bunshin/state"
mkdir -p "${STATE_DIR}"

# Get the current Zellij session name to namespace our state files
ZELLIJ_SESSION="${ZELLIJ_SESSION_NAME:-default}"
SESSION_STATE_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.parent_session"
PANE_COUNT_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.pane_count"
LOCK_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.lock"
DEBUG_LOG="${STATE_DIR}/${ZELLIJ_SESSION}.debug.log"

# Enable debug logging if BUNSHIN_DEBUG is set
DEBUG="${BUNSHIN_DEBUG:-0}"

# Debug logging function
log_debug() {
    if [[ "${DEBUG}" == "1" ]]; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${DEBUG_LOG}"
    fi
}

# Function to acquire lock
acquire_lock() {
    local timeout=10
    local count=0
    while [[ -f "${LOCK_FILE}" ]] && [[ $count -lt $timeout ]]; do
        sleep 0.5
        count=$((count + 1))
    done

    if [[ $count -ge $timeout ]]; then
        log_debug "Failed to acquire lock after ${timeout} seconds"
        return 1
    fi

    echo $$ > "${LOCK_FILE}"
    log_debug "Lock acquired by $$"
    return 0
}

# Function to release lock
release_lock() {
    rm -f "${LOCK_FILE}"
    log_debug "Lock released by $$"
}

# Ensure lock is released on exit
trap release_lock EXIT

# Function to get the parent session ID
get_parent_session() {
    if [[ -f "${SESSION_STATE_FILE}" ]]; then
        local session_id=$(cat "${SESSION_STATE_FILE}")
        log_debug "Retrieved parent session: ${session_id}"
        echo "${session_id}"
    else
        log_debug "No parent session file found"
        echo ""
    fi
}

# Function to increment pane count
increment_pane_count() {
    if ! acquire_lock; then
        echo "1"
        return
    fi

    local count=0
    if [[ -f "${PANE_COUNT_FILE}" ]]; then
        count=$(cat "${PANE_COUNT_FILE}")
    fi
    count=$((count + 1))
    echo "${count}" > "${PANE_COUNT_FILE}"
    log_debug "Incremented pane count to: ${count}"

    release_lock
    echo "${count}"
}

# Main logic
main() {
    log_debug "=== Starting claude-fork wrapper ==="
    log_debug "PWD: $(pwd)"
    log_debug "ZELLIJ_SESSION: ${ZELLIJ_SESSION}"

    # Increment the pane count
    local pane_num
    pane_num=$(increment_pane_count)

    log_debug "Current pane number: ${pane_num}"

    if [[ "${pane_num}" -eq 1 ]]; then
        # This is the first pane - launch Claude normally
        echo "🌱 Launching first Claude pane in this session..."
        echo "   (SessionStart hook will capture session ID instantly)"
        echo ""
        log_debug "First pane - launching Claude normally"
        log_debug "SessionStart hook will save session ID to: ${SESSION_STATE_FILE}"

        # No background process needed! SessionStart hook handles it instantly.
        exec claude "$@"
    else
        # This is a subsequent pane - fork the conversation
        log_debug "Subsequent pane (#${pane_num}) - attempting to fork"

        # Get parent session (should already be saved by SessionStart hook)
        local parent_session
        parent_session=$(get_parent_session)

        # Wait briefly if not found yet (hook should be instant though)
        if [[ -z "${parent_session}" ]]; then
            log_debug "Parent session not found, waiting briefly..."
            local waited=0
            while [[ -z "${parent_session}" ]] && [[ $waited -lt 5 ]]; do
                sleep 1
                waited=$((waited + 1))
                parent_session=$(get_parent_session)
                log_debug "Waiting for SessionStart hook... (${waited}/5)"
            done
        fi

        if [[ -n "${parent_session}" ]]; then
            # Verify the session file exists
            local session_pattern="${HOME}/.claude/projects/*/${parent_session}.jsonl"
            if ls ${session_pattern} 1> /dev/null 2>&1; then
                # Fork the conversation using --resume
                echo "🍴 Forking conversation from session: ${parent_session}"
                echo "   (Pane #${pane_num} - exploring a different path)"
                echo ""
                log_debug "Forking with: claude --resume ${parent_session}"

                # NOTE: --resume creates a NEW session file (a fork), not shared state
                # Each resumed session gets its own independent session file
                exec claude --resume "${parent_session}" "$@"
            else
                log_debug "Session file not found for: ${parent_session}"
                echo "⚠️  Session file not found, launching new conversation..."
                exec claude "$@"
            fi
        else
            # Fallback: launch normally if we couldn't find a parent session
            log_debug "No parent session found after waiting"
            echo "⚠️  Could not find parent session, launching new conversation..."
            echo "   Tip: Make sure SessionStart hook is configured in ~/.claude/settings.json"
            echo ""
            exec claude "$@"
        fi
    fi
}

# Run main function
main "$@"
"#;

const SESSION_CAPTURE_HOOK: &str = r#"#!/bin/bash
# Bunshin SessionStart hook - captures session ID instantly
# This eliminates the 10-second delay from the fork wrapper
set -euo pipefail

# Read JSON from stdin
INPUT=$(cat)

# Extract session metadata
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
SOURCE=$(echo "$INPUT" | jq -r '.source // "startup"')

# Only save on startup (not resume/clear/compact)
# When forking (source="resume"), we DON'T want to overwrite the parent session
if [[ "$SOURCE" == "startup" ]] && [[ -n "$SESSION_ID" ]]; then
    # Get Zellij session name from environment
    ZELLIJ_SESSION="${ZELLIJ_SESSION_NAME:-default}"
    STATE_DIR="${HOME}/.bunshin/state"
    mkdir -p "${STATE_DIR}"

    # Save the session ID immediately - no more 10 second delay!
    echo "${SESSION_ID}" > "${STATE_DIR}/${ZELLIJ_SESSION}.parent_session"

    # Debug logging if enabled
    if [[ "${BUNSHIN_DEBUG:-0}" == "1" ]]; then
        DEBUG_LOG="${STATE_DIR}/${ZELLIJ_SESSION}.debug.log"
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] [SessionStart Hook] Captured session ID instantly: ${SESSION_ID}" >> "${DEBUG_LOG}"
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] [SessionStart Hook] Source: ${SOURCE}" >> "${DEBUG_LOG}"
    fi
fi

# Return success immediately (non-blocking)
exit 0
"#;


fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // Parse command-line arguments
    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-v" => {
                println!("Bunshin (分身) v{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {}
        }
    }

    // Automatically ensure setup (extracts files only if missing)
    setup()?;

    // Launch Zellij with Bunshin configuration
    launch()?;

    Ok(())
}

fn print_help() {
    println!(r#"
╔════════════════════════════════════════════════════════════════╗
║                                                                ║
║         🥷 Bunshin (分身) - Claude Code Orchestrator 🥷        ║
║                                                                ║
║              Shadow Clone Technique for AI Development         ║
║                                                                ║
╚════════════════════════════════════════════════════════════════╝

Usage:
  bunshin                    Launch Bunshin (auto-starts Claude)
  bunshin --version          Show version
  bunshin --help             Show this help

Keybindings:
  Ctrl+b s  Open Bunshin session manager (tmux-style!)
  Ctrl+b c  Create new tab/window (🍴 forks conversation!)
  Ctrl+b d  Detach from session

Inside session manager:
  C         Spawn Claude in new pane (🍴 forks conversation!)
  A         Spawn Claude in new tab (🍴 forks conversation!)
  N         Create new session with Claude
  ?         Show help
  q         Close manager

🌟 Conversation Forking:
  Bunshin automatically forks conversations when you create new tabs/panes.
  This lets you explore different solution paths from the same starting point!

  Ways to fork:
  - Ctrl+b c: Create new tab with forked conversation
  - Session manager 'C': Spawn Claude in new pane (forked)
  - Session manager 'A': Spawn Claude in new tab (forked)

  How it works:
  - First pane: Starts fresh conversation
  - Subsequent panes/tabs: Fork from the first pane's conversation

Examples:
  bunshin                    # Launch (Claude auto-starts)

Note: On first run, Bunshin automatically extracts configs to ~/.bunshin/
      You can edit these files to customize your setup.

Documentation: https://github.com/0xRampey/bunshin
"#);
}

fn get_bunshin_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join(".bunshin"))
}

fn setup() -> Result<()> {
    let bunshin_dir = get_bunshin_dir()?;
    let plugin_dir = bunshin_dir.join("plugins");
    let config_dir = bunshin_dir.join("config");
    let bin_dir = bunshin_dir.join("bin");

    let plugin_path = plugin_dir.join("bunshin.wasm");
    let config_path = config_dir.join("config.kdl");
    let layout_path = config_dir.join("layout.kdl");
    let fork_script_path = bin_dir.join("claude-fork");
    let hook_path = bin_dir.join("bunshin-session-capture");

    // Check if any setup is needed
    let need_setup = !plugin_path.exists() || !config_path.exists() || !layout_path.exists() || !fork_script_path.exists() || !hook_path.exists();

    if !need_setup {
        // All files exist, skip silently
        return Ok(());
    }

    println!("\n🥷 Setting up Bunshin...\n");

    // Create directories
    fs::create_dir_all(&plugin_dir)?;
    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&bin_dir)?;

    // Extract embedded plugin WASM if missing
    if !plugin_path.exists() {
        println!("📦 Installing Bunshin plugin...");
        let mut file = fs::File::create(&plugin_path)?;
        file.write_all(PLUGIN_WASM)?;
        println!("   ✅ Plugin installed: {}", plugin_path.display());
    }

    // Create config file if missing
    if !config_path.exists() {
        println!("⚙️  Creating configuration...");
        create_config_file(&config_path, &plugin_path, &fork_script_path)?;
        println!("   ✅ Config created: {}", config_path.display());
    }

    // Create layout file if missing
    if !layout_path.exists() {
        create_layout_file(&layout_path, &fork_script_path)?;
        println!("   ✅ Layout created: {}", layout_path.display());
    }

    // Install claude-fork wrapper script if missing
    if !fork_script_path.exists() {
        println!("🍴 Installing conversation fork wrapper...");
        install_claude_fork_script(&fork_script_path)?;
        println!("   ✅ Fork wrapper installed: {}", fork_script_path.display());
    }

    // Install SessionStart hook for instant session capture if missing
    if !hook_path.exists() {
        println!("⚡ Installing SessionStart hook for instant session capture...");
        install_session_capture_hook(&hook_path)?;
        println!("   ✅ SessionStart hook installed: {}", hook_path.display());
    }

    // Configure Claude to use the SessionStart hook
    println!("🔧 Configuring Claude to use bunshin SessionStart hook...");
    match configure_claude_hook(&hook_path) {
        Ok(_) => println!("   ✅ Claude hook configured successfully"),
        Err(e) => println!("   ⚠️  Warning: Could not configure Claude hook: {}", e),
    }

    // Check for Zellij
    if !plugin_path.exists() || !config_path.exists() {
        println!("🔍 Checking for Zellij...");
        match which_zellij() {
            Some(path) => {
                println!("   ✅ Found Zellij: {}", path.display());
            }
            None => {
                println!("   ⚠️  Zellij not found in PATH");
                println!("   📥 Please install Zellij:");
                println!("      cargo install zellij");
                println!("      or visit: https://zellij.dev/documentation/installation");
            }
        }
    }

    Ok(())
}

fn which_zellij() -> Option<PathBuf> {
    which::which("zellij").ok()
}

fn create_config_file(path: &Path, plugin_path: &Path, fork_script_path: &Path) -> Result<()> {
    let config = format!(
        r#"// Bunshin (分身) - Auto-generated Configuration

keybinds clear-defaults=true {{
    normal {{
        // Tmux-style prefix keybinding
        bind "Ctrl b" {{ SwitchToMode "tmux"; }}
    }}
    tmux {{
        bind "s" {{
            LaunchOrFocusPlugin "file:{}" {{
                floating true
                move_to_focused_tab true
            }}
            SwitchToMode "normal";
        }}
        bind "c" {{
            NewTab {{
                layout {{
                    pane size=1 borderless=true {{
                        plugin location="tab-bar"
                    }}
                    pane split_direction="Vertical" {{
                        pane {{
                            command "{}"
                        }}
                    }}
                    pane size=2 borderless=true {{
                        plugin location="status-bar"
                    }}
                }}
            }}
            SwitchToMode "normal";
        }}
        bind "d" {{
            Detach;
        }}
        bind "Ctrl c" "Esc" {{
            SwitchToMode "normal";
        }}
    }}
    locked {{
        bind "Ctrl g" {{ SwitchToMode "normal"; }}
    }}
}}
"#,
        plugin_path.display(),
        fork_script_path.display()
    );

    fs::write(path, config)?;
    Ok(())
}

fn create_layout_file(path: &Path, fork_script_path: &Path) -> Result<()> {
    let layout = format!(
        r#"layout {{
    pane size=1 borderless=true {{
        plugin location="tab-bar"
    }}
    pane split_direction="Vertical" {{
        pane {{
            command "{}"
            // cwd defaults to current working directory
        }}
    }}
    pane size=2 borderless=true {{
        plugin location="status-bar"
    }}
}}
"#,
        fork_script_path.display()
    );

    fs::write(path, layout)?;
    Ok(())
}

fn install_claude_fork_script(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // Write the script
    fs::write(path, CLAUDE_FORK_SCRIPT)?;

    // Make it executable
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;

    Ok(())
}

fn install_session_capture_hook(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // Write the hook script
    fs::write(path, SESSION_CAPTURE_HOOK)?;

    // Make it executable
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;

    Ok(())
}

fn configure_claude_hook(hook_path: &Path) -> Result<()> {
    let claude_dir = dirs::home_dir()
        .context("Could not find home directory")?
        .join(".claude");
    let settings_path = claude_dir.join("settings.json");
    
    // Ensure .claude directory exists
    fs::create_dir_all(&claude_dir)?;
    
    // Read or create settings.json
    let settings_content = if settings_path.exists() {
        fs::read_to_string(&settings_path)?
    } else {
        r#"{
    "$schema": "https://json.schemastore.org/claude-code-settings.json"
}"#.to_string()
    };
    
    // Parse JSON
    let mut settings: serde_json::Value = serde_json::from_str(&settings_content)
        .context("Failed to parse settings.json")?;
    
    // Check if our hook is already configured
    let hook_path_str = hook_path.display().to_string();
    let hook_already_exists = settings
        .get("hooks")
        .and_then(|h| h.get("SessionStart"))
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter().any(|item| {
                item.get("hooks")
                    .and_then(|hooks| hooks.as_array())
                    .map(|hook_arr| {
                        hook_arr.iter().any(|h| {
                            h.get("command")
                                .and_then(|c| c.as_str())
                                .map(|cmd| cmd == hook_path_str)
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    
    if hook_already_exists {
        return Ok(());
    }
    
    // Add SessionStart hook
    let hooks = settings
        .as_object_mut()
        .and_then(|obj| {
            obj.entry("hooks")
                .or_insert(serde_json::json!({}))
                .as_object_mut()
        })
        .context("Failed to get hooks object")?;
    
    let session_start = hooks
        .entry("SessionStart")
        .or_insert(serde_json::json!([]))
        .as_array_mut()
        .context("SessionStart is not an array")?;
    
    // Add our hook
    session_start.push(serde_json::json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": hook_path_str
        }]
    }));
    
    // Write back
    let pretty_json = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, pretty_json)?;
    
    Ok(())
}


fn launch() -> Result<()> {
    let zellij_path = which_zellij().context(
        "Zellij not found in PATH. Please install it:\n  cargo install zellij\n  or visit: https://zellij.dev/documentation/installation",
    )?;

    let bunshin_dir = get_bunshin_dir()?;
    let config_path = bunshin_dir.join("config/config.kdl");
    let layout_path = bunshin_dir.join("config/layout.kdl");

    // Launch Zellij with Bunshin configuration
    let mut cmd = Command::new(zellij_path);
    cmd.arg("--config").arg(&config_path);
    cmd.arg("--layout").arg(&layout_path);

    // Set ZELLIJ_CONFIG_DIR environment variable
    cmd.env("ZELLIJ_CONFIG_DIR", bunshin_dir.join("config"));

    let status = cmd.status()?;

    if !status.success() {
        anyhow::bail!("Zellij exited with error");
    }

    Ok(())
}
