# zellij-flash — Future Ideas

Collected open questions and feature ideas beyond v0.1.0. No priority order.

---

## Char-level horizontal selection across scrolled columns

**What:** when `scroll_x > 0` and the user makes a selection that spans the
scrolled area, the selection highlight and extracted text need to correctly
account for the horizontal viewport offset. The current implementation works
in display coordinates, which may produce off-by-one issues for selections
that start or end in the non-visible left portion of a line.

**Why not done:** requires careful audit of the display-coord ↔ logical-coord
mapping in `sel_range_for_line` and `build_line_spans` under non-zero `scroll_x`.

---

## Search highlight persistence

**What:** after pressing `s` (word-jump) and landing on a match, keep the other
matches dimly highlighted for a short time (or until the next keypress). This
lets the user orient — "there are 4 other occurrences of this token on screen"
— before deciding whether to anchor and select.

**How:** store the last `Jump` labels/matches on `State` as a `recent_matches`
field; render them in the content with a faint style until cleared by the next
keypress or mode change.

---

## Open selection in editor

**What:** a key (e.g. `o` or `e`) that takes the current selection, writes it
to a temp file, and opens it in an editor.

**Considerations:**
- Terminal editors (nvim, helix): can open in a new Zellij pane via
  `open_command_pane` or `run_command`.
- Desktop editors (VSCode, Zed): launched via `run_command` with the OS open
  command; the file path is passed as an argument.
- The editor command should be configurable — `$EDITOR` env var as default,
  overridable via the keybind config.
- The temp file needs a deterministic path (e.g. `/tmp/zflash-edit-XXXXXX`)
  and should ideally be cleaned up after the editor exits.
- Requires `PermissionType::RunCommands`.

---

## Configurable key bindings

**What:** allow remapping keys inside the float via the keybind `configuration`
block (e.g. `key_jump "j"` to use `j` instead of `s`).

**Why not done:** requires a key-parsing layer on top of the config map; the
Zellij `BareKey` type would need to be deserialised from a string. Low urgency
since the current defaults are well-chosen.

---

## Mouse support

**What:** click to place the cursor; click-drag (or click + `Space`) to anchor
and extend a selection.

**Why not done:** Zellij's `Mouse` event type delivers click coordinates in the
plugin's own pane space, which need to be mapped back to line/col in the
content area (accounting for gutter width, `scroll_y`, and `scroll_x`). The
mapping is straightforward but requires subscribing to `EventType::Mouse` and
handling the coordinate translation.

---

## Overlay illusion

**What:** open the float with position and size set to exactly cover the source
pane, making it look like an in-place overlay rather than a separate float.

**How:** on open, query the source pane's geometry from the `PaneUpdate`
manifest (x, y, cols, rows) and call `change_floating_panes_coordinates` to
position the plugin pane identically. The scrollback content fills the same
space, so visually it appears to be an in-place selection mode on the original
pane.

**Challenge:** the pane geometry is available in `PaneInfo` but the float
coordinates API uses percentage or absolute strings; mapping pane cell
dimensions to the API format needs testing. Also, the Zellij frame/border adds
a few rows/cols that would need to be accounted for.

---

## Regex search

**What:** upgrade `/` search from plain substring to full regex (or at least
basic patterns like `\b`, character classes, alternation).

**How:** add `regex-lite` dependency (already used in zextract, ~50 KB WASM
overhead). Replace the substring scan in `compute_search_matches` with a
compiled `Regex`. Add an `i` toggle for case sensitivity.

---

## Multi-selection and batch copy

**What:** mark multiple disjoint regions (like Ctrl-click in a GUI editor) and
copy them all joined by a delimiter.

**How:** replace `anchor: Option<(usize, usize)>` with a `Vec<((usize,usize),(usize,usize))>`
of completed ranges, plus the current in-progress anchor. A new key (e.g. `m`)
adds the current selection to the list without closing. `Enter` copies all
ranges joined by newlines.
