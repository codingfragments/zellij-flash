# Changelog

All notable changes to this project will be documented in this file.

## [0.1.2] - 2026-06-03

### Fixed

- **Jump partial-match highlighting**: when the typed prefix matches more positions than the label pool can cover, all matches are now highlighted (yellow by default) instead of showing nothing. The footer shows "N matches, keep typing…" to confirm the search is active.
- **Jump label / continuation conflicts**: label characters no longer conflict with valid search continuations. If a next character is shared by two or more matches (e.g. both `word` and `worm` follow `wo` with `r`), that character is excluded from the label pool so typing it always narrows the search. If a next character is unique to one match, it is pre-assigned as that match's label and cannot appear on any other position.

### Added

- **`color_jump_partial_fg`** config key: separate highlight color for partial matches (too many to label). Defaults to yellow (`#eed49f`), visually distinct from the red labeled-match prefix highlight (`color_jump_match_fg`).
- **`doc/jump-mode.md`**: detailed reference for the word-jump algorithm — matching rules, label states, assignment logic, rendering priority, and color config.
- **Jump test fixture** (`fixtures/load-jump-test.sh`): five scenarios covering unique continuations, ambiguous continuations, mixed cases, partial flood, and EOL matches.

## [0.1.1] - 2026-05-20

### Fixed

- **Multi-tab pane selection**: `PaneUpdate` could arrive before the first `TabUpdate`, causing `source_pane::pick()` to search all tabs with non-deterministic HashMap iteration order and select the wrong tab's pane. Grab is now deferred until `active_tab_index` is known; the first `TabUpdate` also invalidates any pre-tab `source_pane` so the next `PaneUpdate` re-picks with the correct tab filter.
- **Initial scroll position**: `content_rows` defaults to 24 before the first render, so the grab-time `scroll_y` calculation placed the cursor mid-screen on larger popups. `scroll_y` is now clamped to `max_scroll` at the top of every `render()` call once the real viewport height is known.
- **Page-up/down empty space**: `recenter_scroll()` centred the cursor unconditionally, leaving blank rows below the last line when near the end of the buffer. It now clamps to `max_scroll` so centering is only applied when the buffer is deep enough to fill the screen.

## [0.1.0] - 2025-01-01

Initial release.
