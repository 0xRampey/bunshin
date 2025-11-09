#!/bin/bash
# Bunshin development build script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored message
log() {
    echo -e "${GREEN}==>${NC} $1"
}

error() {
    echo -e "${RED}Error:${NC} $1" >&2
    exit 1
}

warn() {
    echo -e "${YELLOW}Warning:${NC} $1"
}

info() {
    echo -e "${BLUE}Info:${NC} $1"
}

# Check for wasm32-wasip1 target
check_wasm_target() {
    if ! rustup target list --installed | grep -q "wasm32-wasip1"; then
        warn "wasm32-wasip1 target not installed"
        log "Installing wasm32-wasip1 target..."
        rustup target add wasm32-wasip1
    fi
}

# Build plugin only (fast rebuild for plugin dev)
build_plugin() {
    log "Building plugin WASM..."
    cd plugin
    cargo build --release --target wasm32-wasip1
    cd ..
    info "Plugin WASM: plugin/target/wasm32-wasip1/release/bunshin.wasm"
}

# Build CLI (includes plugin via build.rs)
build_cli() {
    log "Building CLI (will auto-build plugin)..."
    cd cli
    cargo build --release
    cd ..
    info "CLI binary: cli/target/release/bunshin"
}

# Full build (both plugin and CLI)
build_all() {
    log "Starting full build..."
    check_wasm_target
    build_cli
    log "Build complete!"
}

# Clean build artifacts
clean() {
    log "Cleaning build artifacts..."
    rm -rf plugin/target
    rm -rf cli/target
    rm -rf target
    log "Clean complete!"
}

# Run the built binary
run() {
    if [ ! -f "cli/target/release/bunshin" ]; then
        error "Binary not found. Run './dev.sh build' first."
    fi

    log "Running bunshin..."
    ./cli/target/release/bunshin "$@"
}

# Install to ~/.cargo/bin
install() {
    log "Installing bunshin to ~/.cargo/bin..."
    cd cli
    cargo install --path .
    cd ..
    log "Install complete! Run 'bunshin' to use."
}

# Run tests
test() {
    log "Running plugin tests..."
    cd plugin
    cargo test
    cd ..

    log "Running CLI tests..."
    cd cli
    cargo test
    cd ..
}

# Show help
show_help() {
    cat << EOF
${GREEN}Bunshin Development Script${NC}

${YELLOW}Usage:${NC}
  ./dev.sh [command]

${YELLOW}Commands:${NC}
  ${BLUE}build${NC}         Build CLI and plugin (default)
  ${BLUE}plugin${NC}        Build only the plugin WASM (fast iteration)
  ${BLUE}cli${NC}           Build only the CLI
  ${BLUE}clean${NC}         Remove all build artifacts
  ${BLUE}run${NC}           Run the built binary
  ${BLUE}install${NC}       Install to ~/.cargo/bin
  ${BLUE}test${NC}          Run all tests
  ${BLUE}help${NC}          Show this help message

${YELLOW}Examples:${NC}
  ./dev.sh build         # Full build
  ./dev.sh plugin        # Quick plugin rebuild
  ./dev.sh run           # Run bunshin
  ./dev.sh clean build   # Clean build from scratch

${YELLOW}Development Workflow:${NC}
  1. Edit plugin code:     ./dev.sh plugin    (fast rebuild)
  2. Edit CLI code:        ./dev.sh cli
  3. Full rebuild:         ./dev.sh build
  4. Test changes:         ./dev.sh run
  5. Install locally:      ./dev.sh install

${YELLOW}Project Structure:${NC}
  plugin/    - Zellij WASM plugin
  cli/       - CLI that embeds plugin
  .claude/   - SessionStart hook
EOF
}

# Main command dispatcher
case "${1:-build}" in
    build)
        build_all
        ;;
    plugin)
        check_wasm_target
        build_plugin
        ;;
    cli)
        build_cli
        ;;
    clean)
        clean
        ;;
    run)
        shift
        run "$@"
        ;;
    install)
        install
        ;;
    test)
        test
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        error "Unknown command: $1\nRun './dev.sh help' for usage."
        ;;
esac
