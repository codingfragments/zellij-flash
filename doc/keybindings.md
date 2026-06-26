# Keybindings

All keys active inside the zellij-flash float.

## Normal mode

| Key | Action |
|---|---|
| `↑` `↓` | Move cursor one line |
| `←` `→` | Move cursor one character (wraps at line edges) |
| `Shift-←` | Pan viewport 5 columns left (cursor stays) |
| `Shift-→` | Pan viewport 5 columns right (cursor stays) |
| `PgUp` | Move cursor up half a page, re-center vertically |
| `PgDn` | Move cursor down half a page, re-center vertically |
| `w` | Forward to start of next word (`[a-zA-Z0-9_]+`) |
| `W` | Forward to start of next WORD (non-whitespace run) |
| `b` | Backward to start of previous word |
| `B` | Backward to start of previous WORD |
| `e` | Forward to end of current/next word |
| `E` | Forward to end of current/next WORD |
| `0` | Start of line (col 0) |
| `$` | Last character of line |
| `Space` | Set selection anchor at cursor (press again to clear) |
| `s` | Enter word-jump mode |
| `S` | Enter word-jump mode — plants selection anchor at destination on completion |
| `l` | Enter line-jump mode |
| `L` | Enter line-jump mode — plants selection anchor at destination on completion |
| `/` | Enter search mode (no-op while selection anchor is active) |
| `g` | Cycle to next scrollback depth profile |
| `Enter` | Copy selection to clipboard and close (warn if no selection) |
| `Shift-Enter` | Insert selection into source pane and close (approval dialog if selection contains newlines; warn if no selection) |
| `Esc` | Context-sensitive cancel — see below |

All word motions (`w W b B e E 0 $`) and cursor moves work identically whether
or not a selection anchor is active. When an anchor is set, every cursor move
extends the selection.

---

## Word-jump mode (`s` / `S`)

Press `s` to enter. Press `S` to enter with select-jump: on completion the
selection anchor is planted at the destination (zero-width), ready to extend.
The footer shows `[SEL]` while in select-jump mode. `Esc` cancels without
touching the anchor.

Type chars to narrow matches; labels appear when ≤ label-pool size matches remain.

| Key | Action |
|---|---|
| Any printable char | Narrow matches by typed prefix |
| `Backspace` | Remove last typed char, recompute matches |
| Label key | Jump cursor to labeled match position |
| `Esc` | Cancel jump, return to Normal |

**How it works:** matches are found case-insensitively in the visible lines. Labels
are assigned by distance from the cursor (nearest first). The label character
overlays the *last* char of the matched prefix so earlier chars stay visible to
confirm the match. Label chars are chosen to never conflict with typed chars or
with valid search continuations. If a selection anchor is active, the jump
extends the selection.

See [`jump-mode.md`](jump-mode.md) for the full algorithm and color reference.

---

## Line-jump mode (`l` / `L`)

Press `l` to enter. Press `L` to enter with select-jump: on completion the
selection anchor is planted at the destination (zero-width), ready to extend.
The footer shows `[SEL]` while in select-jump mode. `Esc` cancels without
touching the anchor.

Labels appear instantly on every visible line.

| Key | Action |
|---|---|
| Lowercase label (`a`–`z`) | Jump to labeled line **below** cursor |
| Uppercase label (`A`–`Z`) | Jump to labeled line **above** cursor |
| `Esc` | Cancel, return to Normal |

**How it works:** labels appear in the gutter (replacing line numbers). Lines below
the cursor get lowercase labels (`a` = nearest, `z` = furthest). Lines above get
uppercase (`A` = nearest, `Z` = furthest). The cursor line has no label. If a
selection anchor is active, the jump extends the selection.

The label scheme is configurable — see [`configuration.md`](configuration.md).

---

## Search mode (`/`)

Only available when no selection anchor is active. Two phases:

### Input phase

Entered by pressing `/` in Normal mode.

| Key | Action |
|---|---|
| Any printable char | Append to query; matches highlight live |
| `Backspace` | Remove last query char |
| `Enter` | Confirm query → switch to navigation phase |
| `Esc` | Cancel search, return to Normal (cursor does not move) |

Footer shows `/query█  Enter:confirm  Esc:cancel`. All matches across the full
captured buffer are highlighted as you type: green for non-current matches,
yellow-bold for the first match at or after the cursor position.

### Navigation phase

Entered automatically after pressing `Enter` to confirm the query.

| Key | Action |
|---|---|
| `n` | Jump to next match (wraps), re-center viewport |
| `N` | Jump to previous match (wraps), re-center viewport |
| `Space` | Exit search **and set selection anchor** at current match start |
| `Esc` | Exit search, return to Normal (cursor stays at current match) |
| Any other key | Exit search, return to Normal |

Footer shows `/query  M/N  n:next  N:prev  Space:select  Esc:done`.

**Typical workflow:** `/` → type → `Enter` → `n`/`N` to find the right match →
`Space` to anchor there → use arrows/motions/jump to extend selection → `Enter`
to copy.

---

## Confirm mode (Shift-Enter with newlines)

Entered automatically when inserting a multi-line selection.

| Key | Action |
|---|---|
| `y` | Confirm insert, close float |
| `Enter` | Confirm insert, close float |
| `Esc` | Cancel, return to Normal (selection preserved) |

---

## Esc cancel chain

`Esc` always resolves the innermost active context first:

1. In word-jump, line-jump, or search (either phase) → cancel, return to Normal
2. In confirm mode → cancel approval, return to Normal
3. Selection anchor is active → clear anchor, return to Normal
4. No active mode or anchor → close the float
