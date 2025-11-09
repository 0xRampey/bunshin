#!/bin/bash
# Claude conversation forking wrapper for bunshin
# Simplified version - no pane counting needed!

set -euo pipefail

# State directory for tracking session IDs
STATE_DIR="${HOME}/.bunshin/state"
mkdir -p "${STATE_DIR}"

# Get the current Zellij session name
ZELLIJ_SESSION="${ZELLIJ_SESSION_NAME:-default}"
SESSION_STATE_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.parent_session"
DEBUG_LOG="${STATE_DIR}/${ZELLIJ_SESSION}.debug.log"

# Enable debug logging if BUNSHIN_DEBUG is set
DEBUG="${BUNSHIN_DEBUG:-0}"

# Debug logging function
log_debug() {
    if [[ "${DEBUG}" == "1" ]]; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${DEBUG_LOG}"
    fi
}

# Function to get the parent session ID
get_parent_session() {
    if [[ -f "${SESSION_STATE_FILE}" ]] && [[ -s "${SESSION_STATE_FILE}" ]]; then
        local session_id=$(cat "${SESSION_STATE_FILE}")
        log_debug "Found parent session: ${session_id}"
        echo "${session_id}"
    else
        log_debug "No parent session file found or file is empty"
        echo ""
    fi
}

# Main logic
main() {
    log_debug "=== Starting claude-fork wrapper ==="
    log_debug "PWD: $(pwd)"
    log_debug "ZELLIJ_SESSION: ${ZELLIJ_SESSION}"

    # Check if parent session exists
    local parent_session
    parent_session=$(get_parent_session)

    if [[ -z "${parent_session}" ]]; then
        # No parent session → This is the first pane
        echo "🌱 Launching first Claude pane in this session..."
        echo "   (SessionStart hook will capture session ID instantly)"
        echo "   (Subsequent tabs will fork from this conversation)"
        echo ""
        log_debug "No parent session - this is the first pane"
        log_debug "SessionStart hook will save session ID to: ${SESSION_STATE_FILE}"

        # Launch Claude normally
        # SessionStart hook will capture the session ID and save it
        exec claude "$@"
    else
        # Parent session exists → Fork from it
        log_debug "Parent session exists - attempting to fork"

        # Verify the session file actually exists
        local session_pattern="${HOME}/.claude/projects/*/${parent_session}.jsonl"
        if ls ${session_pattern} 1> /dev/null 2>&1; then
            # Fork the conversation
            echo "🍴 Forking conversation from session: ${parent_session}"
            echo "   (Exploring a different path from the same starting point)"
            echo ""
            log_debug "Forking with: claude --resume ${parent_session}"

            # --resume creates a NEW session file (a fork), not shared state
            exec claude --resume "${parent_session}" "$@"
        else
            log_debug "Session file not found for: ${parent_session}"
            echo "⚠️  Session file not found: ${parent_session}"
            echo "   The parent session might have been deleted."
            echo "   Launching new conversation instead..."
            echo ""
            exec claude "$@"
        fi
    fi
}

# Run main function
main "$@"
