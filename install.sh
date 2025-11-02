#!/bin/bash
# Bunshin Installation Script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔨 Bunshin Installation Script${NC}"
echo ""

# Check if running from the correct directory
if [ ! -f "Cargo.toml" ] || [ ! -f "target/release/bunshin" ]; then
    echo -e "${RED}❌ Error: Please run this script from the bunshin project directory${NC}"
    echo -e "${YELLOW}💡 Make sure you've built the release binary first:${NC}"
    echo "   cargo build --release"
    exit 1
fi

# Determine install location
INSTALL_DIR="${HOME}/.local/bin"

# Create install directory if it doesn't exist
if [ ! -d "$INSTALL_DIR" ]; then
    echo -e "${YELLOW}📁 Creating directory: $INSTALL_DIR${NC}"
    mkdir -p "$INSTALL_DIR"
fi

# Copy binary
echo -e "${GREEN}📦 Installing bunshin to $INSTALL_DIR${NC}"
cp target/release/bunshin "$INSTALL_DIR/bunshin"
chmod +x "$INSTALL_DIR/bunshin"

# Check if install directory is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo -e "${YELLOW}⚠️  Warning: $INSTALL_DIR is not in your PATH${NC}"
    echo ""
    echo "To use bunshin from anywhere, add this line to your shell config:"
    echo ""

    # Detect shell
    if [ -n "$ZSH_VERSION" ]; then
        SHELL_CONFIG="~/.zshrc"
    elif [ -n "$BASH_VERSION" ]; then
        SHELL_CONFIG="~/.bashrc"
    else
        SHELL_CONFIG="~/.profile"
    fi

    echo -e "${BLUE}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
    echo ""
    echo "Add it to: $SHELL_CONFIG"
    echo "Then run: source $SHELL_CONFIG"
else
    echo -e "${GREEN}✅ $INSTALL_DIR is already in your PATH${NC}"
fi

# Verify installation
echo ""
if command -v bunshin &> /dev/null; then
    INSTALLED_VERSION=$("$INSTALL_DIR/bunshin" --help 2>&1 | head -1 || echo "bunshin")
    echo -e "${GREEN}✅ Installation successful!${NC}"
    echo ""
    echo -e "${BLUE}📋 Installed to: $INSTALL_DIR/bunshin${NC}"
    echo -e "${BLUE}🏃 Try it: bunshin --help${NC}"
else
    echo -e "${YELLOW}⚠️  Installation complete, but 'bunshin' command not found in PATH${NC}"
    echo "You may need to:"
    echo "  1. Add $INSTALL_DIR to your PATH (see instructions above)"
    echo "  2. Restart your terminal"
    echo "  3. Or run directly: $INSTALL_DIR/bunshin"
fi

echo ""
echo -e "${GREEN}🎉 Ready to use Bunshin!${NC}"
echo ""
echo "Quick start:"
echo "  cd /path/to/your/git/repo"
echo "  bunshin              # Auto-creates session with Claude Code"
echo ""
