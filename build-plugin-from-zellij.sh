#!/bin/bash
set -e

# Temporary script to build plugin using Zellij workspace
# This is needed because zellij-tile 0.44.0 is not yet on crates.io
# Once 0.44.0 is published, we can remove this workaround

ZELLIJ_REPO="/Users/prudhvirampey/Documents/zellij"
BUNSHIN_PLUGIN_DIR="$ZELLIJ_REPO/default-plugins/bunshin"

echo "🔧 Building bunshin plugin using Zellij workspace..."

# Ensure .cargo/config.toml exists
mkdir -p "$BUNSHIN_PLUGIN_DIR/.cargo"
echo '[build]
target = "wasm32-wasip1"' > "$BUNSHIN_PLUGIN_DIR/.cargo/config.toml"

# Build the plugin
cd "$BUNSHIN_PLUGIN_DIR"
cargo build --release

# Copy to our standalone repo
WASM_SRC="$ZELLIJ_REPO/target/wasm32-wasip1/release/bunshin.wasm"
WASM_DEST="$(dirname "$0")/plugin/target/wasm32-wasip1/release/bunshin.wasm"

mkdir -p "$(dirname "$WASM_DEST")"
cp "$WASM_SRC" "$WASM_DEST"

echo "✅ Plugin built and copied to: $WASM_DEST"
echo ""
echo "Now rebuild the CLI:"
echo "  cargo build --release"
