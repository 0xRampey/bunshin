# Bunshin (分身) - Claude Code Orchestrator

**Bunshin** (Japanese for "shadow clone") is a one-command installer that transforms [Zellij](https://zellij.dev) into a powerful AI development orchestrator for Claude Code. Manage multiple Claude instances, sessions, and AI-powered development workflows with familiar tmux-style keybindings.

> *Create shadow clones of Claude Code across your development workspace*

## ✨ Features

- **One-Command Install**: `cargo install bunshin` → done!
- **Auto-Setup**: Automatically installs and configures everything on first run
- **Claude Auto-Start**: Launches Claude Code instantly in your current directory
- **🍴 Conversation Forking**: Automatically fork Claude conversations when opening new panes
- **Session Management**: tmux-style session orchestrator built into Zellij
- **Multi-Instance Support**: Run multiple Claude instances across sessions
- **Embedded Assets**: Plugin and configs bundled in the binary
- **Zero Configuration**: Works out of the box with sensible defaults

## 🚀 Quick Start

### Option 1: Install from crates.io (Compile from source)

```bash
# Add WASM target (one-time setup)
rustup target add wasm32-wasip1

# Install bunshin
cargo install bunshin
```

### Option 2: Download pre-built binary (Recommended)

Download the latest release for your platform from [GitHub Releases](https://github.com/0xRampey/bunshin/releases):

```bash
# Linux x86_64
curl -L https://github.com/0xRampey/bunshin/releases/latest/download/bunshin-linux-x86_64.tar.gz | tar xz
sudo mv bunshin /usr/local/bin/

# macOS Apple Silicon
curl -L https://github.com/0xRampey/bunshin/releases/latest/download/bunshin-macos-aarch64.tar.gz | tar xz
sudo mv bunshin /usr/local/bin/
```

**Pre-built binaries are completely standalone** - no Rust or additional dependencies needed!

### Prerequisites

- **Claude Code**: Must have `claude` in your PATH
  - Get it from: https://claude.ai/download
- **Zellij**: Terminal multiplexer
  - Install: `cargo install zellij` or `brew install zellij`

### First Run

```bash
bunshin
```

That's it! On first run, Bunshin will:
1. ✅ Check for Zellij (prompts install if missing)
2. ✅ Extract embedded plugin and configs to `~/.bunshin/`
3. ✅ Launch Zellij with Claude Code auto-started

## ⌨️ Keybindings

### Quick Launch

| Key | Action |
|-----|--------|
| `Ctrl+b s` | Open Bunshin session manager |
| `Ctrl+b c` | Create new tab/window |
| `Ctrl+b d` | Detach from session |

### Session Manager (Ctrl+b s)

#### Navigation
| Key | Action |
|-----|--------|
| `j`, `↓` | Move down in session list |
| `k`, `↑` | Move up in session list |
| `g`, `Home` | Jump to first session |
| `G`, `End` | Jump to last session |

#### Session Actions
| Key | Action |
|-----|--------|
| `Enter` | Switch to selected session |
| `c` | Create new session |
| `$` | Rename current session |
| `x` | Kill selected session |
| `d` | Detach from session |
| `(` | Switch to previous session |
| `)` | Switch to next session |

#### Claude Code Orchestration 🤖
| Key | Action |
|-----|--------|
| `C` | **Launch Claude in new pane** (🍴 forks conversation!) |
| `A` | **Launch Claude in new tab** (🍴 forks conversation!) |
| `N` | **Create new session with Claude** (auto-named) |

#### Other
| Key | Action |
|-----|--------|
| `?` | Toggle help screen |
| `q`, `Esc` | Close manager |

## 📖 Usage Examples

### Launch Bunshin
```bash
# Launch with Claude auto-start (auto-setup on first run)
bunshin

# Show help
bunshin --help

# Show version
bunshin --version
```

### Workflow: Multiple AI-Assisted Projects

1. **Launch**: `bunshin`
2. **Create sessions** for different projects:
   - Press `Ctrl+b s` → `c` → "frontend" → Enter
   - Press `Ctrl+b s` → `c` → "backend" → Enter
3. **Launch Claude in each**:
   - Switch to frontend session → `Ctrl+b s` → `C`
   - Switch to backend session → `Ctrl+b s` → `C`
4. **Quick switch** between projects:
   - `Ctrl+b s` → `(` / `)`

### Workflow: Dedicated Claude Tab

1. Launch Bunshin: `bunshin`
2. Press `Ctrl+b s` → `A` to create a new tab with Claude
3. Switch between tabs with `Ctrl+b c`

### 🍴 Conversation Forking Workflow

Explore multiple solution paths from the same conversation starting point:

1. **Start**: Launch `bunshin` - Claude starts in the first pane
2. **Build context**: Have a conversation with Claude about your problem
3. **Fork to explore**: Press `Ctrl+b s` → `C` to open a new pane
   - The new pane automatically resumes from your first conversation!
   - Try a different approach while keeping the original conversation intact
4. **Compare solutions**: Switch between panes to compare different approaches
5. **Fork again**: Keep forking to explore even more alternatives

**How it works:**
- **First pane**: Starts a fresh Claude conversation
- **Subsequent panes**: Automatically fork using `claude --resume <session-id>`
- **Smart tracking**: Bunshin tracks your parent session per Zellij session
- **Multiple explorations**: Try different solutions without losing your original context

## 🏗️ Architecture

```
bunshin/
├── cli/                          # Binary crate
│   ├── build.rs                  # Compiles plugin, embeds WASM
│   └── src/
│       └── main.rs               # CLI: setup + launch
├── plugin/                       # Plugin crate (WASM)
│   └── src/
│       └── main.rs               # Zellij plugin (session manager)
└── Cargo.toml                    # Workspace config
```

### How It Works

1. **Build Time**: `build.rs` compiles the plugin to WASM and embeds it in the CLI binary
2. **First Run**: CLI extracts embedded assets to `~/.bunshin/`
3. **Every Run**: Launches Zellij with Bunshin configuration

### Files Created

```
~/.bunshin/
├── plugins/
│   └── bunshin.wasm              # Embedded session manager plugin
├── config/
│   ├── config.kdl                # Zellij keybindings config
│   └── layout.kdl                # Claude auto-start layout
├── bin/
│   └── claude-fork               # Conversation forking wrapper script
└── state/
    └── <session-name>.*          # Session state tracking for forking
```

## 🔧 Configuration

### Custom Claude Path

If Claude is not in your PATH, you can modify the launch command in `~/.bunshin/config/layout.kdl`:

```kdl
layout {
    pane {
        command "/your/custom/path/to/claude"
    }
}
```

### Custom Keybindings

Edit `~/.bunshin/config/config.kdl` to customize keybindings.

## 🆚 Comparison

| Feature | Bunshin | tmux + manual setup |
|---------|---------|-------------------|
| Install | `cargo install bunshin` | Multiple steps |
| Configuration | Zero (auto-configured) | Manual config files |
| Claude Integration | Built-in (`C`, `A`, `N` keys) | Manual scripting |
| Session Manager | Beautiful TUI | Text-based |
| First-time Setup | < 1 minute | 15+ minutes |

## 🐛 Troubleshooting

### Zellij not found

```bash
cargo install zellij
# or
brew install zellij  # macOS
```

### Claude not found

Ensure `claude` is in your PATH:
```bash
which claude
# Should output: /path/to/claude
```

### Re-run setup

```bash
bunshin --setup
```

## 🤝 Contributing

Contributions welcome! This is a standalone installer for the Bunshin plugin.

## 📄 License

MIT License - see LICENSE file for details

## 🎉 Credits

- Built on [Zellij](https://zellij.dev) plugin SDK
- Inspired by tmux session management
- Enhanced for AI-powered development with Claude Code

## 🔗 Links

- **Claude Code**: https://claude.ai/download
- **Zellij**: https://zellij.dev
- **Repository**: https://github.com/0xRampey/bunshin

---

**Ready to create shadow clones! 🥷✨**
