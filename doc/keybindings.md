# Keybindings

All keys active inside the zellij-flash float.

## Universal (any mode)

| Key | Action |
|---|---|
| `Esc` | Context-sensitive cancel: jump mode → selection → close float |

## Normal mode

| Key | Action |
|---|---|
| `↑` `↓` | Move cursor one line |
| `←` `→` | Move cursor one character (wraps at line edges) |
| `PgUp` | Move cursor up half a page, re-center cursor vertically |
| `PgDn` | Move cursor down half a page, re-center cursor vertically |
| `Space` | Set selection anchor at cursor (press again to clear) |
| `s` | Enter word-jump mode |
| `l` | Enter line-jump mode |
| `g` | Cycle to next scrollback depth profile |
| `Enter` | Copy selection to clipboard and close (warn if no selection) |
| `Shift-Enter` | Insert selection into source pane and close (approval dialog if selection contains newlines; warn if no selection) |

## Word-jump mode (`s`)

| Key | Action |
|---|---|
| Any printable char | Narrow matches by prefix |
| Label key (`a`–`z`, `A`–`Z`) | Jump cursor to labeled match |
| `Esc` | Cancel jump, return to Normal |

How it works: type one or more characters of the target word. The plugin
highlights all matches and overlays jump labels on them, ordered by distance
from the cursor. Press a label key to jump. If a selection anchor is active,
the jump extends the selection instead of just moving the cursor.

Label characters are chosen to never conflict with characters already typed as
search input.

## Line-jump mode (`l`)

| Key | Action |
|---|---|
| Label key (`a`–`z`, `A`–`Z`) | Jump cursor to labeled line |
| `Esc` | Cancel jump, return to Normal |

How it works: every visible line immediately receives a label in the line-number
gutter. Labels are assigned by distance from the cursor row. Press a label key
to jump to that line. If a selection anchor is active, the jump extends the
selection.

## Confirm mode (Shift-Enter with newlines)

Entered automatically when inserting a selection that contains newlines.

| Key | Action |
|---|---|
| `y` | Confirm insert, close float |
| `Esc` | Cancel, return to Normal |

## Esc cancel chain

Esc resolves the innermost active context first:

1. If in word-jump or line-jump mode → cancel jump, return to Normal
2. If in confirm mode → cancel approval, return to Normal
3. If selection anchor is active → clear anchor, return to Normal
4. Otherwise → close the float
