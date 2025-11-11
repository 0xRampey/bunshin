# Status-Bar Plugin Customization Guide

The `status-bar` plugin is a customizable Zellij status bar that displays keyboard shortcuts, mode information, and helpful tips. This guide shows how to modify it for your specific use case.

## Project Structure

```
status-bar/
├── src/
│   ├── main.rs              # Plugin entry point, state management
│   ├── first_line.rs        # Top status bar with mode shortcuts
│   ├── second_line.rs       # Bottom bar with detailed keybinds & status messages
│   ├── one_line_ui.rs       # Compact single-line alternative
│   └── tip/                 # Tips system (rotating helpful hints)
│       ├── mod.rs           # Tip system core
│       ├── cache.rs         # Persistent tip cache
│       ├── utils.rs         # Tip selection logic
│       └── data/            # Individual tip implementations
```

## Customization Areas

### 1. Custom Keybindings/Shortcuts

**Location:** `src/first_line.rs` and `src/second_line.rs`

These modules display the keybindings available in the current mode. To customize:

```rust
// In src/first_line.rs
// Modify the key_indicators() function to show your custom shortcuts
// Example: Add your custom command shortcuts
```

**Steps:**
1. Open `src/first_line.rs`
2. Look for the functions that format keybindings (e.g., `key_indicators()`, `keybinds()`)
3. Modify the displayed keys and their descriptions
4. The plugin will automatically format them based on available screen width

### 2. Custom Status Indicators

**Location:** `src/main.rs` and `src/second_line.rs`

The status bar displays various indicators like mode info, clipboard status, and fullscreen state. To add custom status:

```rust
// In src/main.rs
// Extend the State struct to include your custom status fields
struct State {
    tabs: Vec<TabInfo>,
    mode_info: ModeInfo,
    // ADD YOUR CUSTOM STATUS HERE
    custom_status: Option<String>,  // Example
}

// In src/main.rs update() method
// Handle events to update your custom status
```

**Steps:**
1. Add a new field to the `State` struct in `src/main.rs`
2. Handle the relevant events in the `update()` method
3. Render the custom status in the `render()` method
4. Use the `LinePart` struct to format output with proper width calculation

### 3. Customize Tips System

The tips system displays rotating helpful hints about Zellij features. There are three verbosity levels: short, medium, and full.

#### Option A: Modify Existing Tips

**Location:** `src/tip/data/*.rs`

Each file contains a tip implementation. To modify a tip:

```rust
// In src/tip/data/quicknav.rs
pub fn quicknav(mode_info: &ModeInfo) -> TipBody {
    TipBody {
        short: /* short version of tip */,
        medium: /* medium version of tip */,
        full: /* full version of tip */,
    }
}
```

#### Option B: Add New Custom Tips

1. Create a new file `src/tip/data/my_custom_tip.rs`:

```rust
use crate::LinePart;

pub fn my_custom_tip() -> LinePart {
    LinePart {
        part: "Your custom tip text here".to_string(),
        len: 24, // Character width (use unicode_width crate)
    }
}
```

2. Register your tip in `src/tip/data/mod.rs`:

```rust
mod my_custom_tip;

lazy_static! {
    pub static ref TIPS: HashMap<&'static str, TipBody> = {
        let mut map = HashMap::new();
        map.insert("my_custom_tip", my_custom_tip());
        // ... existing tips
        map
    };
}
```

#### Option C: Remove the Tips System

If you don't want tips:

1. In `src/main.rs`, comment out the tip-related code in the `render()` method
2. Remove the `tip` module usage

### 4. Change UI Layout Modes

The status-bar supports three layout modes:
- **Two-line** (default): Top bar (mode shortcuts) + Bottom bar (keybinds)
- **One-line**: Compact single-line alternative
- **Minimal**: Even more compact

**Location:** `src/main.rs`

Look for the `classic_ui` flag and conditional rendering logic in the `render()` method.

## Build and Test

### Build for Development

From the bunshin root directory:

```bash
cargo build --release --target wasm32-wasip1 --manifest-path status-bar/Cargo.toml
```

### Test with dev.kdl

```bash
zellij --layout ./dev.kdl
```

Once inside Zellij, use `Ctrl+b b` to load your custom status-bar plugin.

### Useful Development Commands

- Watch for changes and rebuild:
  ```bash
  cargo watch -x 'build --release --target wasm32-wasip1 --manifest-path status-bar/Cargo.toml'
  ```

## API Reference

### Core Structures

**`LinePart`**: Represents a styled text segment
```rust
pub struct LinePart {
    pub part: String,    // ANSI-styled text
    pub len: usize,      // Display width (unicode-aware)
}
```

**`ColoredElements`**: Styling configuration
```rust
pub struct ColoredElements {
    pub selected: SegmentStyle,
    pub unselected: SegmentStyle,
    pub disabled: SegmentStyle,
    // ... more color definitions
}
```

### Key Functions

- `color_elements()` - Generate styled segments based on palette
- `action_key()` - Map actions to keyboard keys
- `style_key_with_modifier()` - Format keys with modifiers
- `get_common_modifiers()` - Extract shared modifiers from keys

## Common Modifications

### Disable Certain Keybinds from Display

In `src/second_line.rs`, modify the `keybinds()` function to filter out specific keybindings.

### Change Color Scheme

In `src/main.rs`, modify the `color_elements()` function to use different palette colors.

### Add Custom Event Handling

In `src/main.rs`, extend the `update()` method to handle new event types by:
1. Adding events to the `subscribe()` list in `load()`
2. Handling them in the `update()` method

## Resources

- **Zellij Plugin Development:** https://zellij.dev/documentation/api
- **Unicode Width:** Used for accurate character width calculation
- **ANSI Term:** Color and styling for terminal output
- **Serde:** Serialization for persistent cache

## Notes

- The plugin communicates with Zellij through the `zellij-tile` API
- All rendering must account for terminal width constraints
- Unicode width calculations are important for proper alignment
- The tips system caches which tips have been shown to avoid repetition
