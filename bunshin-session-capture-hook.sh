#!/bin/bash
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
