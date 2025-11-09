#!/bin/bash
# Test script for bunshin SessionStart hook
set -euo pipefail

echo "🧪 Testing Bunshin SessionStart Hook"
echo "===================================="
echo ""

# Test 1: Hook script exists and is executable
echo "Test 1: Checking hook installation..."
HOOK_PATH="${HOME}/.bunshin/bin/bunshin-session-capture"

if [[ ! -f "$HOOK_PATH" ]]; then
    echo "❌ FAIL: Hook not found at $HOOK_PATH"
    exit 1
fi

if [[ ! -x "$HOOK_PATH" ]]; then
    echo "❌ FAIL: Hook is not executable"
    exit 1
fi

echo "✅ PASS: Hook exists and is executable"
echo ""

# Test 2: Hook is configured in settings.json
echo "Test 2: Checking Claude settings.json configuration..."
SETTINGS_PATH="${HOME}/.claude/settings.json"

if [[ ! -f "$SETTINGS_PATH" ]]; then
    echo "❌ FAIL: settings.json not found"
    exit 1
fi

if grep -q "bunshin-session-capture" "$SETTINGS_PATH"; then
    echo "✅ PASS: Hook configured in settings.json"
else
    echo "❌ FAIL: Hook not found in settings.json"
    exit 1
fi
echo ""

# Test 3: Verify shebang is correct
echo "Test 3: Checking shebang..."
FIRST_LINE=$(head -1 "$HOOK_PATH")
if [[ "$FIRST_LINE" == "#!/bin/bash" ]]; then
    echo "✅ PASS: Shebang is correct"
else
    echo "❌ FAIL: Shebang is incorrect: $FIRST_LINE"
    exit 1
fi
echo ""

# Test 4: Simulate SessionStart hook with test JSON
echo "Test 4: Testing hook with simulated SessionStart event..."

# Clean up test state
TEST_SESSION="test-hook-validation"
STATE_DIR="${HOME}/.bunshin/state"
mkdir -p "$STATE_DIR"
rm -f "${STATE_DIR}/${TEST_SESSION}.parent_session"

# Create test JSON input (simulating what Claude sends)
TEST_JSON=$(cat <<EOF
{
  "session_id": "test-12345-67890-abcdef",
  "source": "startup",
  "transcript_path": "/tmp/test.jsonl",
  "permission_mode": "default",
  "hook_event_name": "SessionStart",
  "cwd": "/tmp"
}
EOF
)

# Run hook with test input
export ZELLIJ_SESSION_NAME="$TEST_SESSION"
echo "$TEST_JSON" | "$HOOK_PATH"

# Check if session ID was saved
if [[ -f "${STATE_DIR}/${TEST_SESSION}.parent_session" ]]; then
    SAVED_ID=$(cat "${STATE_DIR}/${TEST_SESSION}.parent_session")
    if [[ "$SAVED_ID" == "test-12345-67890-abcdef" ]]; then
        echo "✅ PASS: Hook captured and saved session ID correctly"
    else
        echo "❌ FAIL: Wrong session ID saved: $SAVED_ID"
        exit 1
    fi
else
    echo "❌ FAIL: Hook did not save session ID"
    exit 1
fi
echo ""

# Test 5: Verify hook doesn't save on 'resume' source
echo "Test 5: Testing hook ignores 'resume' events..."

rm -f "${STATE_DIR}/${TEST_SESSION}.parent_session"

RESUME_JSON=$(cat <<EOF
{
  "session_id": "should-not-save",
  "source": "resume",
  "transcript_path": "/tmp/test.jsonl",
  "permission_mode": "default",
  "hook_event_name": "SessionStart",
  "cwd": "/tmp"
}
EOF
)

echo "$RESUME_JSON" | "$HOOK_PATH"

if [[ -f "${STATE_DIR}/${TEST_SESSION}.parent_session" ]]; then
    echo "❌ FAIL: Hook saved session ID on 'resume' (should only save on 'startup')"
    exit 1
else
    echo "✅ PASS: Hook correctly ignores 'resume' events"
fi
echo ""

# Test 6: Test debug logging
echo "Test 6: Testing debug logging..."

rm -f "${STATE_DIR}/${TEST_SESSION}.parent_session"
rm -f "${STATE_DIR}/${TEST_SESSION}.debug.log"

export BUNSHIN_DEBUG=1
echo "$TEST_JSON" | "$HOOK_PATH"

if [[ -f "${STATE_DIR}/${TEST_SESSION}.debug.log" ]]; then
    if grep -q "Captured session ID instantly" "${STATE_DIR}/${TEST_SESSION}.debug.log"; then
        echo "✅ PASS: Debug logging works"
    else
        echo "❌ FAIL: Debug log exists but doesn't contain expected message"
        exit 1
    fi
else
    echo "❌ FAIL: Debug log not created when BUNSHIN_DEBUG=1"
    exit 1
fi
echo ""

# Cleanup
echo "Cleaning up test files..."
rm -f "${STATE_DIR}/${TEST_SESSION}.parent_session"
rm -f "${STATE_DIR}/${TEST_SESSION}.debug.log"

echo ""
echo "===================================="
echo "✅ All tests passed!"
echo ""
echo "Summary:"
echo "  ✅ Hook installed correctly"
echo "  ✅ Hook configured in Claude settings"
echo "  ✅ Shebang is correct"
echo "  ✅ Hook captures session IDs on startup"
echo "  ✅ Hook ignores resume events"
echo "  ✅ Debug logging works"
echo ""
echo "The SessionStart hook is working correctly! 🎉"
