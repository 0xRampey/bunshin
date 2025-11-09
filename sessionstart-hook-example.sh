#!/bin/bash
# SessionStart hook for bunshin - captures session ID immediately
set -euo pipefail

# Read JSON from stdin
INPUT=$(cat)

# Extract session_id from JSON
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id')
SOURCE=$(echo "$INPUT" | jq -r '.source')

# Only save on startup (not resume/clear/compact)
if [[ "$SOURCE" == "startup" ]]; then
    # Get Zellij session name from environment
    ZELLIJ_SESSION="${ZELLIJ_SESSION_NAME:-default}"
    STATE_DIR="${HOME}/.bunshin/state"
    mkdir -p "${STATE_DIR}"

    # Save the session ID immediately!
    echo "${SESSION_ID}" > "${STATE_DIR}/${ZELLIJ_SESSION}.parent_session"

    # Optional: Log for debugging
    if [[ "${BUNSHIN_DEBUG:-0}" == "1" ]]; then
        echo "[SessionStart] Saved session ID: ${SESSION_ID}" >> "${STATE_DIR}/${ZELLIJ_SESSION}.debug.log"
    fi
fi

# Return immediately (non-blocking)
exit 0
