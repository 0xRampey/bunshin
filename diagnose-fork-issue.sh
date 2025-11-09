#!/bin/bash
# Diagnostic script for bunshin conversation forking issues

echo "🔍 Bunshin Conversation Forking Diagnostic"
echo "==========================================="
echo ""

# Check 1: Bunshin binary version
echo "1️⃣ Checking bunshin installation..."
if command -v bunshin &> /dev/null; then
    BUNSHIN_PATH=$(which bunshin)
    echo "   ✅ bunshin found at: $BUNSHIN_PATH"
    BUNSHIN_DATE=$(stat -c '%y' "$BUNSHIN_PATH" 2>/dev/null || stat -f '%Sm' "$BUNSHIN_PATH" 2>/dev/null)
    echo "   📅 Last modified: $BUNSHIN_DATE"
else
    echo "   ❌ bunshin not found in PATH"
    exit 1
fi
echo ""

# Check 2: Fork wrapper script
echo "2️⃣ Checking claude-fork wrapper..."
FORK_WRAPPER="${HOME}/.bunshin/bin/claude-fork"
if [[ -f "$FORK_WRAPPER" ]]; then
    echo "   ✅ Fork wrapper exists: $FORK_WRAPPER"
    if [[ -x "$FORK_WRAPPER" ]]; then
        echo "   ✅ Fork wrapper is executable"
    else
        echo "   ❌ Fork wrapper is NOT executable"
    fi

    # Check for new vs old version
    if grep -q "SessionStart hook will capture" "$FORK_WRAPPER"; then
        echo "   ✅ Using NEW instant-capture fork wrapper"
    elif grep -q "sleep 10" "$FORK_WRAPPER"; then
        echo "   ⚠️  Using OLD 10-second delay fork wrapper"
        echo "   💡 Run: cargo install --path cli --force"
    else
        echo "   ⚠️  Unknown fork wrapper version"
    fi
else
    echo "   ❌ Fork wrapper NOT found"
fi
echo ""

# Check 3: SessionStart hook
echo "3️⃣ Checking SessionStart hook..."
HOOK_PATH="${HOME}/.bunshin/bin/bunshin-session-capture"
if [[ -f "$HOOK_PATH" ]]; then
    echo "   ✅ Hook exists: $HOOK_PATH"
    if [[ -x "$HOOK_PATH" ]]; then
        echo "   ✅ Hook is executable"
    else
        echo "   ❌ Hook is NOT executable"
    fi

    # Check shebang
    FIRST_LINE=$(head -1 "$HOOK_PATH")
    if [[ "$FIRST_LINE" == "#!/bin/bash" ]]; then
        echo "   ✅ Shebang is correct"
    else
        echo "   ❌ Shebang is WRONG: $FIRST_LINE"
        echo "   💡 Should be: #!/bin/bash"
    fi
else
    echo "   ❌ Hook NOT found"
    echo "   💡 Run: rm -f ~/.bunshin/bin/bunshin-session-capture && bunshin --help"
fi
echo ""

# Check 4: Claude settings.json
echo "4️⃣ Checking Claude settings.json..."
SETTINGS_PATH="${HOME}/.claude/settings.json"
if [[ -f "$SETTINGS_PATH" ]]; then
    echo "   ✅ settings.json exists"
    if grep -q "bunshin-session-capture" "$SETTINGS_PATH"; then
        echo "   ✅ Hook configured in settings.json"
        echo "   📄 Hook configuration:"
        jq '.hooks.SessionStart[] | select(.hooks[].command | contains("bunshin"))' "$SETTINGS_PATH" 2>/dev/null || echo "      (jq not available for pretty print)"
    else
        echo "   ❌ Hook NOT configured in settings.json"
        echo "   💡 Run: bunshin --help (will auto-configure)"
    fi
else
    echo "   ❌ settings.json NOT found"
fi
echo ""

