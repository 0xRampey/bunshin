# Status-Bar Plugin for Bunshin

A customizable status bar plugin for Zellij that displays keybindings, mode information, and helpful tips.

## Features

- **Mode-aware display**: Shows relevant keybindings for the current input mode
- **Responsive UI**: Automatically adapts to terminal width with progressive shortening
- **Helpful tips**: Rotating educational tips about Zellij features
- **Status messages**: Displays clipboard operations, fullscreen indicators, and floating pane status
- **Three UI modes**: Classic two-line, modern one-line, or minimal display
- **Persistent tip cache**: Tracks which tips have been shown across sessions

## Building the Plugin

### Prerequisites

- Rust 1.70+
- `wasm32-wasip1` target installed:
  ```bash
  rustup target add wasm32-wasip1
  ```

### Build Steps

From the `status-bar` directory:

```bash
# Development build
cargo build --release --target wasm32-wasip1

# The WASM file will be created at:
# target/wasm32-wasip1/release/status-bar.wasm
```

### Building from Zellij Workspace (Recommended)

Due to API compatibility differences, it's recommended to build from the official Zellij repository:

```bash
# Clone Zellij
git clone https://github.com/zellij-org/zellij.git
cd zellij

# Build the status-bar plugin
cargo build --release --target wasm32-wasip1 --manifest-path default-plugins/status-bar/Cargo.toml

# The WASM will be at: target/wasm32-wasip1/release/status-bar.wasm
```

## Integration with Bunshin

The status-bar plugin is automatically embedded in the Bunshin binary and extracted to `~/.bunshin/plugins/status-bar.wasm` on first run.

### Using the Plugin

In your Zellij config:

```kdl
layout {
    pane size=1 borderless=true {
        plugin location="tab-bar"
    }

    pane {
        // Your content here
    }

    pane size=2 borderless=true {
        plugin location="file:~/.bunshin/plugins/status-bar.wasm"
    }
}
```

## Customization

See [../STATUS_BAR_CUSTOMIZATION.md](../STATUS_BAR_CUSTOMIZATION.md) for detailed customization guides:

- **Custom keybindings**: Modify displayed shortcuts
- **Custom status indicators**: Add your own status information
- **Custom tips**: Add, remove, or modify helpful tips
- **UI layout**: Change how the status bar is displayed

## File Structure

```
├── Cargo.toml              # Dependencies and metadata
├── .cargo/
│   └── config.toml         # WASM build target configuration
├── src/
│   ├── main.rs             # Plugin entry point
│   ├── first_line.rs       # Top status bar (mode shortcuts)
│   ├── second_line.rs      # Bottom bar (keybinds & messages)
│   ├── one_line_ui.rs      # Compact single-line alternative
│   └── tip/                # Tips system
│       ├── mod.rs
│       ├── cache.rs        # Persistent tip cache
│       ├── consts.rs       # Configuration constants
│       ├── utils.rs        # Tip selection logic
│       └── data/           # Individual tip implementations
└── README.md               # This file
```

## Development Tips

### Testing During Development

Use the dev.kdl layout:

```bash
zellij --layout ./dev.kdl
```

Press `Ctrl+b b` to load the status-bar plugin in floating mode.

### Watch for Changes

```bash
cargo watch -x 'build --release --target wasm32-wasip1'
```

### Debug Output

Print debug information using:

```rust
println!("Debug message");
eprintln!("Error message");
```

These will appear in Zellij logs.

## Known Issues

### Compilation Errors

The plugin may not compile in the standalone Bunshin repository due to API differences between:
- `zellij-tile = "0.43.1"` (from crates.io, what Bunshin uses)
- `zellij-tile` (from Zellij workspace, what the plugin was built with)

**Solutions:**

1. **Use pre-built WASM**: Download from official Zellij releases or build in the Zellij workspace
2. **Build from Zellij**: Clone Zellij and build directly (see above)
3. **Contribute upstream**: Help improve Zellij's plugin API compatibility

## Customization Examples

### Add Git Status to the Bar

1. Add a custom field to State in `main.rs`
2. Subscribe to relevant events in `load()`
3. Update status in `update()` method
4. Render in `render()` method

### Change Color Scheme

Modify the `color_elements()` function in `main.rs` to use different palette colors.

### Remove Tips System

Comment out tip-related code in `main.rs` render method.

## Contributing

Found a bug or want to improve the plugin?

1. Check [../STATUS_BAR_CUSTOMIZATION.md](../STATUS_BAR_CUSTOMIZATION.md) for customization examples
2. Read the Zellij plugin documentation: https://zellij.dev/documentation/api
3. Submit issues and PRs to the main Bunshin repo

## License

MIT License - See [../LICENSE](../LICENSE) for details

## References

- [Zellij Documentation](https://zellij.dev/documentation/)
- [Zellij Plugin API](https://zellij.dev/documentation/api)
- [Original Status-Bar Plugin](https://github.com/zellij-org/zellij/tree/main/default-plugins/status-bar)
