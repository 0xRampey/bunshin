#!/bin/bash
# Manual test for conversation forking
# This simulates what happens when you press Ctrl+b c in bunshin

echo "🧪 Manual Fork Test"
echo "==================="
echo ""
echo "This will simulate creating a second pane in bunshin."
echo "We'll test if the fork wrapper can read the parent session."
echo ""

# Setup
export BUNSHIN_DEBUG=1
export ZELLIJ_SESSION_NAME="manual-test-fork"
STATE_DIR="${HOME}/.bunshin/state"
mkdir -p "$STATE_DIR"

echo "Step 1: Cleaning up any old test state..."
rm -f "${STATE_DIR}/manual-test-fork."*
echo "   ✅ Cleaned up"
echo ""

echo "Step 2: Simulating first pane launch..."
echo "   Running: ~/.bunshin/bin/claude-fork --help"
echo ""

# This simulates the first pane
~/.bunshin/bin/claude-fork --help &
FIRST_PID=$!

# Wait for it to start
sleep 2

echo ""
echo "Step 3: Checking if pane count was incremented..."
if [[ -f "${STATE_DIR}/manual-test-fork.pane_count" ]]; then
    COUNT=$(cat "${STATE_DIR}/manual-test-fork.pane_count")
    echo "   ✅ Pane count file exists: $COUNT"
else
    echo "   ❌ Pane count file NOT created!"
fi
echo ""

echo "Step 4: Manually creating a parent session (simulating SessionStart hook)..."
# Simulate what the SessionStart hook would do
FAKE_SESSION_ID="test-fake-session-12345"
echo "$FAKE_SESSION_ID" > "${STATE_DIR}/manual-test-fork.parent_session"
echo "   ✅ Created fake parent session: $FAKE_SESSION_ID"
echo ""

# Kill the first claude process
kill $FIRST_PID 2>/dev/null
wait $FIRST_PID 2>/dev/null

echo "Step 5: Simulating second pane launch (the fork)..."
echo "   Running: ~/.bunshin/bin/claude-fork --help"
echo ""

# This simulates the second pane - should try to fork
~/.bunshin/bin/claude-fork --help &
SECOND_PID=$!

sleep 3

echo ""
echo "Step 6: Checking debug logs..."
if [[ -f "${STATE_DIR}/manual-test-fork.debug.log" ]]; then
    echo "   📝 Debug log contents:"
    cat "${STATE_DIR}/manual-test-fork.debug.log" | sed 's/^/      /'
else
    echo "   ⚠️  No debug log found (maybe BUNSHIN_DEBUG didn't work)"
fi

# Kill the second process
kill $SECOND_PID 2>/dev/null
wait $SECOND_PID 2>/dev/null

echo ""
echo "Step 7: Checking final state..."
echo "   Files created:"
ls -lh "${STATE_DIR}/manual-test-fork."* 2>/dev/null | sed 's/^/      /'

echo ""
echo "==================="
echo "Analysis:"
echo ""

if grep -q "Forking with: claude --resume" "${STATE_DIR}/manual-test-fork.debug.log" 2>/dev/null; then
    echo "✅ Fork wrapper is working correctly!"
    echo "   It detected pane #2 and attempted to fork from: $FAKE_SESSION_ID"
    echo ""
    echo "📍 The issue might be:"
    echo "   1. SessionStart hook not being triggered by Claude"
    echo "   2. Real session ID not being captured"
    echo "   3. Timing issue in real Zellij session"
else
    echo "❌ Fork wrapper did NOT attempt to fork"
    echo ""
    echo "Check the debug log above for details."
fi

echo ""
echo "💡 Next steps:"
echo "   1. Start a REAL bunshin session: bunshin"
echo "   2. In first pane, send a message to Claude"
echo "   3. Wait for response"
echo "   4. Check: cat ~/.bunshin/state/\$ZELLIJ_SESSION_NAME.parent_session"
echo "   5. Press Ctrl+b c to create second tab"
echo "   6. Check: cat ~/.bunshin/state/\$ZELLIJ_SESSION_NAME.debug.log"

# Cleanup
rm -f "${STATE_DIR}/manual-test-fork."*
