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
│  footer line 1                │  ← 2-line footer block
│  footer line 2                │
└───────────────────────────────┘
```

No top input strip. The content area is maximised.

## Cursor and viewport

`cursor: (usize, usize)` is a logical `(line, col)` position into the captured
lines. `scroll_y: usize` is the index of the first visible line.

The viewport always keeps the cursor visible. On half-page PgUp/PgDn the cursor
moves by `viewport_height / 2` lines and `scroll_y` is set so the cursor lands
at the vertical center of the viewport.

Horizontal scroll (`scroll_x`) is added in a later phase. Initially, lines wider
than the viewport are truncated.

## Selection

`anchor: Option<(usize, usize)>` on `State`. Selection is **orthogonal to
mode** — the anchor can be active in Normal, Jump, or LineJump mode.

Selected range = `min(anchor, cursor)..=max(anchor, cursor)` in stream order.
Highlighted during render with an accent background. The selection extends when
the cursor moves while an anchor is active, including after a jump.

## Mode state machine

```
           ┌─────────────┐
     ┌────►│   Normal    │◄──────────────────┐
     │     └──────┬──────┘                   │
     │       s    │    l                      │
     │       ▼    │    ▼                      │
     │  ┌──────┐  │  ┌──────────┐            │
  Esc/  │ Jump │  │  │ LineJump │  Esc/done  │
  done  └──────┘  │  └──────────┘            │
     │             │                          │
     │        Shift-Enter + newlines          │
     │             ▼                          │
     │       ┌─────────┐    y / Esc           │
     └───────│ Confirm │──────────────────────┘
             └─────────┘
```

`anchor` is not a mode — it is a field that remains set across mode transitions
until explicitly cleared by Esc or a second Space press.

## Configuration

No separate config file. All settings come through the `BTreeMap<String, String>`
passed to `load()` from the Zellij keybind `configuration` block.

Parsed keys:

| Key | Type | Default | Description |
|---|---|---|---|
| `profiles` | comma-separated string | `"viewport,200,2000"` | Depth profiles |
| `size` | `"WxH"` string | `"90%x85%"` | Float dimensions |

On profile change (`g`), the plugin re-grabs the scrollback and resets the
cursor to the bottom of the new content.
