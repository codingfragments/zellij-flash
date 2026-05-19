mod render;
mod source_pane;

use std::collections::BTreeMap;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use zellij_tile::prelude::*;

// ── Theme ─────────────────────────────────────────────────────────────────────
// Runtime color theme. Defaults to Catppuccin Macchiato. All fields can be
// overridden via `color_*` keys in the keybind configuration block.

/// Parse a "#rrggbb" or "rrggbb" hex string into a ratatui Color.
fn parse_hex_color(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

#[derive(Debug, Clone)]
struct Theme {
    sel_bg: Color,
    sel_fg: Color,
    cursor_bg: Color,
    cursor_fg: Color,
    gutter_cursor: Color,
    gutter_dim: Color,
    sel_indicator: Color,
    footer_dim: Color,
    footer_key: Color,
    jump_label_bg: Color,
    jump_label_fg: Color,
    jump_match_fg: Color,
    search_match_bg: Color,
    search_current_bg: Color,
    search_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        // Catppuccin Macchiato palette
        let base = Color::Rgb(36, 39, 58); // #24273a
        let overlay0 = Color::Rgb(110, 115, 141); // #6e738d
        let text = Color::Rgb(202, 211, 245); // #cad3f5
        let yellow = Color::Rgb(238, 212, 159); // #eed49f
        let blue = Color::Rgb(138, 173, 244); // #8aadf4
        let teal = Color::Rgb(139, 213, 202); // #8bd5ca
        let subtext1 = Color::Rgb(184, 192, 224); // #b8c0e0
        let peach = Color::Rgb(245, 169, 127); // #f5a97f
        let red = Color::Rgb(237, 135, 150); // #ed8796
        let green = Color::Rgb(166, 218, 149); // #a6da95
        Self {
            sel_bg: blue,
            sel_fg: base,
            cursor_bg: text,
            cursor_fg: base,
            gutter_cursor: yellow,
            gutter_dim: overlay0,
            sel_indicator: teal,
            footer_dim: overlay0,
            footer_key: subtext1,
            jump_label_bg: peach,
            jump_label_fg: base,
            jump_match_fg: red,
            search_match_bg: green,
            search_current_bg: yellow,
            search_fg: base,
        }
    }
}

// ── Profile ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum Profile {
    Viewport,
    Lines(usize),
}

impl Profile {
    fn label(&self) -> String {
        match self {
            Profile::Viewport => "viewport".to_string(),
            Profile::Lines(n) => n.to_string(),
        }
    }
}

fn parse_profiles(s: &str) -> Vec<Profile> {
    let mut out: Vec<Profile> = s
        .split(',')
        .filter_map(|p| {
            let p = p.trim();
            if p.eq_ignore_ascii_case("viewport") {
                Some(Profile::Viewport)
            } else {
                p.parse::<usize>()
                    .ok()
                    .filter(|&n| n > 0)
                    .map(Profile::Lines)
            }
        })
        .collect();
    if out.is_empty() {
        out = default_profiles();
    }
    out
}

fn default_profiles() -> Vec<Profile> {
    vec![Profile::Viewport, Profile::Lines(200), Profile::Lines(2000)]
}

// ── Mode ──────────────────────────────────────────────────────────────────────

/// Label pool: a-z then A-Z. 52 entries; indices used to assign labels by
/// sorted distance from cursor.
const LABEL_CHARS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

#[derive(Debug, Clone)]
enum Mode {
    Normal,
    /// Word-jump: user types a prefix, labels appear on visible matches.
    /// `labels` = (line, col, label_char), sorted by distance from cursor.
    Jump {
        typed: String,
        labels: Vec<(usize, usize, char)>,
    },
    /// Line-jump: every visible line gets a gutter label immediately.
    /// `labels` = (line_idx, label_char), sorted by distance from cursor.
    LineJump {
        labels: Vec<(usize, char)>,
    },
    /// Waiting for `y`/Enter/Esc before inserting multi-line text.
    Confirm {
        text: String,
    },
    /// Incremental search. Only active outside selection mode.
    /// `matches` = (line, col) of each match start, sorted.
    /// `current` = index of the highlighted / cursor-targeted match.
    /// `navigating` = false while typing the query, true after Enter confirms it.
    Search {
        query: String,
        matches: Vec<(usize, usize)>,
        current: usize,
        navigating: bool,
    },
}

// ── State ─────────────────────────────────────────────────────────────────────

