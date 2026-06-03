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

        // Characters used as jump labels for the s word-jump.
        // Any printable non-whitespace chars; duplicates removed; order preserved.
        // Default: a-z then A-Z (52 labels). Shorten or restrict to taste.
        // Examples:
        //   labels "asdfjkl;"          -- home-row only (8 labels)
        //   labels "abcdefghijklmnopqrstuvwxyz"  -- lowercase only (26 labels)
        labels "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"

        // Line-jump label mode. "directional" (default) uses a-z below cursor
        // and A-Z above regardless of the `labels` setting. "unified" splits
        // the `labels` charset in half: first half → below, second half → above.
        line_labels "directional"

        // Float dimensions. Format: "WIDTHxHEIGHT".
        // Accepts percentages ("90%x90%") or absolute cell counts ("200x50").
        // Percentage widths are auto-centered horizontally.
        size "90%x90%"

        // Theme colors — hex strings ("#rrggbb" or "rrggbb").
        // All default to Catppuccin Macchiato. Omit any key to keep the default.
        color_sel_bg          "#8aadf4"  // selection background          (Blue)
        color_sel_fg          "#24273a"  // selection foreground          (Base)
        color_cursor_bg       "#cad3f5"  // cursor cell background        (Text)
        color_cursor_fg       "#24273a"  // cursor cell foreground        (Base)
        color_gutter_mark     "#eed49f"  // cursor-row gutter marker      (Yellow)
        color_gutter_dim      "#6e738d"  // other gutter numbers          (Overlay0)
        color_sel_label       "#8bd5ca"  // "SEL N lines" footer label    (Teal)
        color_footer_dim      "#6e738d"  // footer status text            (Overlay0)
        color_footer_key      "#b8c0e0"  // footer key hint labels        (Subtext1)
        color_jump_label_bg   "#f5a97f"  // jump label background         (Peach)
        color_jump_label_fg   "#24273a"  // jump label foreground         (Base)
        color_jump_match_fg   "#ed8796"  // jump match prefix highlight   (Red)
        color_jump_partial_fg "#eed49f"  // jump partial match highlight  (Yellow)
        color_search_match_bg "#a6da95"  // search match background       (Green)
        color_search_current_bg "#eed49f" // current search match bg      (Yellow)
        color_search_fg       "#24273a"  // search match foreground       (Base)
    };
    SwitchToMode "locked"
}
```

## Keys reference

| Key | Default | Description |
|---|---|---|
| `profiles` | `"viewport,200,2000"` | Comma-separated depth profiles. `viewport` = visible area only; a positive integer = that many scrollback lines. At least one profile required. |
| `size` | _(Zellij default float size)_ | Float dimensions as `WIDTHxHEIGHT`. Percentages or absolute cell counts. Omit to use Zellij's default float placement. |
| `labels` | `"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"` | Characters used as jump labels for `s` word-jump. Any printable non-whitespace chars; duplicates silently removed; order preserved. Fewer labels means more chars must be typed before labels appear. |
| `line_labels` | `"directional"` | Line-jump (`l`) label scheme. `"directional"` (default): `a`–`z` for lines below cursor, `A`–`Z` for lines above — independent of `labels`. `"unified"`: splits the `labels` charset in half, first half for below and second half for above. |
| `color_sel_bg` | `"#8aadf4"` | Selection highlight background. |
| `color_sel_fg` | `"#24273a"` | Selection highlight foreground. |
| `color_cursor_bg` | `"#cad3f5"` | Cursor cell background. |
| `color_cursor_fg` | `"#24273a"` | Cursor cell foreground. |
| `color_gutter_mark` | `"#eed49f"` | Cursor-row gutter marker color. |
| `color_gutter_dim` | `"#6e738d"` | Non-cursor gutter number color. |
| `color_sel_label` | `"#8bd5ca"` | "SEL N lines" footer indicator. |
| `color_footer_dim` | `"#6e738d"` | Footer status/dim text. |
| `color_footer_key` | `"#b8c0e0"` | Footer key hint labels. |
| `color_jump_label_bg` | `"#f5a97f"` | Jump label background. |
| `color_jump_label_fg` | `"#24273a"` | Jump label foreground. |
| `color_jump_match_fg` | `"#ed8796"` | Jump matched-prefix highlight color. |
| `color_jump_partial_fg` | `"#eed49f"` | Jump partial-match highlight (too many to label). |
| `color_search_match_bg` | `"#a6da95"` | Non-current search match background. |
| `color_search_current_bg` | `"#eed49f"` | Current search match background. |
| `color_search_fg` | `"#24273a"` | Search match foreground. |

## Future configuration (not yet implemented)

### Key bindings

Custom key bindings inside the float are planned but not yet configurable.

### Jump label charset

The characters used for jump labels (`a-zA-Z`) are currently hardcoded.
