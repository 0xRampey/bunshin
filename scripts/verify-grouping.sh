#!/bin/bash
# Verification script for session grouping feature
# This script creates test sessions in different repos and verifies grouping

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}=== Bunshin Session Grouping Verification ===${NC}"
echo ""

# Check dependencies
check_dep() {
    if ! command -v $1 &> /dev/null; then
        echo -e "${RED}Error: $1 is not installed${NC}"
        echo "Install with: $2"
        exit 1
    fi
}

check_dep "zellij" "cargo install zellij"
check_dep "bunshin" "cargo install --path cli"

# Optional: asciinema for recording
RECORD=false
if command -v asciinema &> /dev/null; then
    RECORD=true
    echo -e "${GREEN}asciinema found - will record session${NC}"
fi

# Create temporary test directories (simulating different repos)
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

echo "Creating test repos in $TMPDIR..."

# Create fake git repos
for repo in "project-alpha" "project-beta" "bunshin"; do
    mkdir -p "$TMPDIR/$repo"
    cd "$TMPDIR/$repo"
    git init -q
    echo "# $repo" > README.md
    git add README.md
    git commit -q -m "Initial commit"
done

echo -e "${GREEN}Test repos created${NC}"
echo ""

# Instructions for manual verification
cat << 'EOF'
=== Manual Verification Steps ===

1. Start bunshin in each repo and create sessions:

   cd /tmp/test-repos/project-alpha
   bunshin  # Creates a session, press 'N' to create named session "alpha-main"

   cd /tmp/test-repos/project-beta
   bunshin  # Press 'N' to create "beta-dev"

   cd /tmp/test-repos/bunshin
   bunshin  # Press 'N' to create "bunshin-feature"

2. Open session manager (Ctrl+b s)

3. Verify sessions are grouped:

   ▼ bunshin (1)
       bunshin-feature    1 window   1 pane   1 client

   ▼ project-alpha (1)
       alpha-main         1 window   1 pane   1 client

   ▼ project-beta (1)
       beta-dev           1 window   1 pane   1 client

4. Verify navigation (j/k) works across groups

5. Verify new session creation associates with current repo

EOF

# If asciinema is available, offer to record
if [ "$RECORD" = true ]; then
    echo ""
    echo -e "${YELLOW}To record a terminal session:${NC}"
    echo "  asciinema rec --title 'Bunshin Session Grouping' grouping-demo.cast"
    echo ""
    echo "Then upload to asciinema.org or embed in PR"
fi

echo ""
echo -e "${GREEN}Verification script complete${NC}"
echo "Run manual steps above to verify the feature"