# Check 5: Current Zellij session
echo "5️⃣ Checking Zellij session..."
if [[ -n "${ZELLIJ_SESSION_NAME}" ]]; then
    echo "   ✅ Running in Zellij session: ${ZELLIJ_SESSION_NAME}"

    STATE_DIR="${HOME}/.bunshin/state"
    echo "   📂 State directory: $STATE_DIR"

    if [[ -d "$STATE_DIR" ]]; then
        echo "   📋 State files for this session:"
        ls -lh "${STATE_DIR}/${ZELLIJ_SESSION_NAME}"* 2>/dev/null || echo "      (no state files yet)"

        PARENT_SESSION_FILE="${STATE_DIR}/${ZELLIJ_SESSION_NAME}.parent_session"
        if [[ -f "$PARENT_SESSION_FILE" ]]; then
            PARENT_ID=$(cat "$PARENT_SESSION_FILE")
            echo ""
            echo "   🎯 Parent session ID: $PARENT_ID"

            # Check if session file exists
            SESSION_FILE=$(find ~/.claude/projects -name "${PARENT_ID}.jsonl" 2>/dev/null | head -1)
            if [[ -n "$SESSION_FILE" ]]; then
                echo "   ✅ Parent session file exists: $SESSION_FILE"
                FILE_DATE=$(stat -c '%y' "$SESSION_FILE" 2>/dev/null || stat -f '%Sm' "$SESSION_FILE" 2>/dev/null)
                echo "   📅 Last modified: $FILE_DATE"
            else
                echo "   ❌ Parent session file NOT found!"
                echo "   💡 This session ID might be old/invalid"
            fi
        else
            echo ""
            echo "   ⚠️  No parent session captured yet"
            echo "   💡 Try: Start a fresh bunshin session and send a message in first pane"
        fi

        # Check debug log if exists
        DEBUG_LOG="${STATE_DIR}/${ZELLIJ_SESSION_NAME}.debug.log"
        if [[ -f "$DEBUG_LOG" ]]; then
            echo ""
            echo "   📝 Recent debug log entries:"
            tail -10 "$DEBUG_LOG" | sed 's/^/      /'
        fi
    else
        echo "   ⚠️  State directory doesn't exist yet"
    fi
else
    echo "   ⚠️  NOT running in Zellij"
    echo "   💡 This diagnostic should be run inside a bunshin session"
fi
echo ""

# Check 6: Recent Claude sessions
echo "6️⃣ Checking recent Claude sessions..."
CLAUDE_PROJECTS="${HOME}/.claude/projects"
if [[ -d "$CLAUDE_PROJECTS" ]]; then
    echo "   📂 Claude projects directory exists"
    echo "   📋 Most recent Claude sessions (last 5):"
    find "$CLAUDE_PROJECTS" -name "*.jsonl" ! -name "agent-*.jsonl" -type f -printf '%T@ %p\n' 2>/dev/null | \
        sort -rn | head -5 | while read timestamp path; do
            session_id=$(basename "$path" .jsonl)
            file_date=$(date -d "@$timestamp" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || date -r "$timestamp" '+%Y-%m-%d %H:%M:%S' 2>/dev/null)
            echo "      $session_id ($file_date)"
        done
else
    echo "   ❌ Claude projects directory NOT found"
fi
echo ""

# Summary and recommendations
echo "==========================================="
echo "📊 Summary & Recommendations"
echo "==========================================="
echo ""

ISSUES_FOUND=0

if [[ ! -f "$FORK_WRAPPER" ]] || [[ ! -x "$FORK_WRAPPER" ]]; then
    echo "❌ Fork wrapper issue detected"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

if [[ ! -f "$HOOK_PATH" ]] || [[ ! -x "$HOOK_PATH" ]]; then
    echo "❌ SessionStart hook issue detected"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

if [[ -f "$HOOK_PATH" ]]; then
    FIRST_LINE=$(head -1 "$HOOK_PATH")
    if [[ "$FIRST_LINE" != "#!/bin/bash" ]]; then
        echo "❌ Hook shebang is broken"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    fi
fi

if [[ -f "$SETTINGS_PATH" ]] && ! grep -q "bunshin-session-capture" "$SETTINGS_PATH"; then
    echo "❌ Hook not configured in Claude settings"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

if [[ -n "${ZELLIJ_SESSION_NAME}" ]]; then
    PARENT_SESSION_FILE="${STATE_DIR}/${ZELLIJ_SESSION_NAME}.parent_session"
    if [[ ! -f "$PARENT_SESSION_FILE" ]]; then
        echo "⚠️  No parent session captured for current Zellij session"
        echo "   💡 This is expected on first pane - send a message first"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    fi
fi

echo ""
if [[ $ISSUES_FOUND -eq 0 ]]; then
    echo "✅ No major issues detected!"
    echo ""
    echo "If forking still doesn't work:"
    echo "1. Make sure you're in a Zellij session (run 'bunshin')"
    echo "2. Send a message in the first pane and wait for response"
    echo "3. Then press Ctrl+b c to create a new tab"
    echo "4. Enable debug mode: export BUNSHIN_DEBUG=1"
    echo "5. Check logs in ~/.bunshin/state/*.debug.log"
else
    echo "🔧 Recommended fixes:"
    echo ""
    echo "cd /home/user/bunshin"
    echo "cargo install --path cli --force"
    echo "rm -rf ~/.bunshin/state/*  # Clear old state"
    echo "bunshin  # Start fresh session"
fi

echo ""
echo "💡 For debug mode, run:"
echo "   export BUNSHIN_DEBUG=1"
echo "   bunshin"
echo "   # Then check: cat ~/.bunshin/state/*.debug.log"
