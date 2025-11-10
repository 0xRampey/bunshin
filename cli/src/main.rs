use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const PLUGIN_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bunshin.wasm"));
const ZELLIJ_VERSION: &str = "0.43.1";

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
  Ctrl+b c  Create new tab/window
  Ctrl+b d  Detach from session

Inside session manager:
  C         Spawn Claude in new pane
  A         Spawn Claude in new tab
  N         Create new session with Claude
  ?         Show help
  q         Close manager

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

    // Check if setup is needed
    if plugin_path.exists() && config_path.exists() && layout_path.exists() {
        // Setup already done, skip silently
        return Ok(());
    }

    // Create directories
    fs::create_dir_all(&plugin_dir)?;
    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&bin_dir)?;

    // Extract embedded plugin WASM
    let mut file = fs::File::create(&plugin_path)?;
    file.write_all(PLUGIN_WASM)?;

    // Create config file
    create_config_file(&config_path, &plugin_path)?;

    // Create layout file
    create_layout_file(&layout_path)?;

    // Check for Zellij
    match which_zellij() {
        Some(_path) => {}
        None => {
            println!("Zellij not found in PATH");
            println!("Please install Zellij:");
            println!("  cargo install zellij");
            println!("  or visit: https://zellij.dev/documentation/installation");
        }
    }

    Ok(())
}

fn which_zellij() -> Option<PathBuf> {
    which::which("zellij").ok()
}

fn create_config_file(path: &Path, plugin_path: &Path) -> Result<()> {
    let config = format!(
        r#"// Bunshin (分身) - Auto-generated Configuration

// Disable welcome screen and tips
show_startup_tips false
show_release_notes false

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
            NewTab;
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
        plugin_path.display()
    );

    fs::write(path, config)?;
    Ok(())
}

fn create_layout_file(path: &Path) -> Result<()> {
    let layout = r#"layout {
    pane size=1 borderless=true {
        plugin location="tab-bar"
    }
    pane split_direction="Vertical" {
        pane {
            command "claude"
            // cwd defaults to current working directory
        }
    }
    pane size=2 borderless=true {
        plugin location="status-bar"
    }
}
"#;

    fs::write(path, layout)?;
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
