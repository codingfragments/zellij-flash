# zellij-flash — Design Plan

A Zellij plugin that opens a floating pane showing the source pane's scrollback,
lets the user navigate it with a cursor, select text char-precisely, and copy or
insert it — with nvim-flash-style jump-to-word and jump-to-line modes.

---

## Problem statement

Zellij has no built-in way to select and copy arbitrary text from a pane's
scrollback. Direct pane overlays are not possible through the plugin API. The
workaround is to open a large floating plugin pane, read the scrollback via
`get_pane_scrollback`, render it as navigable text, and let the user select and
act on it.

---

## Key design decisions and rationale

### Plain text rendering (no ANSI color reproduction)

`get_pane_scrollback` returns strings. Whether those strings carry embedded ANSI
escape sequences is an open question at implementation time, but reproducing
terminal colors through ratatui's `Span` system would require a full ANSI parser
and would be imperfect for complex sequences. The UX value of this plugin is in
jump + selection, not in color reproduction. Rendering plain text keeps the
rendering layer simple and focuses effort where it matters.

Relative line numbers are shown in a left gutter: the cursor row shows its
absolute line number in the captured buffer; every other row shows its distance
from the cursor. This matches nvim's `relativenumber` and makes half-page jumps
and explicit line targets immediately readable.

### Floating pane, not an overlay

Zellij's plugin API does not support rendering over an existing pane. The plugin
opens as a large floating pane (default 90% width × 85% height, configurable via
the keybind `size` key). The source pane's content is copied into the plugin's
own render buffer. This is the same approach as `zextract`.

### Source pane tracking

Identical to the `zextract` pattern documented in
`../extractor/doc/pane-content-extraction.md`:

- Use `LaunchOrFocusPlugin` (persistent process, not a fresh spawn each time).
- Subscribe to `PaneUpdate` and `TabUpdate` from `load()`.
- In every `PaneUpdate`, record `last_focused_non_plugin` **before** calling
  `source_pane::pick()` — the brief transitional window where the terminal pane
  still appears focused must not be missed.
- Scope the hint to the active tab via `active_tab_index` from `TabUpdate`.
- `pick()` uses a four-tier preference: currently-focused non-plugin pane →
  hint → first tiled non-suppressed non-plugin pane → any non-plugin pane.

No separate config file. All configuration comes through the `BTreeMap<String,
String>` passed to `load()` from the keybind `configuration` block. No
`HostFolderChanged` dance needed.

### Scrollback depth profiles

Three built-in profiles: `viewport` (visible area only), `200` (200 scrollback
lines), `2000` (2000 scrollback lines). Configurable via the keybind:

```kdl
bind "Alt f" {
    LaunchOrFocusPlugin "zflash.wasm" {
        profiles "viewport,200,2000"
        size "90%x85%"
    }
}
```

`profiles` is a comma-separated list: `viewport` means viewport-only, a number
means that many lines of scrollback (lines_above_viewport + viewport, capped).
The plugin opens at the first profile. `g` cycles through profiles at runtime.
The current profile name is shown in the footer.

### Layout

```
┌─ zellij-flash ── [200] ───────────────────────────────┐
│  42  some line of output here                          │
│   3  another line                                      │
│   2  and another                                       │
│   1  the line just above cursor                        │
│   0► current cursor line █                             │
│   1  line below cursor                                 │
│   …                                                    │
├────────────────────────────────────────────────────────┤
│ [NORMAL] s:jump  l:line  Space:sel  Enter:copy  Esc:q  │
│ [profile: 200]  1234 lines                             │
└────────────────────────────────────────────────────────┘
```

- **No top input strip.** Content area is maximised.
- **Footer: 2 lines** inside a bordered block. Content changes per mode.
- The border title shows the plugin name. The profile label in the footer shows
  the active depth.
- Wide lines are **truncated** initially (with a `…` indicator). Horizontal
  scroll is added in a later phase.

### Cursor model

State tracks a `(line, col)` logical position into the captured text. The
viewport is a `(scroll_y, scroll_x)` offset that keeps the cursor visible.

- **Initial position:** last line of content, col 0. Most recent output is what
  the user usually wants.
- **Arrow keys:** left/right wrap at line edges (stream model, consistent with
  linear selection). Up/down move one logical line.
