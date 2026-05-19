# zellij-flash MVP — What We Built, How, and Why

## The problem

Zellij has no built-in way to select and copy arbitrary text from a pane's
scrollback. The plugin API does not support rendering over an existing pane
(no true overlay), so the only viable approach is to open a large floating
plugin pane, copy the source pane's content into it, and build a full
navigation and selection UI inside that float.

---

## Core constraints that shaped every decision

**Constraint 1 — No overlay.** The floating pane approach means the user is
looking at a copy of the scrollback, not the scrollback itself. This makes
source-pane tracking non-trivial (the source pane loses focus the instant the
plugin opens) and requires a careful persistent-process pattern to remember
which pane was active.

**Constraint 2 — WASM allocator budget.** Zellij plugins run as WASM modules.
Allocating a fresh ratatui `Buffer` on every render frame churns the WASM heap
until the host refuses with "growth operation limited". The fix is to hold one
buffer on `State` and reset it in-place, reallocating only when the terminal
size changes.

**Constraint 3 — No filesystem config.** Rather than adding a KDL config file
(which requires the `HostFolderChanged` async dance from zextract), all
configuration is passed through the keybind `configuration` block. This keeps
the plugin stateless between invocations and avoids a whole category of race
conditions.

---

## Source pane tracking

The `LaunchOrFocusPlugin` pattern keeps the plugin process alive between
invocations. While backgrounded, it receives `PaneUpdate` on every focus change
and records `last_focused_non_plugin`. When the keybind fires and the plugin
steals focus, the terminal pane is already unfocused in subsequent events — but
the hint is already stored.

`source_pane::pick()` resolves the source in four tiers:
1. Currently-focused non-plugin pane (brief window at open)
2. `last_focused_non_plugin` hint (the workhorse)
3. First tiled non-suppressed non-plugin pane (cold-start heuristic)
4. Any non-plugin pane (last resort)

The hint is scoped to the active tab via `active_tab_index` to prevent
cross-tab selection in multi-tab sessions. The hint must be updated **before**
`pick()` is called in the same `PaneUpdate` handler — there is a brief
transitional event where the source pane still appears focused, and missing it
causes a fallback to a lower-priority tier.

This pattern is documented in full in `../extractor/doc/pane-content-extraction.md`
and was reused verbatim from the zextract project.

---

## Rendering

Plain text only — no ANSI color reproduction. The UX value is in navigation and
selection, not in color. Reproducing terminal colors through ratatui would
require a full ANSI parser and produce imperfect results for complex sequences.

**Relative line numbers** are shown in a left gutter: the cursor row shows `0►`,
other rows show their distance from the cursor. This matches nvim's
`relativenumber` and makes PgUp/PgDn jumps and line-jump labels immediately
readable.

**Horizontal scroll** uses `scroll_x` / `scroll_y` offsets. Lines wider than
the viewport show a `…` indicator at the left or right edge. Scroll follows the
cursor automatically; `Shift-←`/`Shift-→` pan the viewport 5 columns without
moving the cursor.

---

## Mode state machine

```
Normal ──s──► Jump (word)    ──label──► Normal (cursor moved)
       ──l──► LineJump       ──label──► Normal
       ──/──► Search:input   ──Enter──► Search:nav ──Space──► Normal (anchor set)
       ──⇧↵─► Confirm        ──y/↵───► (insert + close)
```

`anchor: Option<(usize, usize)>` is orthogonal to mode — it can be active in
any mode and is never implicitly cleared by a mode transition. Esc resolves the
innermost active context first: jump/search → selection → close.

---

## What shipped in v0.1.0

### Phase 1 — Foundation
- **1a** Scaffold: float opens, scrollback rendered, Esc closes
- **1b** Cursor: `(line, col)` state, arrow movement with line-edge wrapping,
  half-page PgUp/PgDn with vertical re-centering
- **1c** Profiles: `profiles` keybind key parsed, `g` cycles depth, `size` key
  resizes the float on open

### Phase 2 — Selection and actions
- **2a** Selection: `Space` anchors at exact char position, arrows extend,
  blue highlight with dark fg (Catppuccin Macchiato), `Esc` cancels before quit
- **2b** Copy/insert: `Enter` copies to clipboard, `Shift-Enter` inserts into
  source pane via `write_chars_to_pane_id`; multi-line insert shows inline
  approval prompt (`y`/`Enter`/`Esc`)

### Phase 3 — Navigation
- **3a** Word-jump (`s`): type chars → labels appear on visible matches sorted
  by cursor distance; label char on the *last* char of the matched prefix so
  earlier chars stay visible confirming the match; jumps extend selection if
  anchor active
- **3b** Line-jump (`l`): instant gutter labels, lowercase `a`–`z` for lines
  below cursor (nearest = `a`), uppercase `A`–`Z` for lines above (nearest = `A`)

### Phase 4 — Horizontal scroll
- `scroll_x` viewport offset follows cursor on left/right movement; `…`
  indicators at left/right edges; `Shift-←`/`Shift-→` pan 5 columns

### Phase 5 — nvim motions and search
- **5a** Word motions: `w W b B e E 0 $` — all work in and out of selection
  mode via a shared `cclass()` helper (Space=0, Word=1, Other=2)
- **5b** Search: `/` enters input phase (type freely), `Enter` commits →
  navigation phase (`n`/`N` next/prev, `Space` anchors at match start)

### Theming and configuration
- All 15 UI color roles configurable via `color_*` keybind keys; defaults are
  Catppuccin Macchiato; `Theme` struct parsed from hex strings in `load()`
- Jump label charset configurable via `labels` key; `line_labels "unified"`
  splits that charset for line-jump too

### CI and release
- `rust-toolchain.toml` pins to 1.94.1
- GitHub Actions: CI on all branch prefixes + PRs; release workflow on `v*.*.*`
  tags (builds WASM, SHA-256, publishes GitHub Release)
- Pre-push hook runs `just check` (fmt + clippy + test + wasm + size budget)

---

## Permissions

```rust
request_permission(&[
    PermissionType::ReadApplicationState,   // PaneUpdate, TabUpdate
    PermissionType::ChangeApplicationState, // rename_plugin_pane, resize float
    PermissionType::ReadPaneContents,       // get_pane_scrollback
    PermissionType::WriteToClipboard,       // copy_to_clipboard
    PermissionType::WriteToStdin,           // write_chars_to_pane_id (insert)
]);
```
