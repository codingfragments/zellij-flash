# Architecture

## Source pane tracking

zellij-flash uses the same persistent-plugin pattern as
[zextract](../extractor/doc/pane-content-extraction.md).

The plugin is launched with `LaunchOrFocusPlugin`, which keeps the process alive
between invocations. While backgrounded it receives `PaneUpdate` on every focus
change and records `last_focused_non_plugin` — the ID of the most recently
focused terminal pane. When the keybind fires and the plugin steals focus, that
ID is already known.

`source_pane::pick()` resolves the source in four tiers:

| Priority | Condition |
|---|---|
| 1 | Currently focused non-plugin pane (brief window at open) |
| 2 | `last_focused_non_plugin` hint (validated against current manifest) |
| 3 | First tiled, non-suppressed, non-plugin pane |
| 4 | Any non-plugin pane |

The hint is scoped to the active tab via `active_tab_index` (from `TabUpdate`)
to prevent cross-tab pane selection in multi-tab sessions.

**Critical ordering constraint:** `last_focused_non_plugin` must be updated
*before* `pick()` is called in the same `PaneUpdate` handler. There is a brief
transitional event where the terminal pane still appears focused even as the
plugin is being raised. Updating after `pick()` loses that window.

## Scrollback extraction

`get_pane_scrollback(PaneId::Terminal(source), want_full)` returns
`lines_above_viewport` and `viewport`.

- `viewport` profile: use `viewport` only, `want_full = false`.
- Line-capped profiles: concatenate `lines_above_viewport + viewport`, take the
  last N lines, `want_full = true`.

The captured text is stored as `Vec<String>` (one entry per logical line) on
`State`. Content is rendered as plain text — no ANSI color reproduction.

## Render model

Rendering uses [ratatui](https://ratatui.rs) against the Zellij plugin render
API (same as zextract).

**Buffer reuse:** a single `render_buffer: Option<Buffer>` is held on `State`
and reset in-place each frame. Allocating a fresh buffer each frame churns the
WASM allocator until Zellij's host refuses with "growth operation limited".
The buffer is reallocated only when the terminal size changes.

**Layout:**

```
┌───────────────────────────────┐
│                               │  ← content area (fills all rows)
│  relative line numbers + text │
│                               │
├───────────────────────────────┤
│  footer line 1 (status)       │  ← 2-line footer block
│  footer line 2 (key hints)    │
└───────────────────────────────┘
```

No top input strip. The content area is maximised. The footer content changes
per mode (Normal, Jump, LineJump, Search:input, Search:nav, Confirm).

## Cursor and viewport

`cursor: (usize, usize)` is a logical `(line, col)` position into the captured
lines. `scroll_y: usize` is the index of the first visible line. `scroll_x:
usize` is the horizontal char offset.

Both offsets follow the cursor automatically after every move. On half-page
PgUp/PgDn the cursor moves by `viewport_height / 2` lines and `scroll_y` is
set so the cursor lands at the vertical centre. On left/right moves within a
line, `scroll_x` adjusts so the cursor stays visible, accounting for the `…`
indicator that occupies the rightmost display column when content overflows.

`Shift-←`/`Shift-→` pan `scroll_x` by 5 columns without moving the cursor,
clamped at 0 on the left.

## Selection

`anchor: Option<(usize, usize)>` on `State`. Selection is **orthogonal to
mode** — the anchor can be active in Normal, Jump, LineJump, or Search:nav mode.

Selected range = `min(anchor, cursor)..=max(anchor, cursor)` in stream order.
Highlighted during render with a blue background / dark foreground. The selection
extends whenever the cursor moves while an anchor is active, including after a
jump or a search navigation step.

## Mode state machine

```
                     ┌──────────────────────────────┐
                     ▼                              │ Esc/done
              ┌─────────────┐                       │
        ┌────►│   Normal    │◄──────────────────────┤
        │     └──┬──┬──┬───┘                        │
        │        │s │l │/                           │
        │        ▼  │  │  ┌─────────────────────┐  │
        │  ┌──────┐ │  │  │  Search:input        │  │
     Esc/  │ Jump │ │  └─►│  (type query)        │  │
     done  └──────┘ │     └──────────┬────────── ┘  │
        │            │          Enter│  Esc→Normal   │
        │            ▼               ▼               │
        │      ┌──────────┐  ┌──────────────────┐   │
        └──────│ LineJump │  │  Search:nav       │   │
            Esc└──────────┘  │  (n/N navigate)  │───┘
                             │  Space→anchor+Normal  │
                             └──────────────────────┘

        Shift-Enter + newlines:
        Normal ──────────────► Confirm ──y/Enter──► (insert + close)
                                        └──Esc────► Normal
```

`anchor` is not a mode — it is a field that remains set across mode transitions
until explicitly cleared by `Esc` (step 3 of the cancel chain) or a second
`Space` press. It can be active simultaneously with Jump, LineJump, or
Search:nav, causing those modes to extend the selection on jump/navigation.

### Search mode detail

**Input phase** (`Mode::Search { navigating: false }`):
- Any printable char appends to the query; matches highlight live across all
  captured lines.
- `Enter` commits the query → switches to navigation phase, cursor jumps to
  first match at or after the current position.
- `Esc` cancels and returns to Normal without moving the cursor.

**Navigation phase** (`Mode::Search { navigating: true }`):
- `n` / `N` jump to next / previous match (wrapping), viewport re-centres.
- `Space` sets `anchor = Some(cursor)` (match start) and returns to Normal —
  the primary power move for "search then select".
- `Esc` or any unrecognised key returns to Normal, cursor stays at current match.

## Configuration

No separate config file. All settings come through the `BTreeMap<String, String>`
passed to `load()` from the Zellij keybind `configuration` block.

Key settings:

| Key | Default | Description |
|---|---|---|
| `profiles` | `"viewport,200,2000"` | Depth profiles, cycled with `g` |
| `size` | _(Zellij default)_ | Float dimensions `"WxH"` |
| `labels` | `"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"` | Word-jump label charset |
| `line_labels` | `"directional"` | `"unified"` to use `labels` split for line-jump too |
| `color_*` | Catppuccin Macchiato | 15 color roles, see [`configuration.md`](configuration.md) |

On profile change (`g`), the plugin re-grabs the scrollback, resets the cursor
to the bottom of the new content, and clears any active selection.