struct State {
    source_pane: Option<u32>,
    last_focused_non_plugin: Option<u32>,
    active_tab_index: Option<usize>,
    own_plugin_id: u32,
    lines: Vec<String>,
    extraction_done: bool,
    profiles: Vec<Profile>,
    current_profile: usize,
    /// Label characters used for word-jump (`s`). Configured via the `labels`
    /// keybind key. Defaults to a-z then A-Z.
    jump_labels: Vec<char>,
    /// When true, line-jump (`l`) uses `jump_labels` split into two halves:
    /// first half for lines below (nearest = first), second half for lines
    /// above (nearest = first of second half). When false (default), line-jump
    /// uses its own directional scheme: a-z below, A-Z above.
    line_labels_unified: bool,
    /// Logical cursor position: (line index, char col) into `lines`.
    cursor: (usize, usize),
    /// Selection anchor. When Some, the selection spans from anchor to cursor
    /// (order-independent). None means no active selection.
    anchor: Option<(usize, usize)>,
    /// Index of the first visible line in the content viewport.
    scroll_y: usize,
    /// Horizontal viewport offset in char columns.
    scroll_x: usize,
    /// Content area height — updated each render, used for half-page math.
    content_rows: usize,
    /// Content area width — updated each render, used for horizontal scroll math.
    content_cols: usize,
    /// Size string from keybind config ("90%x85%"), applied once on open.
    pending_size: Option<String>,
    theme: Theme,
    mode: Mode,
    /// Transient status message (warning, confirmation). Cleared on next keypress.
    message: Option<String>,
    render_buffer: Option<Buffer>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            source_pane: None,
            last_focused_non_plugin: None,
            active_tab_index: None,
            own_plugin_id: 0,
            lines: Vec::new(),
            extraction_done: false,
            profiles: default_profiles(),
            current_profile: 0,
            jump_labels: LABEL_CHARS.to_vec(),
            line_labels_unified: false,
            cursor: (0, 0),
            anchor: None,
            scroll_y: 0,
            scroll_x: 0,
            content_rows: 24,
            content_cols: 80,
            theme: Theme::default(),
            mode: Mode::Normal,
            message: None,
            pending_size: None,
            render_buffer: None,
        }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadPaneContents,
            PermissionType::WriteToClipboard,
            PermissionType::WriteToStdin,
        ]);
        subscribe(&[
            EventType::Key,
            EventType::PaneUpdate,
            EventType::TabUpdate,
            EventType::PermissionRequestResult,
        ]);
        self.own_plugin_id = get_plugin_ids().plugin_id;

        if let Some(p) = configuration.get("profiles") {
            self.profiles = parse_profiles(p);
        }
        if let Some(size) = configuration.get("size") {
            self.pending_size = Some(size.clone());
        }
        if let Some(lbls) = configuration.get("labels") {
            // Keep printable non-whitespace chars, deduplicate, preserve order.
            let mut parsed: Vec<char> = Vec::new();
            for c in lbls.chars() {
                if !c.is_whitespace() && !c.is_control() && !parsed.contains(&c) {
                    parsed.push(c);
                }
            }
            if !parsed.is_empty() {
                self.jump_labels = parsed;
            }
        }
        if let Some(v) = configuration.get("line_labels") {
            self.line_labels_unified = matches!(v.trim(), "unified" | "custom" | "true" | "on");
        }

        // Override individual theme colors from keybind config.
        // Each key is "color_<role>" with a "#rrggbb" hex value.
        macro_rules! apply_color {
            ($key:expr, $field:expr) => {
                if let Some(v) = configuration.get($key) {
                    if let Some(c) = parse_hex_color(v) {
                        $field = c;
                    }
                }
            };
        }
        apply_color!("color_sel_bg", self.theme.sel_bg);
        apply_color!("color_sel_fg", self.theme.sel_fg);
        apply_color!("color_cursor_bg", self.theme.cursor_bg);
        apply_color!("color_cursor_fg", self.theme.cursor_fg);
        apply_color!("color_gutter_mark", self.theme.gutter_cursor);
        apply_color!("color_gutter_dim", self.theme.gutter_dim);
        apply_color!("color_sel_label", self.theme.sel_indicator);
        apply_color!("color_footer_dim", self.theme.footer_dim);
        apply_color!("color_footer_key", self.theme.footer_key);
        apply_color!("color_jump_label_bg", self.theme.jump_label_bg);
        apply_color!("color_jump_label_fg", self.theme.jump_label_fg);
        apply_color!("color_jump_match_fg", self.theme.jump_match_fg);
        apply_color!("color_search_match_bg", self.theme.search_match_bg);
        apply_color!("color_search_current_bg", self.theme.search_current_bg);
        apply_color!("color_search_fg", self.theme.search_fg);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(_) => {
                rename_plugin_pane(self.own_plugin_id, "zellij-flash");
                self.apply_size();
                self.try_grab();
                true
            }
            Event::TabUpdate(tabs) => {
                if let Some(active) = tabs.iter().find(|t| t.active) {
                    self.active_tab_index = Some(active.position);
                }
                false
            }
            Event::PaneUpdate(manifest) => {
                let active_panes: Box<dyn Iterator<Item = &PaneInfo>> = match self.active_tab_index
                {
                    Some(idx) => match manifest.panes.get(&idx) {
                        Some(panes) => Box::new(panes.iter()),
                        None => Box::new(manifest.panes.values().flatten()),
                    },
                    None => Box::new(manifest.panes.values().flatten()),
                };
                for pane in active_panes {
                    if !pane.is_plugin && pane.is_focused {
                        self.last_focused_non_plugin = Some(pane.id);
                    }
                }

                let new_source = source_pane::pick(
                    &manifest,
                    self.last_focused_non_plugin,
                    self.active_tab_index,
                );
                let changed = new_source.is_some() && self.source_pane != new_source;
                if changed {
                    self.source_pane = new_source;
                }
                if !self.extraction_done {
                    self.try_grab();
                    return true;
                }
                changed
            }
            Event::Key(key) => self.handle_key(key),
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let area = Rect {
            x: 0,
            y: 0,
            width: cols as u16,
            height: rows as u16,
        };

        self.content_rows = rows.saturating_sub(4).max(1);
        self.content_cols = cols;

        let mut buf = match self.render_buffer.take() {
            Some(mut b) if b.area() == &area => {
                b.reset();
                b
            }
            _ => Buffer::empty(area),
        };

        self.render_all(area, &mut buf);
        render::flush(&buf);
        self.render_buffer = Some(buf);
    }
}

impl State {
    // ── Config ────────────────────────────────────────────────────────────────

    fn apply_size(&self) {
        let Some(ref size_str) = self.pending_size else {
            return;
        };
        let parts: Vec<&str> = size_str.splitn(2, 'x').collect();
        if parts.len() != 2 {
            return;
        }
        let width = parts[0].trim().to_string();
        let height = parts[1].trim().to_string();
        let x = center_x_for_width(&width);
        if let Some(coords) =
            FloatingPaneCoordinates::new(x, None, Some(width), Some(height), None, None)
        {
            change_floating_panes_coordinates(vec![(PaneId::Plugin(self.own_plugin_id), coords)]);
        }
    }

    // ── Grab ─────────────────────────────────────────────────────────────────

    fn try_grab(&mut self) {
        if self.extraction_done {
            return;
        }
        let Some(source) = self.source_pane else {
            return;
        };

        let profile = self
            .profiles
            .get(self.current_profile)
            .copied()
            .unwrap_or(Profile::Lines(200));

        let want_full = matches!(profile, Profile::Lines(_));
        let Ok(contents) = get_pane_scrollback(PaneId::Terminal(source), want_full) else {
            return;
        };

        let mut all: Vec<String> = match profile {
            Profile::Viewport => contents.viewport,
            Profile::Lines(_) => contents
                .lines_above_viewport
                .into_iter()
                .chain(contents.viewport)
                .collect(),
        };

        if let Profile::Lines(cap) = profile {
            if all.len() > cap {
                all.drain(..all.len() - cap);
            }
        }

        self.lines = all;
        self.extraction_done = true;

        let last = self.lines.len().saturating_sub(1);
        self.cursor = (last, 0);
        self.scroll_y = last.saturating_sub(self.content_rows.saturating_sub(1));
    }

    // ── Cursor movement ───────────────────────────────────────────────────────

    fn line_len(&self, line: usize) -> usize {
        self.lines.get(line).map(|l| l.chars().count()).unwrap_or(0)
    }

    fn move_up(&mut self) {
        if self.cursor.0 == 0 {
            return;
        }
        self.cursor.0 -= 1;
        self.cursor.1 = self.cursor.1.min(self.line_len(self.cursor.0));
        self.scroll_cursor_into_view();
    }

