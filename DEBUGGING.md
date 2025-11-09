# Debugging Conversation Forking in Bunshin

If Ctrl+b c creates a new Claude conversation instead of forking, follow these steps:

## Step 1: Clean Install

```bash
cd /home/user/bunshin
cargo install --path cli --force

# Clean up old state
rm -rf ~/.bunshin/state/*
```

## Step 2: Enable Debug Mode

```bash
export BUNSHIN_DEBUG=1
bunshin
```

## Step 3: Test in First Pane

In the first pane that opens:

1. Send a message to Claude (anything):
   ```
   hello
   ```

2. Wait for Claude to respond

3. Check if session was captured:
   ```bash
   # In another terminal (not in bunshin):
   cat ~/.bunshin/state/$ZELLIJ_SESSION_NAME.parent_session
   ```

   **Expected**: You should see a UUID like `abc123-def456-...`

   **If empty/missing**: The SessionStart hook isn't working!

## Step 4: Test Fork in Second Pane

1. Press `Ctrl+b` then `c` to create a new tab

2. You should see this message:
   ```
   🍴 Forking conversation from session: <uuid>
   ```

3. In the new tab, ask Claude:
   ```
   What was my last message?
   ```

   **Expected**: Claude should remember your "hello" from the first pane

   **If not**: Forking didn't work

## Step 5: Check Debug Logs

```bash
# In another terminal:
cat ~/.bunshin/state/$ZELLIJ_SESSION_NAME.debug.log
```

Look for:
- `[SessionStart Hook] Captured session ID instantly: <uuid>` ← Hook working
- `Forking with: claude --resume <uuid>` ← Fork wrapper working

## Common Issues

### Issue 1: No parent_session file created

**Symptom**: `~/.bunshin/state/*.parent_session` doesn't exist after sending message

**Cause**: SessionStart hook isn't being triggered by Claude

**Fix**:
```bash
# Check if hook is actually configured:
cat ~/.claude/settings.json | jq '.hooks.SessionStart'

# Should show:
# [
#   {
#     "hooks": [
#       {
#         "command": "/root/.bunshin/bin/bunshin-session-capture",
#         "type": "command"
#       }
#     ],
#     "matcher": ""
#   }
# ]

# If missing, reconfigure:
rm ~/.claude/settings.json
bunshin --help  # Will recreate settings
```

### Issue 2: Hook exists but session not captured

**Symptom**: Debug log shows no `[SessionStart Hook]` entries

**Possible causes**:
1. Hook isn't executable: `chmod +x ~/.bunshin/bin/bunshin-session-capture`
2. Shebang is broken: `head -1 ~/.bunshin/bin/bunshin-session-capture` should be `#!/bin/bash`
3. jq not installed: `which jq` (hook needs jq to parse JSON)

**Fix**:
```bash
# Reinstall hook:
rm ~/.bunshin/bin/bunshin-session-capture
bunshin --help

# Verify it works manually:
export ZELLIJ_SESSION_NAME="test"
echo '{"session_id":"test-123","source":"startup"}' | ~/.bunshin/bin/bunshin-session-capture
cat ~/.bunshin/state/test.parent_session
# Should show: test-123
```

### Issue 3: Old sessions being grabbed

**Symptom**: Fork uses wrong/old session ID

**Fix**:
```bash
# Clear all old state:
rm -rf ~/.bunshin/state/*

# Start fresh bunshin session
bunshin
```

### Issue 4: Second tab says "Could not find parent session"

**Symptom**: Second tab shows warning instead of fork message

**Possible causes**:
1. Pressed Ctrl+b c too fast (before first pane captured session)
2. Parent session file got deleted
3. Session ID in parent_session file is invalid

**Fix**:
1. Wait for Claude to fully start in first pane before forking
2. Send a message and wait for response
3. Then press Ctrl+b c

## Manual Test: Does the Hook Work?

Test the SessionStart hook independently:

```bash
# Create test JSON (this is what Claude sends to hooks):
cat > /tmp/test-hook-input.json << 'EOF'
{
  "session_id": "manual-test-session-id",
  "source": "startup",
  "transcript_path": "/tmp/test.jsonl",
  "permission_mode": "default",
  "hook_event_name": "SessionStart",
  "cwd": "/tmp"
}
EOF

# Run the hook:
export ZELLIJ_SESSION_NAME="hook-test"
cat /tmp/test-hook-input.json | ~/.bunshin/bin/bunshin-session-capture

# Check if it worked:
cat ~/.bunshin/state/hook-test.parent_session
# Should show: manual-test-session-id
```

If this works, the hook is fine - the issue is Claude not triggering it.

## Verify Your Setup

Run the diagnostic:
```bash
./diagnose-fork-issue.sh
```

All checks should be ✅

## Still Not Working?

If everything looks good but forking still doesn't work, the issue is likely:

1. **Claude isn't triggering SessionStart hooks at all**
   - Check Claude version: `claude --version`
   - SessionStart hooks might not work in all Claude versions
   - Try: Send output to me along with your Claude version

2. **Timing issue**
   - Hook is too slow
   - Try adding longer wait in fork wrapper (edit `~/.bunshin/bin/claude-fork`, change `max_wait=5` to `max_wait=10`)

3. **Zellij session name issue**
   - Check: `echo $ZELLIJ_SESSION_NAME` inside bunshin
   - State files use this name, if it's wrong/changing, won't work

## Get Help

Share these outputs:
```bash
# 1. Diagnostic output:
./diagnose-fork-issue.sh > ~/bunshin-diagnostic.txt

# 2. Debug log from actual session:
cat ~/.bunshin/state/*.debug.log > ~/bunshin-debug.txt

# 3. Hook test:
export ZELLIJ_SESSION_NAME="test"
echo '{"session_id":"test-123","source":"startup"}' | ~/.bunshin/bin/bunshin-session-capture
cat ~/.bunshin/state/test.parent_session > ~/hook-test.txt

# 4. Claude version:
claude --version > ~/claude-version.txt
```

Share these files for debugging.
