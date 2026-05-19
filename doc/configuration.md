# Configuration

All configuration is passed through the Zellij keybind `configuration` block.
No separate config file is needed.

## Full example with all defaults

```kdl
bind "S" {
    LaunchOrFocusPlugin "file:~/.config/zellij/plugins/zellij_flash.wasm" {
        floating true

        // Scrollback depth profiles, cycled with `g` inside the float.
        // Values: "viewport" (visible area only) or a number (scrollback line cap).
        // First profile is active on open.
        profiles "viewport,200,2000"

        // Float dimensions. Format: "WIDTHxHEIGHT".
        // Accepts percentages ("90%x85%") or absolute cell counts ("200x50").
        // Percentage widths are auto-centered horizontally.
        size "90%x85%"
    };
    SwitchToMode "locked"
}
```

## Keys reference

| Key | Default | Description |
|---|---|---|
| `profiles` | `"viewport,200,2000"` | Comma-separated depth profiles. `viewport` = visible area only; a positive integer = that many scrollback lines. At least one profile required. |
| `size` | _(Zellij default float size)_ | Float dimensions as `WIDTHxHEIGHT`. Percentages or absolute cell counts. Omit to use Zellij's default float placement. |

## Future configuration (not yet implemented)

The following options are planned but not yet available. This section will be
updated as each is added.

### Theme colors

All UI colors default to **Catppuccin Macchiato**. Future releases will allow
overriding individual roles via the configuration block:

```kdl
// Planned — not yet available
bind "S" {
    LaunchOrFocusPlugin "file:~/.config/zellij/plugins/zellij_flash.wasm" {
        floating true
        profiles "viewport,200,2000"
        size "90%x85%"

        // Color roles — hex strings, e.g. "#8aadf4"
        // Defaults shown are Catppuccin Macchiato values.
        color_sel_bg      "#8aadf4"  // selection background (Blue)
        color_sel_fg      "#24273a"  // selection foreground (Base)
        color_cursor_bg   "#cad3f5"  // cursor cell background (Text)
        color_cursor_fg   "#24273a"  // cursor cell foreground (Base)
        color_gutter_mark "#eed49f"  // cursor row gutter marker (Yellow)
        color_gutter_dim  "#6e738d"  // other gutter numbers (Overlay0)
        color_sel_label   "#8bd5ca"  // "SEL N lines" footer indicator (Teal)
        color_footer_dim  "#6e738d"  // footer status text (Overlay0)
        color_footer_key  "#b8c0e0"  // footer key hint labels (Subtext1)
    };
    SwitchToMode "locked"
}
```

### Key bindings

Custom key bindings inside the float are planned but not yet configurable.

### Jump label charset

The characters used for jump labels (`a-zA-Z`) are currently hardcoded.