- **PgUp/PgDn:** half-page jump. After the jump, the cursor line is re-centered
  vertically in the viewport (same as vim's `Ctrl-U`/`Ctrl-D`).
- **Horizontal scroll:** deferred to a later phase. Initially the view is fixed
  at col 0 and wide lines are truncated.

### Selection model

Linear (stream) selection, char-precise. State: `anchor: Option<(usize, usize)>`.

- **Space:** if no anchor, set `anchor = Some(cursor)`. If anchor already set,
  clear it (toggle off).
- **Arrows during active anchor:** move cursor, selection spans from anchor to
  cursor (or cursor to anchor, whichever is earlier in the stream).
- **Rendering:** selected range is highlighted (inverted or accent background).
- **Jump during active selection:** pressing `s` or `l` and completing a jump
  moves the cursor to the jump target AND extends the selection — the anchor
  stays fixed, the cursor moves. This is the core power move: anchor with Space,
  flash-jump to the far end of what you want, press Enter.

### Enter / Shift-Enter

**Enter (copy):**
- If selection is active: copy selected text to clipboard as-is (newlines
  preserved), close plugin.
- If no selection: show status warning in footer ("No selection — Space to
  anchor"), stay open.

**Shift-Enter (insert):**
- If selection is active and text contains no newlines: write to source pane via
  `write_chars_to_pane_id`, close plugin.
- If selection contains newlines: enter `Mode::Confirm` — footer shows inline
  approval prompt ("Insert N lines into pane? `y` confirm / `Esc` cancel"). `y`
  fires the insert and closes; `Esc` returns to normal mode.
- If no selection: same warning as Enter, stay open.

### Esc — context-sensitive cancel chain

1. If in `Mode::Jump` or `Mode::LineJump`: cancel jump, return to `Mode::Normal`.
2. If in `Mode::Confirm`: cancel approval, return to `Mode::Normal`.
3. If selection is active (anchor set): clear anchor, return to `Mode::Normal`.
4. Otherwise: close the plugin.

### Jump modes

#### `s` — word-first (flash-style)

1. Press `s` → enter `Mode::Jump`. Footer shows "jump: _".
2. User types chars. Plugin finds all occurrences of the typed prefix in the
   visible content (case-insensitive).
3. When matches are few enough to label (≤52), render jump labels (`a`–`z`,
   `A`–`Z`) superimposed on each match position. Labels are assigned by
   **distance from cursor** (closest = `a`, next = `b`, …). Label characters are
   chosen to not conflict with chars already typed (so typing `s` → `a` →
   label `a` can't be confused with typing the letter `a` as the next search
   char).
4. User presses a label key → cursor moves to that match. If a selection anchor
   is active, the selection extends to the new cursor position.
5. On jump: scroll viewport so new cursor line is vertically centered.
6. `Esc` cancels jump mode.

#### `l` — label-first (line jump)

1. Press `l` → enter `Mode::LineJump`. Footer shows "line: ".
2. Every visible line immediately gets a 1-char label rendered in the line-number
   gutter. Same `a`–`z`/`A`–`Z` pool, assigned by distance from cursor.
3. User presses a label key → cursor moves to that line (col 0 or preserved col,
   TBD). Selection extends if anchor active.
4. On jump: cursor line re-centered vertically.
5. `Esc` cancels.

Rationale for label-first on `l`: line targets are visually obvious (the user
can see the line they want), so no search-narrowing step is needed. The label
pool of 52 covers all visible lines for any realistic terminal height.

### Render buffer reuse

Following the zextract lesson: allocating a fresh ratatui `Buffer` each frame
churns the WASM allocator until Zellij's host refuses ("growth operation
limited"). Hold one `render_buffer: Option<Buffer>` on `State`, reset in-place
each frame, reallocate only when the terminal size changes.

---

## Mode state machine

```
           ┌─────────────┐
     ┌────►│   Normal    │◄────────────────────────────┐
     │     └──────┬──────┘                             │
     │            │ s           ┌──────────────┐       │
     │            ▼             │   Confirm    │       │
     │     ┌─────────────┐      │  (newline    │       │
  Esc/done │    Jump     │      │   approval)  │       │
     │     │  (word)     │      └──────┬───────┘       │
     │     └─────────────┘     y/Esc  │               │
     │            │ l                  └───────────────┘
     │            ▼
     │     ┌─────────────┐
     └─────│  LineJump   │
           └─────────────┘
```

Selection anchor is **orthogonal to mode** — it can be active in Normal, Jump,
or LineJump. It is not a mode of its own; it's a field `anchor: Option<(usize,
usize)>` on `State`.

---

## Implementation phases

Each phase produces a working, buildable, testable plugin. No phase depends on
code from an unbuilt later phase.

---

### Phase 1a — Scaffold + scrollback render

**Goal:** plugin opens as a float, grabs the source pane's scrollback, renders
it as plain text with a relative line-number gutter. Esc closes. Nothing else.

**What to build:**
- Cargo workspace, `zellij-tile` dependency, `wasm32-wasi` target.
- `State` struct: `source_pane`, `last_focused_non_plugin`, `active_tab_index`,
  `own_plugin_id`, `lines: Vec<String>`, `render_buffer`.
- `load()`: permissions, subscribe to Key/PaneUpdate/TabUpdate/PermissionResult.
- `update()`: PaneUpdate → source_pane::pick (copy module verbatim from
  zextract); TabUpdate → active_tab_index; Key Esc → close_self().
- `render()`: ratatui, render lines with relative line-number gutter. Cursor row
  hardcoded to last line for now (no cursor movement yet). Footer: plugin name +
  profile name + line count.
- No cursor, no selection, no jump.

**Testable:** open the plugin in a terminal with some output, confirm the
scrollback appears and Esc closes.

---

### Phase 1b — Cursor movement

**Goal:** add a `(line, col)` cursor that moves with arrow keys and half-page
PgUp/PgDn. Cursor line re-centers on half-page jump.

**What to build:**
- `State` adds: `cursor: (usize, usize)`, `scroll_y: usize`.
- Cursor starts at `(lines.len().saturating_sub(1), 0)`.
- Arrow keys: up/down move one line, left/right wrap at line edges.
- PgUp/PgDn: move cursor by `viewport_height / 2` lines, then re-center
  `scroll_y` so cursor line is at `viewport_height / 2`.
- Render: highlight cursor cell (inverted style). Line-number gutter shows 0 at
  cursor row, distances elsewhere.
- Footer: shows `[NORMAL]` mode tag + key hints.

**Testable:** cursor moves, half-page jumps re-center, gutter numbers update.

---

### Phase 1c — Profile cycling

**Goal:** parse `profiles` from the keybind configuration, show profile label in
footer, cycle with `g`.

**What to build:**
- Parse `configuration.get("profiles")` → `Vec<Profile>` where `Profile` is
  `Viewport` or `Lines(usize)`.
- Default: `[Viewport, Lines(200), Lines(2000)]`.
- `current_profile: usize` on State, cycles with `g`.
- On profile change: re-grab scrollback, reset cursor to bottom.
- Footer shows `[200]` or `[viewport]`.
- Parse `configuration.get("size")` → call `change_floating_panes_coordinates`
  on open to resize to configured dimensions.

**Testable:** `g` cycles profiles, line count changes, cursor resets.

---

### Phase 2a — Selection

**Goal:** Space anchors selection at cursor, arrows extend char-precisely.
Selected range is highlighted. Esc clears selection.

**What to build:**
- `anchor: Option<(usize, usize)>` on State.
- Space: toggle anchor (set if None, clear if Some).
- Render: compute selected byte range from anchor + cursor (order-independent),
  highlight all cells in range with accent background. Wrap across lines.
- Footer in selection mode shows char count + line count of selection.
- Esc priority: clear anchor before quitting.

**Testable:** anchor, extend with arrows, Esc cancels, highlight is correct.

---

### Phase 2b — Enter / Shift-Enter / Confirm dialog

**Goal:** Enter copies selection to clipboard; Shift-Enter inserts into source
pane (with newline approval dialog); both warn if no selection.

**What to build:**
- `Mode::Confirm { text: String }` variant.
- Enter: if anchor, copy selected text → `copy_to_clipboard` → `close_self`.
  If no anchor, footer warning, stay open.
- Shift-Enter: if anchor and no newlines in text, `write_chars_to_pane_id` →
  `close_self`. If newlines, enter `Mode::Confirm`.
- In `Mode::Confirm`: footer shows approval prompt. `y` → insert → close.
  `Esc` → back to `Mode::Normal`.

**Testable:** copy verified in clipboard, insert appears in source pane, multi-
line approval dialog appears and both paths (confirm/cancel) work.

---

### Phase 3a — Word jump (`s`)

**Goal:** press `s`, type chars, see labels on matching word positions, press
label to jump cursor there.

**What to build:**
- `Mode::Jump { typed: String, labels: Vec<(usize, usize, char)> }` variant.
  Each label entry is `(line, col, label_char)`.
- On each keystroke in Jump mode: find all occurrences of `typed` prefix in
  visible lines (case-insensitive). If ≤52 matches, assign labels by distance
  from cursor. Render labels superimposed on match positions with highlight style.
- On label key press: move cursor to labeled position. If anchor active, selection
  extends. Re-center viewport.
- Label char pool filtered to exclude chars already in `typed` (no ambiguity).
- Esc: back to Normal.

**Testable:** type `s` then a common char (e.g., `e`), confirm labels appear,
press label, cursor lands there.

---

### Phase 3b — Line jump (`l`)

**Goal:** press `l`, see labels on every visible line in the gutter, press label
to jump to that line.

**What to build:**
- `Mode::LineJump { labels: Vec<(usize, char)> }` — one label per visible line.
- On enter: assign labels by distance from cursor row. Render labels in the
  line-number gutter (replacing the number display).
- On label key: cursor moves to that line (preserve col if within bounds, else
  col 0). Re-center viewport. If anchor active, selection extends.
- Esc: back to Normal.

**Testable:** `l` shows gutter labels, press label, cursor jumps.

---

### Phase 4 — Horizontal scroll

**Goal:** lines wider than the viewport are shown in full via horizontal
scrolling. Arrow left/right when at line edges scrolls the viewport.

**What to build:**
- `scroll_x: usize` on State.
- `max_line_width()` computed from content.
- Right arrow at end-of-line: if `scroll_x + viewport_width < max_line_width`,
  increment `scroll_x`. Otherwise wrap to next line col 0 and reset `scroll_x`.
- Render: slice each line from `scroll_x` before rendering. Show a `…` indicator
  on the right edge when content extends beyond the viewport.
- Selection rendering: must account for `scroll_x` offset in cell coordinates.
- Footer: show horizontal offset when `scroll_x > 0`.

**Testable:** open a pane with long lines, arrow right scrolls, content appears,
selection spans scrolled content correctly.

---

## Key binding summary (inside the float)

| Key | Mode | Action |
|---|---|---|
| `Esc` | Any | Context-sensitive cancel chain |
| `↑ ↓ ← →` | Normal / Select | Move cursor (wrap at line edges) |
| `PgUp` / `PgDn` | Normal / Select | Half-page jump, re-center cursor |
| `Space` | Normal | Set/clear selection anchor |
| `s` | Normal / Select | Enter word-jump mode |
| `l` | Normal / Select | Enter line-jump mode |
| `g` | Normal | Cycle scrollback depth profile |
| `Enter` | Normal | Copy selection to clipboard, quit |
| `Shift-Enter` | Normal | Insert selection into source pane, quit |
| `y` | — | Reserved (not in phase 1) |
| Label key | Jump / LineJump | Jump cursor to labeled position |

---

## Permissions required

```rust
request_permission(&[
    PermissionType::ReadApplicationState,   // PaneUpdate, TabUpdate
    PermissionType::ChangeApplicationState, // rename_plugin_pane, resize float
    PermissionType::ReadPaneContents,       // get_pane_scrollback
    PermissionType::WriteToClipboard,       // copy_to_clipboard
    PermissionType::WriteToStdin,           // write_chars_to_pane_id (insert)
]);
```

---

## Open questions / future phases

- **Char-level horizontal selection** across scrolled columns (phase 4 dependency).
- **`y` key**: copy without quitting (grab multiple things in one session).
- **Search highlight persistence**: after a word-jump, keep matches dimly
  highlighted for orientation.
- **Config for jump label charset**: currently hardcoded `a-zA-Z`.
- **Config for key bindings**: currently hardcoded.
- **Mouse support**: click to place cursor or anchor.
- **Configurable theme colors**: all colors are currently defined as named
  semantic constants (`THEME_SEL_BG`, `THEME_CURSOR_BG`, etc.) defaulting to
  Catppuccin Macchiato. A future phase should expose these through the keybind
  `configuration` map (e.g. `sel_bg "#8aadf4"`) and parse hex/named values in
  `load()`. The semantic constant layer is already in place to make this a
  localised change.
