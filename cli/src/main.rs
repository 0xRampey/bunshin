use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const PLUGIN_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bunshin.wasm"));
const ZELLIJ_VERSION: &str = "0.43.1";

const CLAUDE_FORK_SCRIPT: &str = r#"#!/bin/bash
# Claude conversation forking wrapper for bunshin
# This script manages conversation forking when opening new panes in a bunshin session

set -euo pipefail

# State directory for tracking session IDs
STATE_DIR="${HOME}/.bunshin/state"
mkdir -p "${STATE_DIR}"

# Get the current Zellij session name to namespace our state files
ZELLIJ_SESSION="${ZELLIJ_SESSION_NAME:-default}"
SESSION_STATE_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.parent_session"
PANE_COUNT_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.pane_count"

# Function to get the most recent Claude session ID from the projects directory
get_most_recent_claude_session() {
    local project_dir="${HOME}/.claude/projects"
    if [[ ! -d "${project_dir}" ]]; then
        echo ""
        return
    fi

    # Find the most recent .jsonl file (excluding agent-* files)
    local recent_file=$(find "${project_dir}" -type f -name "*.jsonl" ! -name "agent-*.jsonl" -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)

    if [[ -n "${recent_file}" ]]; then
        # Extract the session ID from the filename (it's the UUID before .jsonl)
        basename "${recent_file}" .jsonl
    else
        echo ""
    fi
}

# Function to save the parent session ID
save_parent_session() {
    local session_id="$1"
    echo "${session_id}" > "${SESSION_STATE_FILE}"
}

# Function to get the parent session ID
get_parent_session() {
    if [[ -f "${SESSION_STATE_FILE}" ]]; then
        cat "${SESSION_STATE_FILE}"
    else
        echo ""
    fi
}

# Function to increment pane count
increment_pane_count() {
    local count=0
    if [[ -f "${PANE_COUNT_FILE}" ]]; then
        count=$(cat "${PANE_COUNT_FILE}")
    fi
    count=$((count + 1))
    echo "${count}" > "${PANE_COUNT_FILE}"
    echo "${count}"
}

# Function to get pane count
get_pane_count() {
    if [[ -f "${PANE_COUNT_FILE}" ]]; then
        cat "${PANE_COUNT_FILE}"
    else
        echo "0"
    fi
}

# Main logic
main() {
    # Increment the pane count
    local pane_num
    pane_num=$(increment_pane_count)

    # Get parent session if it exists
    local parent_session
    parent_session=$(get_parent_session)

    if [[ "${pane_num}" -eq 1 ]]; then
        # This is the first pane - launch Claude normally
        echo "🌱 Launching first Claude pane in this session..."
        echo "   (Subsequent panes will fork from this conversation)"

        # Launch Claude normally, but save its session ID for forking
        # We'll capture the session ID after Claude initializes
        (
            # Wait for Claude to initialize and create its session file
            sleep 3

            # Get the most recent Claude session
            local session_id
            session_id=$(get_most_recent_claude_session)

            if [[ -n "${session_id}" ]]; then
                save_parent_session "${session_id}"
            fi
        ) >/dev/null 2>&1 &

        exec claude "$@"
    else
        # This is a subsequent pane - try to fork the conversation
        if [[ -z "${parent_session}" ]]; then
            # Parent session not found yet, try to get it
            parent_session=$(get_most_recent_claude_session)

            if [[ -n "${parent_session}" ]]; then
                save_parent_session "${parent_session}"
            fi
        fi

        if [[ -n "${parent_session}" ]]; then
            # Fork the conversation
            echo "🍴 Forking conversation from session: ${parent_session}"
            echo "   (Pane #${pane_num} - continuing from where you left off)"
            echo ""

            exec claude --resume "${parent_session}" "$@"
        else
            # Fallback: launch normally if we couldn't find a parent session
            echo "⚠️  Could not find parent session, launching new conversation..."
            exec claude "$@"
        fi
    fi
}

# Run main function
main "$@"
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

    // Check if any setup is needed
    let need_setup = !plugin_path.exists() || !config_path.exists() || !layout_path.exists() || !fork_script_path.exists();

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
