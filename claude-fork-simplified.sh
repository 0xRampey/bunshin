#!/bin/bash
# Claude conversation forking wrapper for bunshin
# Works with SessionStart hook for instant session ID capture

set -euo pipefail

# State directory for tracking session IDs
STATE_DIR="${HOME}/.bunshin/state"
mkdir -p "${STATE_DIR}"

# Get the current Zellij session name to namespace our state files
ZELLIJ_SESSION="${ZELLIJ_SESSION_NAME:-default}"
SESSION_STATE_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.parent_session"
PANE_COUNT_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.pane_count"
LOCK_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.lock"
DEBUG_LOG="${STATE_DIR}/${ZELLIJ_SESSION}.debug.log"

# Enable debug logging if BUNSHIN_DEBUG is set
DEBUG="${BUNSHIN_DEBUG:-0}"

# Debug logging function
log_debug() {
    if [[ "${DEBUG}" == "1" ]]; then
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "${DEBUG_LOG}"
    fi
}

# Function to acquire lock
acquire_lock() {
    local timeout=10
    local count=0
    while [[ -f "${LOCK_FILE}" ]] && [[ $count -lt $timeout ]]; do
        sleep 0.5
        count=$((count + 1))
    done

    if [[ $count -ge $timeout ]]; then
        log_debug "Failed to acquire lock after ${timeout} seconds"
        return 1
    fi

    echo $$ > "${LOCK_FILE}"
    log_debug "Lock acquired by $$"
    return 0
}

# Function to release lock
release_lock() {
    rm -f "${LOCK_FILE}"
    log_debug "Lock released by $$"
}

# Ensure lock is released on exit
trap release_lock EXIT

# Function to get the parent session ID
get_parent_session() {
    if [[ -f "${SESSION_STATE_FILE}" ]]; then
        local session_id=$(cat "${SESSION_STATE_FILE}")
        log_debug "Retrieved parent session: ${session_id}"
        echo "${session_id}"
    else
        log_debug "No parent session file found"
        echo ""
    fi
}

# Function to increment pane count
increment_pane_count() {
    if ! acquire_lock; then
        echo "1"
        return
    fi

    local count=0
    if [[ -f "${PANE_COUNT_FILE}" ]]; then
        count=$(cat "${PANE_COUNT_FILE}")
    fi
    count=$((count + 1))
    echo "${count}" > "${PANE_COUNT_FILE}"
    log_debug "Incremented pane count to: ${count}"

    release_lock
    echo "${count}"
}

# Main logic
main() {
    log_debug "=== Starting claude-fork wrapper ==="
    log_debug "PWD: $(pwd)"
    log_debug "ZELLIJ_SESSION: ${ZELLIJ_SESSION}"

    # Increment the pane count
    local pane_num
    pane_num=$(increment_pane_count)

    log_debug "Current pane number: ${pane_num}"

    if [[ "${pane_num}" -eq 1 ]]; then
        # This is the first pane - launch Claude normally
        echo "🌱 Launching first Claude pane in this session..."
        echo "   (SessionStart hook will capture session ID instantly)"
        echo ""
        log_debug "First pane - launching Claude normally"
        log_debug "SessionStart hook will save session ID to: ${SESSION_STATE_FILE}"

        # No background process needed! SessionStart hook handles it instantly.
        exec claude "$@"
    else
        # This is a subsequent pane - fork the conversation
        log_debug "Subsequent pane (#${pane_num}) - attempting to fork"

        # Get parent session (should already be saved by SessionStart hook)
        local parent_session
        parent_session=$(get_parent_session)

        # Wait briefly if not found yet (hook should be instant though)
        if [[ -z "${parent_session}" ]]; then
            log_debug "Parent session not found, waiting briefly..."
            local waited=0
            while [[ -z "${parent_session}" ]] && [[ $waited -lt 5 ]]; do
                sleep 1
                waited=$((waited + 1))
                parent_session=$(get_parent_session)
                log_debug "Waiting for SessionStart hook... (${waited}/5)"
            done
        fi

        if [[ -n "${parent_session}" ]]; then
            # Verify the session file exists
            local session_pattern="${HOME}/.claude/projects/*/${parent_session}.jsonl"
            if ls ${session_pattern} 1> /dev/null 2>&1; then
                # Fork the conversation using --resume
                echo "🍴 Forking conversation from session: ${parent_session}"
                echo "   (Pane #${pane_num} - exploring a different path)"
                echo ""
                log_debug "Forking with: claude --resume ${parent_session}"

                # NOTE: --resume creates a NEW session file (a fork), not shared state
                # Each resumed session gets its own independent session file
                exec claude --resume "${parent_session}" "$@"
            else
                log_debug "Session file not found for: ${parent_session}"
                echo "⚠️  Session file not found, launching new conversation..."
                exec claude "$@"
            fi
        else
            # Fallback: launch normally if we couldn't find a parent session
            log_debug "No parent session found after waiting"
            echo "⚠️  Could not find parent session, launching new conversation..."
            echo "   Tip: Make sure SessionStart hook is configured in ~/.claude/settings.json"
            echo ""
            exec claude "$@"
        fi
    fi
}

# Run main function
main "$@"
