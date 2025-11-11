#!/bin/bash
set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🥷 Bunshin Dev Build${NC}"

# Build the project
echo -e "${BLUE}Building...${NC}"
cargo build --release

# Force plugin extraction by removing old plugins
echo -e "${BLUE}Extracting plugins...${NC}"
rm -f ~/.bunshin/plugins/*.wasm

echo -e "${GREEN}✓ Build complete! Launching Bunshin...${NC}"
echo ""

# Launch with the fresh build
exec ./target/release/bunshin "$@"
