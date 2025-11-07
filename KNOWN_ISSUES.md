# Known Issues

## Plugin WASM Build Differences

**Issue:** The WASM plugin built in our standalone repo differs from the one built in the Zellij workspace (1.2M vs 1.0M), causing a `_start` function export error when loading.

**Root Cause Investigation:**
- Our plugin uses `zellij-tile = "0.43.1"` from crates.io
- Original uses `zellij-tile = { path = "../../zellij-tile" }` from Zellij workspace
- Possible dependency resolution differences
- Workspace-level build configurations we might be missing

**Current Workaround:**
Use the pre-built WASM from the Zellij repo during development:

```bash
# From the Zellij repo directory
cd /path/to/zellij
cargo build --release --target wasm32-wasip1 --manifest-path default-plugins/bunshin/Cargo.toml

# Copy to our build
cp target/wasm32-wasip1/release/bunshin.wasm /path/to/bunshin/plugin/target/wasm32-wasip1/release/

# Rebuild bunshin CLI
cd /path/to/bunshin
cargo build --release
```

**TODO:**
- [ ] Investigate exact build differences (objdump -x comparison)
- [ ] Check if newer zellij-tile versions on crates.io fix the issue
- [ ] Consider bundling pre-built WASM in releases instead of building during cargo install
- [ ] Document minimal reproducible example for upstream bug report if needed

**Status:** Non-blocking for releases (can use pre-built WASM), blocks local development workflow.
