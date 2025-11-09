#!/bin/bash
# Claude conversation forking wrapper for bunshin
# This script manages conversation forking when opening new panes in a bunshin session

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
START_TIME_FILE="${STATE_DIR}/${ZELLIJ_SESSION}.start_time"

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

# Function to get Claude session created AFTER a specific timestamp
get_claude_session_after_timestamp() {
    local start_timestamp="$1"
    local cwd=$(pwd)
    # Encode the path for Claude's storage format
    local encoded_path=$(echo "${cwd}" | sed 's/\//-/g')
    local project_dir="${HOME}/.claude/projects/${encoded_path}"

    log_debug "Looking for sessions created after timestamp ${start_timestamp} in: ${project_dir}"

    if [[ ! -d "${project_dir}" ]]; then
        log_debug "Project directory not found: ${project_dir}"
        # Fallback to searching all projects
        project_dir="${HOME}/.claude/projects"
    fi

    if [[ ! -d "${project_dir}" ]]; then
        log_debug "No Claude projects directory found"
        echo ""
        return
    fi

    # Find .jsonl files (excluding agent-*) created/modified after our start time
    local recent_file=""
    while IFS= read -r -d '' file; do
        local file_mtime=$(stat -c '%Y' "$file" 2>/dev/null || echo "0")
        if [[ "$file_mtime" -gt "$start_timestamp" ]]; then
            recent_file="$file"
            log_debug "Found file: $file (mtime: $file_mtime > start: $start_timestamp)"
            break
        fi
    done < <(find "${project_dir}" -type f -name "*.jsonl" ! -name "agent-*.jsonl" -print0 2>/dev/null | xargs -0 ls -t 2>/dev/null)

    if [[ -n "${recent_file}" ]]; then
        local session_id=$(basename "${recent_file}" .jsonl)
        log_debug "Found session created after start time: ${session_id}"
        echo "${session_id}"
    else
        log_debug "No session files found created after timestamp ${start_timestamp}"
        echo ""
    fi
}

# Function to save the parent session ID
save_parent_session() {
    local session_id="$1"
    log_debug "Saving parent session: ${session_id}"
    echo "${session_id}" > "${SESSION_STATE_FILE}"
}

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

    # Get parent session if it exists
    local parent_session
    parent_session=$(get_parent_session)

    if [[ "${pane_num}" -eq 1 ]]; then
        # This is the first pane - launch Claude normally
        echo "🌱 Launching first Claude pane in this session..."
        echo "   (Subsequent panes will fork from this conversation)"
        echo ""
        log_debug "First pane - launching Claude normally"

        # Record the start time BEFORE launching Claude
        local start_time=$(date +%s)
        echo "${start_time}" > "${START_TIME_FILE}"
        log_debug "Recorded start time: ${start_time}"

        # Launch Claude normally, but save its session ID for forking
        # We'll capture the session ID after Claude initializes
        (
            # Wait for Claude to initialize and create its session file
            sleep 10

            # Get the session created after our start time
            local session_id
            session_id=$(get_claude_session_after_timestamp "${start_time}")

            if [[ -n "${session_id}" ]]; then
                save_parent_session "${session_id}"
                log_debug "Background process: Saved parent session ${session_id}"
            else
                log_debug "Background process: No new session ID found after start time ${start_time}"
            fi
        ) >/dev/null 2>&1 &

        exec claude "$@"
    else
        # This is a subsequent pane - try to fork the conversation
        log_debug "Subsequent pane (#${pane_num}) - attempting to fork"

        # Wait a bit for the first pane to save the session ID
        local max_wait=20
        local waited=0
        while [[ -z "${parent_session}" ]] && [[ $waited -lt $max_wait ]]; do
            sleep 1
            waited=$((waited + 1))
            parent_session=$(get_parent_session)
            log_debug "Waiting for parent session... (${waited}/${max_wait})"
        done

        if [[ -z "${parent_session}" ]] && [[ -f "${START_TIME_FILE}" ]]; then
            # Parent session not found yet, try to get it directly using start time
            local start_time=$(cat "${START_TIME_FILE}")
            log_debug "Parent session still not found, trying direct lookup after timestamp ${start_time}"
            parent_session=$(get_claude_session_after_timestamp "${start_time}")

            if [[ -n "${parent_session}" ]]; then
                save_parent_session "${parent_session}"
                log_debug "Found and saved parent session via direct lookup: ${parent_session}"
            fi
        fi

        if [[ -n "${parent_session}" ]]; then
            # Verify the session file exists
            local session_pattern="${HOME}/.claude/projects/*/${parent_session}.jsonl"
            if ls ${session_pattern} 1> /dev/null 2>&1; then
                # Fork the conversation
                echo "🍴 Forking conversation from session: ${parent_session}"
                echo "   (Pane #${pane_num} - exploring a different path)"
                echo ""
                log_debug "Forking with: claude --resume ${parent_session}"

                exec claude --resume "${parent_session}" "$@"
            else
                log_debug "Session file not found for: ${parent_session}"
                echo "⚠️  Session file not found, launching new conversation..."
                exec claude "$@"
            fi
        else
            # Fallback: launch normally if we couldn't find a parent session
            log_debug "No parent session found after all attempts, launching new conversation"
            echo "⚠️  Could not find parent session, launching new conversation..."
            echo "   Tip: Wait a few seconds after the first pane starts before forking"
            echo ""
            exec claude "$@"
        fi
    fi
}

# Run main function
main "$@"
