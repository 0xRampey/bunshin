# Feature Verification Guide

This document describes how to verify the session grouping feature.

## Quick Verification

### 1. Install and Build

```bash
# Clone and build
git clone https://github.com/0xRampey/bunshin
cd bunshin
cargo build --release

# Install
cargo install --path cli
```

### 2. Create Test Sessions

```bash
# In repo A
cd ~/projects/my-app
bunshin
# Press 'c' to create session, name it "my-app-dev"
# Press 'q' to exit

# In repo B
cd ~/projects/other-project
bunshin
# Press 'c' to create session, name it "other-feature"
```

### 3. Verify Grouping

Open session manager with `Ctrl+b s`:

**Expected Output:**
```
              Bunshin - Claude Code Orchestrator
─────────────────────────────────────────────────────
▼ my-app (1)
    * my-app-dev        1 window   1 pane   1 client

▼ other-project (1)
      other-feature     1 window   1 pane   1 client
```

### 4. Verify Persistence

```bash
# Check the sessions file
cat ~/.bunshin/sessions.json
# Should show: {"my-app-dev":"my-app","other-feature":"other-project"}
```

## Recording a Demo

### Using asciinema

```bash
# Install asciinema
pip install asciinema

# Record
asciinema rec --title "Bunshin Session Grouping" demo.cast

# In recording:
# 1. cd to a git repo
# 2. Run bunshin
# 3. Create a few sessions (press 'c' or 'N')
# 4. Open session manager (Ctrl+b s)
# 5. Show grouping
# 6. Navigate with j/k
# 7. Exit

# Upload (optional)
asciinema upload demo.cast
```

### Taking Screenshots

1. Open session manager (`Ctrl+b s`)
2. Take screenshot with your OS tool
3. Attach to PR description

## Automated Verification

Run the verification script:

```bash
./scripts/verify-grouping.sh
```

## What to Check

| Feature | How to Verify |
|---------|---------------|
| Repo detection | Create session in git repo, check sessions.json |
| Grouping | Open session manager, verify headers show repo names |
| Persistence | Restart bunshin, verify grouping persists |
| Navigation | Use j/k to move through grouped list |
| "Other" group | Create session outside git repo, should show in "Other" |
| New session | Press 'c' or 'N', verify it gets current repo |

## CI/CD Integration

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs:

1. **Build** - Compiles CLI and WASM plugin
2. **Test** - Runs unit tests
3. **Lint** - Checks formatting and clippy
4. **Integration** - Verifies setup and session handling

For visual verification in CI, consider:
- Using `ttyd` or `gotty` for web-based terminal
- Capturing terminal output with `script`
- Using Playwright/Puppeteer with a terminal emulator
