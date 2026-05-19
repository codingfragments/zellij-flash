# zellij-flash

A Zellij plugin for selecting and copying text from pane scrollback — with
nvim-flash-style jump-to-word and jump-to-line navigation.

## Use case

Zellij has no built-in way to select arbitrary text from a terminal pane's
scrollback. `zellij-flash` opens a floating pane that renders the source pane's
output, lets you navigate it with a cursor, select text precisely, and copy it
to the clipboard or insert it directly into the source pane.

Typical workflows:
- Copy a URL, path, or command that scrolled past
- Grab a block of output for a ticket or chat message
- Insert a previous command back into the shell without retyping

## Install

Build from source:

```sh
cargo build --release --target wasm32-wasip1
```

Copy `target/wasm32-wasip1/release/zellij_flash.wasm` to your Zellij plugin
directory (e.g. `~/.config/zellij/plugins/`).

## Configure

Add a keybind in your Zellij config (`~/.config/zellij/config.kdl`):

```kdl
keybinds {
    normal {
        bind "Alt f" {
            LaunchOrFocusPlugin "file:~/.config/zellij/plugins/zellij_flash.wasm" {
                profiles "viewport,200,2000"
                size     "90%x85%"
            }
        }
    }
}
```

### Configuration keys

| Key | Default | Description |
|---|---|---|
| `profiles` | `"viewport,200,2000"` | Comma-separated depth profiles. `viewport` = visible area only; a number = that many scrollback lines. |
| `size` | `"90%x85%"` | Float dimensions as `WIDTHxHEIGHT`. Percentages or absolute cells. |

## Usage

Press the keybind to open the float. The source pane's scrollback is rendered
with relative line numbers (cursor row = 0, other rows show distance).

| Key | Action |
|---|---|
| `↑ ↓ ← →` | Move cursor |
| `PgUp / PgDn` | Half-page jump, cursor re-centers |
| `Space` | Anchor / clear selection |
| `s` | Word jump — type chars, pick a label |
| `l` | Line jump — pick a line label |
| `g` | Cycle scrollback depth |
| `Enter` | Copy selection to clipboard, close |
| `Shift-Enter` | Insert selection into source pane, close |
| `Esc` | Cancel jump → cancel selection → close |

See [`doc/keybindings.md`](doc/keybindings.md) for the full keybinding reference and [`doc/configuration.md`](doc/configuration.md) for all configuration options with defaults.

## Status

Under active development. See [`PLAN.md`](PLAN.md) for the phase roadmap.
