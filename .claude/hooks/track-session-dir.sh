#!/bin/bash
# Bunshin SessionStart Hook: Track working directory for each Zellij session

# Ensure the bunshin directory exists
mkdir -p ~/.bunshin

# Get the session directories file
SESSION_DIRS_FILE="$HOME/.bunshin/session-dirs.json"

# Initialize the file if it doesn't exist
if [ ! -f "$SESSION_DIRS_FILE" ]; then
    echo '{}' > "$SESSION_DIRS_FILE"
fi

# Get current session name and project directory
ZELLIJ_SESSION="${ZELLIJ_SESSION_NAME:-unknown}"
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"

# Update the JSON file with the session -> directory mapping
# Use jq if available, otherwise use a simple sed approach
if command -v jq &> /dev/null; then
    # Use jq for robust JSON manipulation
    jq --arg session "$ZELLIJ_SESSION" --arg dir "$PROJECT_DIR" \
        '.[$session] = $dir' "$SESSION_DIRS_FILE" > "${SESSION_DIRS_FILE}.tmp" && \
        mv "${SESSION_DIRS_FILE}.tmp" "$SESSION_DIRS_FILE"
else
    # Fallback: simple approach without jq (less robust but works)
    # Read existing content, remove the session if it exists, add new entry
    python3 -c "
import json
import sys
try:
    with open('$SESSION_DIRS_FILE', 'r') as f:
        data = json.load(f)
except:
    data = {}
data['$ZELLIJ_SESSION'] = '$PROJECT_DIR'
with open('$SESSION_DIRS_FILE', 'w') as f:
    json.dump(data, f, indent=2)
"
fi

# Return success to allow the session to start
exit 0