    fn move_down(&mut self) {
        if self.cursor.0 + 1 >= self.lines.len() {
            return;
        }
        self.cursor.0 += 1;
        self.cursor.1 = self.cursor.1.min(self.line_len(self.cursor.0));
        self.scroll_cursor_into_view();
    }

    fn move_left(&mut self) {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
            self.scroll_x_into_view();
        } else if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
            self.cursor.1 = self.line_len(self.cursor.0);
            self.scroll_cursor_into_view();
        }
    }

    fn move_right(&mut self) {
        let len = self.line_len(self.cursor.0);
        if self.cursor.1 < len {
            self.cursor.1 += 1;
            self.scroll_x_into_view();
        } else if self.cursor.0 + 1 < self.lines.len() {
            self.cursor.0 += 1;
            self.cursor.1 = 0;
            self.scroll_cursor_into_view();
        }
    }

    fn page_up(&mut self) {
        let half = (self.content_rows / 2).max(1);
        self.cursor.0 = self.cursor.0.saturating_sub(half);
        self.cursor.1 = self.cursor.1.min(self.line_len(self.cursor.0));
        self.recenter_scroll();
    }

    fn page_down(&mut self) {
        let half = (self.content_rows / 2).max(1);
        let last = self.lines.len().saturating_sub(1);
        self.cursor.0 = (self.cursor.0 + half).min(last);
        self.cursor.1 = self.cursor.1.min(self.line_len(self.cursor.0));
        self.recenter_scroll();
    }

    fn scroll_cursor_into_view(&mut self) {
        if self.cursor.0 < self.scroll_y {
            self.scroll_y = self.cursor.0;
        } else if self.cursor.0 >= self.scroll_y + self.content_rows {
            self.scroll_y = self.cursor.0 + 1 - self.content_rows;
        }
        self.scroll_x_into_view();
    }

    fn recenter_scroll(&mut self) {
        self.scroll_y = self.cursor.0.saturating_sub(self.content_rows / 2);
        self.scroll_x_into_view();
    }

    /// Adjust scroll_x so the cursor column is visible.
    fn scroll_x_into_view(&mut self) {
        let avail = self.avail_w();
        if avail == 0 {
            return;
        }
        if self.cursor.1 < self.scroll_x {
            self.scroll_x = self.cursor.1;
        } else if self.cursor.1 + 1 >= self.scroll_x + avail {
            // +1 accounts for the `…` indicator occupying the last display column
            // when the line overflows — scroll before the cursor lands on it.
            self.scroll_x = self.cursor.1 + 2 - avail;
        }
    }

    /// Gutter width in chars: right-aligned line number + 2-char marker.
    fn gutter_w(&self) -> usize {
        let max_dist = self.content_rows.saturating_sub(1);
        max_dist.to_string().len().max(1) + 2
    }

    /// Available content width after the gutter.
    fn avail_w(&self) -> usize {
        self.content_cols.saturating_sub(self.gutter_w())
    }

    // ── Profile cycling ───────────────────────────────────────────────────────

    fn cycle_profile(&mut self) {
        if self.profiles.len() <= 1 {
            return;
        }
        self.current_profile = (self.current_profile + 1) % self.profiles.len();
        self.anchor = None;
        self.scroll_x = 0;
        self.extraction_done = false;
        self.try_grab();
    }

    // ── Key handling ──────────────────────────────────────────────────────────

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        // Any keypress clears the transient message.
        self.message = None;

        let only_shift = key.has_modifiers(&[KeyModifier::Shift]) && key.key_modifiers.len() == 1;

        // Jump mode: typing narrows matches; label key jumps cursor.
        if let Mode::Jump { typed, labels } = self.mode.clone() {
            return self.handle_key_jump(key, typed, labels);
        }

        // Search mode.
        if let Mode::Search {
            query,
            matches,
            current,
            navigating,
        } = self.mode.clone()
        {
            return self.handle_key_search(key, query, matches, current, navigating);
        }

        // Line-jump mode: label key jumps to that line.
        if let Mode::LineJump { labels } = self.mode.clone() {
            return self.handle_key_line_jump(key, labels);
        }

        // Confirm mode: waiting for y/Esc before inserting multi-line text.
        if let Mode::Confirm { text } = self.mode.clone() {
            return match key.bare_key {
                BareKey::Char('y') if key.has_no_modifiers() => {
                    self.do_insert(text);
                    false
                }
                BareKey::Enter => {
                    self.do_insert(text);
                    false
                }
                BareKey::Esc => {
                    self.mode = Mode::Normal;
                    true
                }
                _ => true,
            };
        }

        match key.bare_key {
            BareKey::Esc => {
                // Cancel chain: confirm → selection → close.
                if self.anchor.is_some() {
                    self.anchor = None;
                    true
                } else {
                    close_self();
                    false
                }
            }
            BareKey::Up => {
                self.move_up();
                true
            }
            BareKey::Down => {
                self.move_down();
                true
            }
            BareKey::Left if only_shift => {
                // Pan viewport left 5 cols; clamps at 0.
                self.scroll_x = self.scroll_x.saturating_sub(5);
                true
            }
            BareKey::Right if only_shift => {
                // Pan viewport right 5 cols; clamps so line start stays visible.
                let max_x = self
                    .lines
                    .iter()
                    .map(|l| l.chars().count())
                    .max()
                    .unwrap_or(0)
                    .saturating_sub(self.avail_w().saturating_sub(1));
                self.scroll_x = (self.scroll_x + 5).min(max_x);
                true
            }
            BareKey::Left => {
                self.move_left();
                true
            }
            BareKey::Right => {
                self.move_right();
                true
            }
            BareKey::PageUp => {
                self.page_up();
                true
            }
            BareKey::PageDown => {
                self.page_down();
                true
            }
            BareKey::Char('g') if key.has_no_modifiers() => {
                self.cycle_profile();
                true
            }
            BareKey::Char(' ') if key.has_no_modifiers() => {
                if self.anchor.is_some() {
                    self.anchor = None;
                } else {
                    self.anchor = Some(self.cursor);
                }
                true
            }
            BareKey::Char('s') if key.has_no_modifiers() => {
                self.mode = Mode::Jump {
                    typed: String::new(),
                    labels: Vec::new(),
                };
                true
            }
            BareKey::Char('l') if key.has_no_modifiers() => {
                let labels = self.compute_line_labels();
                self.mode = Mode::LineJump { labels };
                true
            }
            // ── Word motions (work in and out of selection) ───────────────────
            BareKey::Char('w') if key.has_no_modifiers() => {
                self.motion_w(false);
                true
            }
            BareKey::Char('W') if key.has_no_modifiers() || only_shift => {
                self.motion_w(true);
                true
            }
            BareKey::Char('b') if key.has_no_modifiers() => {
                self.motion_b(false);
                true
            }
            BareKey::Char('B') if key.has_no_modifiers() || only_shift => {
                self.motion_b(true);
                true
            }
            BareKey::Char('e') if key.has_no_modifiers() => {
                self.motion_e(false);
                true
            }
            BareKey::Char('E') if key.has_no_modifiers() || only_shift => {
                self.motion_e(true);
                true
            }
            BareKey::Char('0') if key.has_no_modifiers() => {
                self.motion_line_start();
                true
            }
            BareKey::Char('$') if key.has_no_modifiers() => {
                self.motion_line_end();
                true
            }
            // `/` search — only outside selection mode.
            BareKey::Char('/') if key.has_no_modifiers() && self.anchor.is_none() => {
                self.mode = Mode::Search {
                    query: String::new(),
                    matches: Vec::new(),
                    current: 0,
                    navigating: false,
                };
                true
            }
            BareKey::Enter if only_shift => {
                self.action_insert();
                true
            }
            BareKey::Enter => {
                self.action_copy();
                true
            }
            _ => false,
        }
    }

    fn action_copy(&mut self) -> bool {
        match self.selected_text() {
            Some(text) => {
                copy_to_clipboard(&text);
                close_self();
                false
            }
            None => {
                self.message = Some("No selection — press Space to anchor".into());
                true
            }
        }
    }

    fn action_insert(&mut self) -> bool {
        let Some(text) = self.selected_text() else {
            self.message = Some("No selection — press Space to anchor".into());
            return true;
        };
        if text.contains('\n') {
            let line_count = text.lines().count();
            self.mode = Mode::Confirm { text: text.clone() };
            self.message = Some(format!(
                "Insert {} lines into pane?  y/Enter:confirm  Esc:cancel",
                line_count
            ));
            true
        } else {
            self.do_insert(text);
            false
        }
    }

    fn do_insert(&mut self, text: String) {
        if let Some(pane_id) = self.source_pane {
            write_chars_to_pane_id(&text, PaneId::Terminal(pane_id));
        }
        close_self();
    }

    // ── Jump mode ─────────────────────────────────────────────────────────────

    fn handle_key_jump(
        &mut self,
        key: KeyWithModifier,
        mut typed: String,
        labels: Vec<(usize, usize, char)>,
    ) -> bool {
        match key.bare_key {
            BareKey::Esc => {
                self.mode = Mode::Normal;
                return true;
            }
            BareKey::Backspace => {
                typed.pop();
                let labels = self.compute_jump_labels(&typed);
                self.mode = Mode::Jump { typed, labels };
                return true;
            }
            BareKey::Char(c) => {
                // If labels are showing and c matches a label → jump.
                if !labels.is_empty() {
                    if let Some(&(line, col, _)) = labels.iter().find(|&&(_, _, lc)| lc == c) {
                        self.jump_to(line, col);
                        self.mode = Mode::Normal;
                        return true;
                    }
                }
                // Otherwise append to search string and recompute.
                if !c.is_control()
                    && (key.has_no_modifiers()
                        || (key.has_modifiers(&[KeyModifier::Shift])
                            && key.key_modifiers.len() == 1))
                {
                    typed.push(c);
                    let labels = self.compute_jump_labels(&typed);
                    self.mode = Mode::Jump { typed, labels };
                    return true;
                }
            }
            _ => {}
        }
        true
    }

    fn compute_jump_labels(&self, typed: &str) -> Vec<(usize, usize, char)> {
        if typed.is_empty() {
            return Vec::new();
        }

        let typed_lower: Vec<char> = typed.to_lowercase().chars().collect();
        let tlen = typed_lower.len();

        // Search visible lines only.
        let vis_start = self.scroll_y;
        let vis_end = (self.scroll_y + self.content_rows).min(self.lines.len());

        let mut matches: Vec<(usize, usize)> = Vec::new();
        for line_idx in vis_start..vis_end {
            let chars_lower: Vec<char> = self.lines[line_idx].to_lowercase().chars().collect();
            let n = chars_lower.len();
            if tlen > n {
                continue;
            }
            for col in 0..=(n - tlen) {
                if chars_lower[col..col + tlen] == typed_lower[..] {
                    matches.push((line_idx, col));
                }
            }
        }

        if matches.is_empty() || matches.len() > self.jump_labels.len() {
            return Vec::new();
        }

        // Sort by distance from cursor (line distance weighted heavier).
        let (cline, ccol) = self.cursor;
        matches.sort_by_key(|&(line, col)| {
            let dl = (line as isize - cline as isize).unsigned_abs();
            let dc = (col as isize - ccol as isize).unsigned_abs();
            dl * 10_000 + dc
        });

        // Build label pool excluding typed chars (both cases) to prevent ambiguity.
        let exclude: std::collections::HashSet<char> = typed
            .chars()
            .flat_map(|c| [c.to_ascii_lowercase(), c.to_ascii_uppercase()])
            .collect();
        let pool: Vec<char> = self
            .jump_labels
            .iter()
            .filter(|&&c| !exclude.contains(&c))
            .copied()
            .collect();

        matches
            .into_iter()
            .zip(pool)
            .map(|((line, col), label)| (line, col, label))
            .collect()
    }

    fn jump_to(&mut self, line: usize, col: usize) {
        self.cursor = (line, col);
        self.recenter_scroll();
    }

    // ── Word motions ──────────────────────────────────────────────────────────

    fn char_at(&self, line: usize, col: usize) -> Option<char> {
        self.lines.get(line)?.chars().nth(col)
    }

    /// Advance one step in stream order, wrapping at line ends.
    fn next_pos(&self, line: usize, col: usize) -> Option<(usize, usize)> {
        if col < self.line_len(line) {
            Some((line, col + 1))
        } else if line + 1 < self.lines.len() {
            Some((line + 1, 0))
        } else {
            None
        }
    }

    /// Retreat one step in stream order, wrapping at line starts.
    fn prev_pos(&self, line: usize, col: usize) -> Option<(usize, usize)> {
        if col > 0 {
            Some((line, col - 1))
        } else if line > 0 {
            Some((line - 1, self.line_len(line - 1)))
        } else {
            None
        }
    }

    /// Character class at a position. EOL (col == line_len) counts as Space.
    /// `wide = true` → WORD mode: only Space vs NonSpace.
    /// `wide = false` → word mode: Space | Word | Other.
    fn cclass(&self, line: usize, col: usize, wide: bool) -> u8 {
        match self.char_at(line, col) {
            None => 0, // EOL = space
            Some(c) if c.is_whitespace() => 0,
            Some(_) if wide => 1,
            Some(c) if c.is_alphanumeric() || c == '_' => 1,
            Some(_) => 2,
        }
    }

    /// `w` / `W` — forward to start of next word.
    fn motion_w(&mut self, wide: bool) {
        let (mut line, mut col) = self.cursor;
        let start = self.cclass(line, col, wide);
        // Skip current class run.
        loop {
            let Some((nl, nc)) = self.next_pos(line, col) else {
                break;
            };
            (line, col) = (nl, nc);
            if self.cclass(line, col, wide) != start {
                break;
            }
        }
        // Skip spaces.
        while self.cclass(line, col, wide) == 0 {
            let Some((nl, nc)) = self.next_pos(line, col) else {
                break;
            };
            (line, col) = (nl, nc);
        }
        self.cursor = (line, col);
        self.scroll_cursor_into_view();
    }

    /// `b` / `B` — backward to start of previous word.
    fn motion_b(&mut self, wide: bool) {
        let (mut line, mut col) = self.cursor;
        // Retreat one step first.
        let Some((nl, nc)) = self.prev_pos(line, col) else {
            return;
        };
        (line, col) = (nl, nc);
        // Skip spaces backward.
        while self.cclass(line, col, wide) == 0 {
            let Some((nl, nc)) = self.prev_pos(line, col) else {
                break;
            };
            (line, col) = (nl, nc);
        }
        // Skip same-class run backward to find its start.
        let target = self.cclass(line, col, wide);
        loop {
            let Some((nl, nc)) = self.prev_pos(line, col) else {
                break;
            };
            if self.cclass(nl, nc, wide) == target {
                (line, col) = (nl, nc);
            } else {
                break;
            }
        }
        self.cursor = (line, col);
        self.scroll_cursor_into_view();
    }

    /// `e` / `E` — forward to end of current / next word.
    fn motion_e(&mut self, wide: bool) {
        let (mut line, mut col) = self.cursor;
        // Advance one step first.
        let Some((nl, nc)) = self.next_pos(line, col) else {
            return;
        };
        (line, col) = (nl, nc);
        // Skip spaces.
        while self.cclass(line, col, wide) == 0 {
            let Some((nl, nc)) = self.next_pos(line, col) else {
                break;
            };
            (line, col) = (nl, nc);
        }
        // Advance through current class run until class changes.
        let target = self.cclass(line, col, wide);
        loop {
            let Some((nl, nc)) = self.next_pos(line, col) else {
                break;
            };
            if self.cclass(nl, nc, wide) == target {
                (line, col) = (nl, nc);
            } else {
                break;
            }
        }
        self.cursor = (line, col);
        self.scroll_cursor_into_view();
    }

    /// `0` — start of line.
    fn motion_line_start(&mut self) {
        self.cursor.1 = 0;
        self.scroll_x_into_view();
    }

    /// `$` — end of line (last char, not past it).
    fn motion_line_end(&mut self) {
        let len = self.line_len(self.cursor.0);
        self.cursor.1 = len.saturating_sub(1);
        self.scroll_x_into_view();
    }

    // ── Search mode ───────────────────────────────────────────────────────────

    fn compute_search_matches(&self, query: &str) -> Vec<(usize, usize)> {
        if query.is_empty() {
            return Vec::new();
        }
        let q: Vec<char> = query.to_lowercase().chars().collect();
        let qlen = q.len();
        let mut out = Vec::new();
        for (li, line) in self.lines.iter().enumerate() {
            let lc: Vec<char> = line.to_lowercase().chars().collect();
            if lc.len() < qlen {
                continue;
            }
            for col in 0..=(lc.len() - qlen) {
                if lc[col..col + qlen] == q[..] {
                    out.push((li, col));
                }
            }
        }
        out
    }

    /// Index of first match at or after cursor, wrapping to 0.
    fn search_current_from_cursor(&self, matches: &[(usize, usize)]) -> usize {
        let (cl, cc) = self.cursor;
        matches
            .iter()
            .position(|&(ml, mc)| ml > cl || (ml == cl && mc >= cc))
            .unwrap_or(0)
    }

    fn handle_key_search(
        &mut self,
        key: KeyWithModifier,
        mut query: String,
        mut matches: Vec<(usize, usize)>,
        mut current: usize,
        navigating: bool,
    ) -> bool {
        let only_shift = key.has_modifiers(&[KeyModifier::Shift]) && key.key_modifiers.len() == 1;

        if navigating {
            // Navigation phase: n/N move between matches, anything else exits.
            match key.bare_key {
                BareKey::Esc => {
                    self.mode = Mode::Normal;
                    true
                }
                BareKey::Char(' ') if key.has_no_modifiers() => {
                    // Exit search and anchor selection at current match start.
                    self.anchor = Some(self.cursor);
                    self.mode = Mode::Normal;
                    true
                }
                BareKey::Char('n') if key.has_no_modifiers() => {
                    if !matches.is_empty() {
                        current = (current + 1) % matches.len();
                        self.jump_search_cursor(&matches, current);
                    }
                    self.mode = Mode::Search {
                        query,
                        matches,
                        current,
                        navigating: true,
                    };
                    true
                }
                BareKey::Char('N') if key.has_no_modifiers() || only_shift => {
                    if !matches.is_empty() {
                        current = (current + matches.len() - 1) % matches.len();
                        self.jump_search_cursor(&matches, current);
                    }
                    self.mode = Mode::Search {
                        query,
                        matches,
                        current,
                        navigating: true,
                    };
                    true
                }
                _ => {
                    // Any other key exits search, stays at current match.
                    self.mode = Mode::Normal;
                    true
                }
            }
        } else {
            // Input phase: type freely; Enter confirms, Esc cancels.
            match key.bare_key {
                BareKey::Esc => {
                    self.mode = Mode::Normal;
                    true
                }
                BareKey::Enter => {
                    // Commit query — switch to navigation phase.
                    current = self.search_current_from_cursor(&matches);
                    self.jump_search_cursor(&matches, current);
                    self.mode = Mode::Search {
                        query,
                        matches,
                        current,
                        navigating: true,
                    };
                    true
                }
                BareKey::Backspace => {
                    query.pop();
                    matches = self.compute_search_matches(&query);
                    current = self.search_current_from_cursor(&matches);
                    self.jump_search_cursor(&matches, current);
                    self.mode = Mode::Search {
                        query,
                        matches,
                        current,
                        navigating: false,
                    };
                    true
                }
                BareKey::Char(c) if !c.is_control() && (key.has_no_modifiers() || only_shift) => {
                    query.push(c);
                    matches = self.compute_search_matches(&query);
                    current = self.search_current_from_cursor(&matches);
                    self.jump_search_cursor(&matches, current);
                    self.mode = Mode::Search {
                        query,
                        matches,
                        current,
                        navigating: false,
                    };
                    true
                }
                _ => true,
            }
        }
    }

    fn jump_search_cursor(&mut self, matches: &[(usize, usize)], current: usize) {
        if let Some(&(line, col)) = matches.get(current) {
            self.cursor = (line, col);
            self.recenter_scroll();
        }
    }

    // ── Line-jump mode ────────────────────────────────────────────────────────

    fn compute_line_labels(&self) -> Vec<(usize, char)> {
        let vis_start = self.scroll_y;
        let vis_end = (self.scroll_y + self.content_rows).min(self.lines.len());
        let (cline, _) = self.cursor;

        let below = (vis_start..vis_end).filter(|&l| l > cline);
        let above = (vis_start..cline.min(vis_end)).rev();

        let mut labels: Vec<(usize, char)> = Vec::new();

        if self.line_labels_unified {
            // Split jump_labels in half: first half → below, second half → above.
            let n = self.jump_labels.len();
            let mid = (n + 1) / 2; // first half gets the extra char if odd
            let below_pool = &self.jump_labels[..mid];
            let above_pool = &self.jump_labels[mid..];
            for (line, &lc) in below.zip(below_pool.iter()) {
                labels.push((line, lc));
            }
            for (line, &lc) in above.zip(above_pool.iter()) {
                labels.push((line, lc));
            }
        } else {
            // Default directional scheme: a-z below (nearest = a), A-Z above (nearest = A).
            for (line, lc) in below.zip('a'..='z') {
                labels.push((line, lc));
            }
            for (line, lc) in above.zip('A'..='Z') {
                labels.push((line, lc));
            }
        }
        labels
    }

    fn handle_key_line_jump(&mut self, key: KeyWithModifier, labels: Vec<(usize, char)>) -> bool {
        match key.bare_key {
            BareKey::Esc => {
                self.mode = Mode::Normal;
                true
            }
            BareKey::Char(c) => {
                if let Some(&(line, _)) = labels.iter().find(|&&(_, lc)| lc == c) {
                    // Preserve col if it fits on the target line, else clamp to 0.
                    let col = self.cursor.1.min(self.line_len(line));
                    self.jump_to(line, col);
                    self.mode = Mode::Normal;
                } else {
                    self.mode = Mode::Normal;
                }
                true
            }
            _ => {
                self.mode = Mode::Normal;
                true
            }
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    fn render_all(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 5 {
            Paragraph::new("too small")
                .style(Style::default().fg(self.theme.footer_dim))
                .render(area, buf);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(4)])
            .split(area);

        self.render_content(chunks[0], buf);
        self.render_footer(chunks[1], buf);
    }

    fn render_content(&self, area: Rect, buf: &mut Buffer) {
        let inner = area;

        if self.lines.is_empty() {
            Paragraph::new(if self.extraction_done {
                "No content captured."
            } else {
                "Loading…"
            })
            .style(Style::default().fg(self.theme.footer_dim))
            .render(inner, buf);
            return;
        }

        let viewport_h = inner.height as usize;
        let total = self.lines.len();
        let cursor_line = self.cursor.0.min(total.saturating_sub(1));
        let cursor_col = self.cursor.1;

        let scroll_y = self.scroll_y.min(total.saturating_sub(1));
        let visible_end = (scroll_y + viewport_h).min(total);
        let visible = &self.lines[scroll_y..visible_end];

        let max_dist = viewport_h.saturating_sub(1);
        let num_w = max_dist.to_string().len().max(1);
        let gutter_w = num_w + 2;
        let avail_w = (inner.width as usize).saturating_sub(gutter_w);

        let sel = self.selection_range();

        // Collect jump labels for this frame (empty when not in Jump mode).
        let jump_labels: &[(usize, usize, char)] = if let Mode::Jump { ref labels, .. } = self.mode
        {
            labels
        } else {
            &[]
        };

        // Line-jump labels: (line_idx, label_char) for gutter replacement.
        let line_jump_labels: &[(usize, char)] = if let Mode::LineJump { ref labels } = self.mode {
            labels
        } else {
            &[]
        };

        // Search highlights: collect (line, col, is_current) from Search mode.
        let (search_all, search_current_idx, search_qlen) = if let Mode::Search {
            ref matches,
            current,
            ref query,
            ..
        } = self.mode
        {
            (matches.as_slice(), current, query.chars().count())
        } else {
            (&[][..], 0, 0)
        };

        let gutter_dim = Style::default()
            .fg(self.theme.gutter_dim)
            .add_modifier(Modifier::DIM);
        let gutter_cursor_style = Style::default()
            .fg(self.theme.gutter_cursor)
            .add_modifier(Modifier::BOLD);

        let content_lines: Vec<Line<'static>> = visible
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let abs = scroll_y + i;
                let is_cursor_line = abs == cursor_line;
                let dist = (abs as isize - cursor_line as isize).unsigned_abs();

                // In LineJump mode, replace the gutter number with the label.
                let (gutter_str, gutter_style) =
                    if let Some(&(_, lc)) = line_jump_labels.iter().find(|&&(l, _)| l == abs) {
                        (
                            format!("{:>w$}  ", lc, w = num_w),
                            Style::default()
                                .bg(self.theme.jump_label_bg)
                                .fg(self.theme.jump_label_fg)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        (
                            format!(
                                "{:>w$}{}",
                                dist,
                                if is_cursor_line { "► " } else { "  " },
                                w = num_w
                            ),
                            if is_cursor_line {
                                gutter_cursor_style
                            } else {
                                gutter_dim
                            },
                        )
                    };
                let gutter = Span::styled(gutter_str, gutter_style);

                let scroll_x = self.scroll_x;
                let logical_len = text.chars().count();
                let has_right_overflow = logical_len > scroll_x + avail_w;
                let has_left_overflow = scroll_x > 0;

                // Slice the line to the visible horizontal window.
                // Reserve 1 char on the right for `…` when content overflows.
                let visible_w = if has_right_overflow {
                    avail_w.saturating_sub(1)
                } else {
                    avail_w
                };
                let chars: Vec<char> = text.chars().skip(scroll_x).take(visible_w).collect();

                // Convert logical selection/cursor coords → display coords (relative to scroll_x).
                let raw_sel = sel.and_then(|(s, e)| sel_range_for_line(s, e, abs, logical_len));
                let sel_range =
                    raw_sel.map(|(s, e)| (s.saturating_sub(scroll_x), e.saturating_sub(scroll_x)));
                let cur_col = if is_cursor_line {
                    Some(cursor_col.saturating_sub(scroll_x))
                } else {
                    None
                };

                let typed_len = if let Mode::Jump { ref typed, .. } = self.mode {
                    typed.chars().count()
                } else {
                    0
                };
                // Label sits on the LAST char of the matched prefix (display coords).
                let line_labels: Vec<(usize, char)> = jump_labels
                    .iter()
                    .filter(|&&(l, _, _)| l == abs)
                    .filter_map(|&(_, col, lc)| {
                        let disp = (col + typed_len.saturating_sub(1)).saturating_sub(scroll_x);
                        if disp < visible_w {
                            Some((disp, lc))
                        } else {
                            None
                        }
                    })
                    .collect();

                // Search matches on this line in display coords (col, is_current).
                let line_search: Vec<(usize, bool)> = search_all
                    .iter()
                    .enumerate()
                    .filter(|(_, &(ml, _))| ml == abs)
                    .filter_map(|(i, &(_, mc))| {
                        let dc = mc.saturating_sub(scroll_x);
                        if dc < visible_w {
                            Some((dc, i == search_current_idx))
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut spans = vec![gutter];
                if has_left_overflow {
                    spans.push(Span::styled(
                        "…",
                        Style::default().fg(self.theme.footer_dim),
                    ));
                }
                spans.extend(build_line_spans(
                    &chars,
                    sel_range,
                    cur_col,
                    &line_labels,
                    typed_len,
                    &line_search,
                    search_qlen,
                    &self.theme,
                ));
                if has_right_overflow {
                    spans.push(Span::styled(
                        "…",
                        Style::default().fg(self.theme.footer_dim),
                    ));
                }
                Line::from(spans)
            })
            .collect();

        Paragraph::new(content_lines).render(inner, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let bold = Style::default()
            .fg(self.theme.footer_key)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(self.theme.footer_dim);
        let sel_style = Style::default()
            .fg(self.theme.sel_indicator)
            .add_modifier(Modifier::BOLD);

        let profile_label = self
            .profiles
            .get(self.current_profile)
            .map(|p| p.label())
            .unwrap_or_else(|| "?".to_string());

        let (cline, ccol) = self.cursor;
        let pos_str = if self.scroll_x > 0 {
            format!("{}:{}  +{}", cline + 1, ccol + 1, self.scroll_x)
        } else {
            format!("{}:{}", cline + 1, ccol + 1)
        };

        // Status line: profile, line count, cursor pos, h-scroll offset, selection info.
        let mut line1_spans = vec![
            Span::raw(" "),
            Span::styled(format!("[{}]", profile_label), dim),
            Span::raw("  "),
            Span::styled(format!("{} lines", self.lines.len()), dim),
            Span::raw("  "),
            Span::styled(pos_str, dim),
        ];
        if let Some((nlines, nchars)) = self.selection_info() {
            line1_spans.push(Span::raw("  "));
            line1_spans.push(Span::styled(
                format!("SEL {} lines {} chars", nlines, nchars),
                sel_style,
            ));
        }
        let line1 = Line::from(line1_spans);

        let line2 = if let Mode::Search {
            query,
            matches,
            current,
            navigating,
        } = &self.mode
        {
            let count_str = if matches.is_empty() && !query.is_empty() {
                "  (no matches)".to_string()
            } else if !matches.is_empty() {
                format!("  {}/{}", current + 1, matches.len())
            } else {
                String::new()
            };
            if *navigating {
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        format!("/{query}{count_str}"),
                        Style::default()
                            .fg(self.theme.search_current_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled("n", bold),
                    Span::raw(":next  "),
                    Span::styled("N", bold),
                    Span::raw(":prev  "),
                    Span::styled("Space", bold),
                    Span::raw(":select  "),
                    Span::styled("Esc", bold),
                    Span::raw(":done"),
                ])
            } else {
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        format!("/{query}█{count_str}"),
                        Style::default()
                            .fg(self.theme.search_current_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled("Enter", bold),
                    Span::raw(":confirm  "),
                    Span::styled("Esc", bold),
                    Span::raw(":cancel"),
                ])
            }
        } else if let Mode::LineJump { labels } = &self.mode {
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("line jump — {} lines labeled", labels.len()),
                    Style::default()
                        .fg(self.theme.jump_label_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled("Esc", bold),
                Span::raw(":cancel"),
            ])
        } else if let Mode::Jump { typed, labels } = &self.mode {
            let hint = if labels.is_empty() && !typed.is_empty() {
                format!("jump: {}  (no matches)", typed)
            } else if labels.is_empty() {
                "jump: type to search…".to_string()
            } else {
                format!("jump: {}  ({} matches)", typed, labels.len())
            };
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    hint,
                    Style::default()
                        .fg(self.theme.jump_label_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled("Esc", bold),
                Span::raw(":cancel"),
            ])
        } else if let Mode::Confirm { .. } = &self.mode {
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    self.message.clone().unwrap_or_default(),
                    Style::default()
                        .fg(self.theme.sel_indicator)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        } else if self.anchor.is_some() {
            let mut spans = vec![
                Span::raw(" "),
                Span::styled("↑↓←→", bold),
                Span::raw(":extend  "),
                Span::styled("Enter", bold),
                Span::raw(":copy  "),
                Span::styled("⇧Enter", bold),
                Span::raw(":insert  "),
                Span::styled("Space", bold),
                Span::raw(":clear-sel  "),
                Span::styled("Esc", bold),
                Span::raw(":clear-sel"),
            ];
            if let Some(msg) = &self.message {
                spans.push(Span::raw("    "));
                spans.push(Span::styled(
                    msg.clone(),
                    Style::default().fg(self.theme.sel_indicator),
                ));
            }
            Line::from(spans)
        } else {
            let mut spans = vec![
                Span::raw(" "),
                Span::styled("wWeEbB0$", bold),
                Span::raw(":word  "),
                Span::styled("/", bold),
                Span::raw(":search  "),
                Span::styled("s", bold),
                Span::raw(":jump  "),
                Span::styled("l", bold),
                Span::raw(":line  "),
                Span::styled("Space", bold),
                Span::raw(":select  "),
                Span::styled("g", bold),
                Span::raw(":depth  "),
                Span::styled("Enter", bold),
                Span::raw(":copy  "),
                Span::styled("Esc", bold),
                Span::raw(":close"),
            ];
            if let Some(msg) = &self.message {
                spans.push(Span::raw("    "));
                spans.push(Span::styled(
                    msg.clone(),
                    Style::default().fg(self.theme.sel_indicator),
                ));
            }
            Line::from(spans)
        };

        Paragraph::new(vec![line1, line2])
            .block(Block::default().borders(Borders::ALL))
            .render(area, buf);
    }

    // ── Selection helpers ─────────────────────────────────────────────────────

    /// Normalized selection range: (start, end) where start ≤ end in stream
    /// order. Returns None when no anchor is set.
    fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.anchor?;
        let cursor = self.cursor;
        if anchor <= cursor {
            Some((anchor, cursor))
        } else {
            Some((cursor, anchor))
        }
    }

    /// Extract the selected text from `lines` as a plain string with newlines.
    fn selected_text(&self) -> Option<String> {
        let ((sl, sc), (el, ec)) = self.selection_range()?;

        if sl == el {
            let chars: Vec<char> = self.lines.get(sl)?.chars().collect();
            let start = sc.min(chars.len());
            let end = (ec + 1).min(chars.len());
            return Some(chars[start..end].iter().collect());
        }

        let mut out = String::new();
        if let Some(line) = self.lines.get(sl) {
            let chars: Vec<char> = line.chars().collect();
            out.extend(chars[sc.min(chars.len())..].iter());
            out.push('\n');
        }
        for l in sl + 1..el {
            if let Some(line) = self.lines.get(l) {
                out.push_str(line);
                out.push('\n');
            }
        }
        if let Some(line) = self.lines.get(el) {
            let chars: Vec<char> = line.chars().collect();
            out.extend(chars[..(ec + 1).min(chars.len())].iter());
        }
        Some(out)
    }

    /// Returns (line_count, char_count) for the active selection, or None.
    fn selection_info(&self) -> Option<(usize, usize)> {
        let ((sl, sc), (el, ec)) = self.selection_range()?;
        let lines = el - sl + 1;
        let chars = if sl == el {
            ec.saturating_sub(sc) + 1
        } else {
            let first = self.line_len(sl).saturating_sub(sc) + 1; // +1 for newline
            let last = ec + 1;
            let mid: usize = (sl + 1..el).map(|l| self.line_len(l) + 1).sum();
            first + mid + last
        };
        Some((lines, chars))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Compute centered x for a percentage width string, e.g. "90%" → Some("5%").
/// Returns None for non-percentage or unusual values.
fn center_x_for_width(width: &str) -> Option<String> {
    let pct: u32 = width.strip_suffix('%')?.parse().ok()?;
    if pct >= 100 {
        return Some("0%".to_string());
    }
    Some(format!("{}%", (100 - pct) / 2))
}

/// Selection col range for a single visible line, given the normalized
/// selection (start, end). Returns None if this line is outside the selection.
/// Returned range is (sel_start_col, sel_end_col) in char indices, inclusive.
fn sel_range_for_line(
    start: (usize, usize),
    end: (usize, usize),
    line: usize,
    line_len: usize,
) -> Option<(usize, usize)> {
    let (sl, sc) = start;
    let (el, ec) = end;
    if line < sl || line > el {
        return None;
    }
    let col_start = if line == sl { sc } else { 0 };
    let col_end = if line == el {
        ec
    } else {
        line_len.saturating_sub(1)
    };
    Some((col_start, col_end))
}

/// Build ratatui spans for one line of content.
///
/// Priority per character cell (highest wins):
///   1. Jump label position   → label char + label style
///   2. Cursor cell           → cursor style (inverted)
///   3. Selection range       → selection style
///   4. Jump match highlight  → match chars after label dimly highlighted
///   5. Normal text
///
/// `line_labels`: (col, label_char) pairs for labels on this line.
/// `typed_len`: length of the current jump search string (chars after the
///   label are the matched prefix and get a dim highlight).
#[allow(clippy::too_many_arguments)]
fn build_line_spans(
    chars: &[char],
    sel: Option<(usize, usize)>,
    cursor_col: Option<usize>,
    line_labels: &[(usize, char)],
    typed_len: usize,
    search_matches: &[(usize, bool)], // (display_col, is_current)
    search_len: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let sel_style = Style::default().bg(theme.sel_bg).fg(theme.sel_fg);
    let cursor_style = Style::default().bg(theme.cursor_bg).fg(theme.cursor_fg);
    let label_style = Style::default()
        .bg(theme.jump_label_bg)
        .fg(theme.jump_label_fg)
        .add_modifier(Modifier::BOLD);
    let match_style = Style::default()
        .fg(theme.jump_match_fg)
        .add_modifier(Modifier::BOLD);
    let search_style = Style::default()
        .bg(theme.search_match_bg)
        .fg(theme.search_fg);
    let search_cur_style = Style::default()
        .bg(theme.search_current_bg)
        .fg(theme.search_fg)
        .add_modifier(Modifier::BOLD);

    let mut cells: Vec<(char, Style)> = chars
        .iter()
        .enumerate()
        .map(|(i, &ch)| {
            // 1. Jump label (highest).
            if let Some(&(_, lc)) = line_labels.iter().find(|&&(lc, _)| lc == i) {
                return (lc, label_style);
            }
            // 2. Jump prefix match chars.
            let in_jump_match = typed_len > 1
                && line_labels.iter().any(|&(label_col, _)| {
                    let start = label_col.saturating_sub(typed_len - 1);
                    i >= start && i < label_col
                });
            if in_jump_match {
                return (ch, match_style);
            }
            // 3. Cursor.
            if cursor_col == Some(i) {
                return (ch, cursor_style);
            }
            // 4. Current search match.
            if search_len > 0 {
                for &(mc, is_cur) in search_matches {
                    if i >= mc && i < mc + search_len {
                        return (
                            ch,
                            if is_cur {
                                search_cur_style
                            } else {
                                search_style
                            },
                        );
                    }
                }
            }
            // 5. Selection.
            if let Some((s, e)) = sel {
                if i >= s && i <= e {
                    return (ch, sel_style);
                }
            }
            (ch, Style::default())
        })
        .collect();

    // Cursor or selection past end of line.
    let past_end = cursor_col.map(|c| c >= chars.len()).unwrap_or(false);
    if past_end {
        let style = if let Some((s, e)) = sel {
            if chars.len() >= s && chars.len() <= e {
                sel_style
            } else {
                cursor_style
            }
        } else {
            cursor_style
        };
        cells.push((' ', style));
    }

    // Merge consecutive cells with the same style into spans.
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::default();

    for (ch, style) in cells {
        if style != run_style {
            if !run.is_empty() {
                spans.push(Span::styled(run.clone(), run_style));
                run.clear();
            }
            run_style = style;
        }
        run.push(ch);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }
    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    spans
}
