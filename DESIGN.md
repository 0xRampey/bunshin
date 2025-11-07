# Bunshin Architecture & Design Decisions

## Overview

Bunshin is a standalone binary that packages a Zellij plugin (WASM) and configuration files, providing a one-command install experience for Claude Code orchestration.

## Key Architectural Decisions

### 1. Why Extract Files to `~/.bunshin/` Instead of Running Embedded?

**Decision**: Extract plugin and config files to `~/.bunshin/` on first run, rather than using temporary files or keeping everything in-memory.

**Rationale**:

Zellij's CLI requires **file paths** for plugins and configuration:
```bash
zellij --config /path/to/config.kdl --layout /path/to/layout.kdl
```

Zellij cannot accept:
- Configs from stdin
- Embedded data passed directly
- In-memory content

**Alternatives Considered**:

1. **Temp files on every run** (`/tmp/bunshin-XXX/`)
   - ❌ Slower (re-extract on every launch)
   - ❌ Users can't customize configs
   - ❌ Pollutes temp directories
   - ✅ No persistent state

2. **Bundle Zellij itself** (fork Zellij to accept embedded configs)
   - ❌ Huge maintenance burden
   - ❌ Much larger binary
   - ❌ Version conflicts with system Zellij
   - ❌ Doesn't respect Zellij updates

3. **Current approach** (extract once to `~/.bunshin/`)
   - ✅ Fast after first run (no re-extraction)
   - ✅ Users can customize configs by editing `~/.bunshin/config/`
   - ✅ Persistent across runs
   - ✅ Clean separation from system Zellij
   - ✅ Minimal disk space (~1.3MB)
   - ⚠️ Creates persistent directory (but this is a feature, not a bug)

**Conclusion**: The one-time extraction to `~/.bunshin/` is the optimal trade-off for UX, performance, and maintainability.

### 2. Binary Structure: Workspace with Separated Plugin

**Decision**: Use a Cargo workspace with the plugin excluded from default members.

**Structure**:
```
bunshin/
├── Cargo.toml           # Workspace (members = ["cli"], exclude = ["plugin"])
├── cli/                 # Binary crate
│   ├── build.rs         # Compiles plugin to WASM, embeds it
│   └── src/main.rs      # CLI logic
└── plugin/              # Plugin crate (standalone, not in workspace)
    ├── Cargo.toml       # Standalone manifest
    └── src/lib.rs       # Zellij plugin (compiled to WASM)
```

**Rationale**:
- Plugin **must** be compiled to `wasm32-wasip1` target
- Workspace tries to build all members for the native target by default
- Excluding plugin prevents native build errors (`_host_run_plugin_command` symbol not found)
- `build.rs` explicitly compiles plugin with `--target wasm32-wasip1`

### 3. Embedded Assets via `include_bytes!`

**Decision**: Embed the WASM plugin directly in the CLI binary at compile time.

**Implementation**:
```rust
const PLUGIN_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bunshin.wasm"));
```

**Benefits**:
- Single binary distribution (no separate plugin file)
- No network calls during installation
- Guaranteed version compatibility (plugin matches CLI)
- Works offline

**Build Process**:
1. `build.rs` runs during CLI compilation
2. Compiles `plugin/` to WASM (`cargo build --target wasm32-wasip1`)
3. Copies WASM to `$OUT_DIR/bunshin.wasm`
4. `include_bytes!` embeds it in the binary
5. First run extracts to `~/.bunshin/plugins/`

### 4. Automatic Setup (No Flags)

**Decision**: Setup runs automatically on every launch, but only extracts if files are missing.

**Rationale**:
- No user intervention required
- Idempotent (safe to run multiple times)
- Fast path when files exist (just checks existence)
- Self-healing (re-extracts if users delete files)

**Previous design** had `--setup` flag, but this was removed because:
- Extra flag adds cognitive load
- Users shouldn't need to know about setup
- Automatic is simpler and more reliable

### 5. Zellij as External Dependency

**Decision**: Require users to install Zellij separately (or check PATH and prompt).

**Alternatives Considered**:
1. **Bundle Zellij binary** in Bunshin
   - ❌ 10-15MB binary size increase
   - ❌ Separate binaries for each platform
   - ❌ Update lag (need to update Bunshin for Zellij fixes)

2. **Auto-install Zellij** (download on first run)
   - ❌ Network dependency
   - ❌ Security concerns (downloading executables)
   - ❌ Platform detection complexity

3. **Current: Check and prompt** (recommended in docs)
   - ✅ Lightweight
   - ✅ Users control their Zellij version
   - ✅ Respects system package managers
   - ⚠️ Requires manual install step

**Conclusion**: External dependency is cleanest. Most users installing via `cargo install` likely already have Zellij or can easily install it.

## File Layout

### Installed Files
```
~/.bunshin/
├── plugins/
│   └── bunshin.wasm              # Session manager plugin (1.3MB)
└── config/
    ├── config.kdl                # Keybindings (Ctrl+b prefix)
    └── layout.kdl                # Auto-start Claude layout
```

### User Customization
Users can edit files in `~/.bunshin/config/` to customize:
- Keybindings (`config.kdl`)
- Auto-start layout (`layout.kdl`)
- Claude command path

Changes persist across Bunshin updates (files not overwritten if they exist).

## Build Requirements

### For End Users - Pre-built Binaries (Recommended)
- **No requirements!** Download and run
- Binaries from GitHub Releases are completely standalone
- WASM plugin already embedded

### For Source Installation (cargo install)
- Rust toolchain with `wasm32-wasip1` target:
  ```bash
  rustup target add wasm32-wasip1
  cargo install bunshin
  ```
- Plugin builds during installation
- This is standard for Rust packages with special targets

### For Development
- Same as cargo install: Rust + `wasm32-wasip1` target
- Plugin rebuilds on every change

### Build Process

**Development builds:**
1. `build.rs` compiles plugin to WASM on-the-fly
2. Embeds WASM in binary

**Release builds (CI):**
1. Build WASM once → save to `plugin/prebuilt/bunshin.wasm`
2. Build binaries → download pre-built WASM
3. `build.rs` detects pre-built WASM and uses it (no compilation needed)
4. Users don't need `wasm32-wasip1` target installed

## Future Considerations

### Potential Improvements
1. **Config migration**: Detect version changes and offer to update configs
2. **Plugin updates**: Check `~/.bunshin/` version vs embedded version
3. **Zellij auto-install**: Optional feature flag to bundle/download Zellij
4. **Multiple profiles**: Support different layouts (e.g., `bunshin --profile minimal`)

### Non-Goals
- Bundling Zellij (too heavy, maintenance burden)
- Supporting non-Zellij terminal multiplexers
- Running without file extraction (Zellij limitation)

## Performance Characteristics

- **First run**: ~2 seconds (extract files, check Zellij)
- **Subsequent runs**: <100ms (existence check only)
- **Binary size**: ~1.5MB (includes embedded 1.3MB WASM)
- **Disk usage**: ~2.6MB (`~/.bunshin/` + binary)

## Security Considerations

- WASM plugin runs in Zellij's sandboxed WASI runtime
- No network access by default
- No file writes outside `~/.bunshin/`
- Zellij permissions model enforced
