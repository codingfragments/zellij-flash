# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2026-05-20

### Fixed

- **Multi-tab pane selection**: `PaneUpdate` could arrive before the first `TabUpdate`, causing `source_pane::pick()` to search all tabs with non-deterministic HashMap iteration order and select the wrong tab's pane. Grab is now deferred until `active_tab_index` is known; the first `TabUpdate` also invalidates any pre-tab `source_pane` so the next `PaneUpdate` re-picks with the correct tab filter.
- **Initial scroll position**: `content_rows` defaults to 24 before the first render, so the grab-time `scroll_y` calculation placed the cursor mid-screen on larger popups. `scroll_y` is now clamped to `max_scroll` at the top of every `render()` call once the real viewport height is known.
- **Page-up/down empty space**: `recenter_scroll()` centred the cursor unconditionally, leaving blank rows below the last line when near the end of the buffer. It now clamps to `max_scroll` so centering is only applied when the buffer is deep enough to fill the screen.

## [0.1.0] - 2025-01-01

Initial release.
