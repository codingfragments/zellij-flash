mod render;
mod source_pane;

use std::collections::BTreeMap;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use zellij_tile::prelude::*;

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
                p.parse::<usize>().ok().filter(|&n| n > 0).map(Profile::Lines)
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
    /// Logical cursor position: (line index, char col) into `lines`.
    cursor: (usize, usize),
    /// Selection anchor. When Some, the selection spans from anchor to cursor
    /// (order-independent). None means no active selection.
    anchor: Option<(usize, usize)>,
    /// Index of the first visible line in the content viewport.
    scroll_y: usize,
    /// Content area height — updated each render, used for half-page math.
    content_rows: usize,
    /// Size string from keybind config ("90%x85%"), applied once on open.
    pending_size: Option<String>,
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
            cursor: (0, 0),
            anchor: None,
            scroll_y: 0,
            content_rows: 24,
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

        // Store size string for use once ChangeApplicationState is granted.
        if let Some(size) = configuration.get("size") {
            self.pending_size = Some(size.clone());
        }
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
                let active_panes: Box<dyn Iterator<Item = &PaneInfo>> =
                    match self.active_tab_index {
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
        let Some(ref size_str) = self.pending_size else { return };
        let parts: Vec<&str> = size_str.splitn(2, 'x').collect();
        if parts.len() != 2 { return; }
        let width = parts[0].trim().to_string();
        let height = parts[1].trim().to_string();
        let x = center_x_for_width(&width);
        if let Some(coords) = FloatingPaneCoordinates::new(
            x,
            None,
            Some(width),
            Some(height),
            None,
            None,
        ) {
            change_floating_panes_coordinates(vec![(
                PaneId::Plugin(self.own_plugin_id),
                coords,
            )]);
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

        let profile = self.profiles.get(self.current_profile).copied()
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
        if self.cursor.0 == 0 { return; }
        self.cursor.0 -= 1;
        self.cursor.1 = self.cursor.1.min(self.line_len(self.cursor.0));
        self.scroll_cursor_into_view();
    }

    fn move_down(&mut self) {
        if self.cursor.0 + 1 >= self.lines.len() { return; }
        self.cursor.0 += 1;
        self.cursor.1 = self.cursor.1.min(self.line_len(self.cursor.0));
        self.scroll_cursor_into_view();
    }

    fn move_left(&mut self) {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
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
    }

    fn recenter_scroll(&mut self) {
        self.scroll_y = self.cursor.0.saturating_sub(self.content_rows / 2);
    }

    // ── Profile cycling ───────────────────────────────────────────────────────

    fn cycle_profile(&mut self) {
        if self.profiles.len() <= 1 { return; }
        self.current_profile = (self.current_profile + 1) % self.profiles.len();
        self.anchor = None;
        self.extraction_done = false;
        self.try_grab();
    }

    // ── Key handling ──────────────────────────────────────────────────────────

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Esc => {
                // Cancel chain: selection first, then close.
                if self.anchor.is_some() {
                    self.anchor = None;
                    true
                } else {
                    close_self();
                    false
                }
            }
            BareKey::Up        => { self.move_up();    true }
            BareKey::Down      => { self.move_down();  true }
            BareKey::Left      => { self.move_left();  true }
            BareKey::Right     => { self.move_right(); true }
            BareKey::PageUp    => { self.page_up();    true }
            BareKey::PageDown  => { self.page_down();  true }
            BareKey::Char('g') if key.has_no_modifiers() => { self.cycle_profile(); true }
            BareKey::Char(' ') if key.has_no_modifiers() => {
                if self.anchor.is_some() {
                    self.anchor = None;
                } else {
                    self.anchor = Some(self.cursor);
                }
                true
            }
            _ => false,
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    fn render_all(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 5 {
            Paragraph::new("too small")
                .style(Style::default().fg(Color::DarkGray))
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
            .style(Style::default().fg(Color::DarkGray))
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

        let gutter_dim = Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM);
        let gutter_cursor_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

        let content_lines: Vec<Line<'static>> = visible
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let abs = scroll_y + i;
                let is_cursor_line = abs == cursor_line;
                let dist = (abs as isize - cursor_line as isize).unsigned_abs();

                let gutter_str = format!(
                    "{:>w$}{}",
                    dist,
                    if is_cursor_line { "► " } else { "  " },
                    w = num_w
                );
                let gutter = Span::styled(
                    gutter_str,
                    if is_cursor_line { gutter_cursor_style } else { gutter_dim },
                );

                let chars: Vec<char> = text.chars().take(avail_w).collect();
                let sel_range = sel.and_then(|(s, e)| sel_range_for_line(s, e, abs, chars.len()));
                let cur_col = if is_cursor_line { Some(cursor_col) } else { None };

                let mut spans = vec![gutter];
                spans.extend(build_line_spans(&chars, sel_range, cur_col));
                Line::from(spans)
            })
            .collect();

        Paragraph::new(content_lines).render(inner, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::DarkGray);
        let sel_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);

        let profile_label = self.profiles
            .get(self.current_profile)
            .map(|p| p.label())
            .unwrap_or_else(|| "?".to_string());

        let (cline, ccol) = self.cursor;

        // Status line: profile, line count, cursor pos, selection info.
        let mut line1_spans = vec![
            Span::raw(" "),
            Span::styled(format!("[{}]", profile_label), dim),
            Span::raw("  "),
            Span::styled(format!("{} lines", self.lines.len()), dim),
            Span::raw("  "),
            Span::styled(format!("{}:{}", cline + 1, ccol + 1), dim),
        ];
        if let Some((nlines, nchars)) = self.selection_info() {
            line1_spans.push(Span::raw("  "));
            line1_spans.push(Span::styled(
                format!("SEL {} lines {} chars", nlines, nchars),
                sel_style,
            ));
        }
        let line1 = Line::from(line1_spans);

        let line2 = if self.anchor.is_some() {
            Line::from(vec![
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
            ])
        } else {
            Line::from(vec![
                Span::raw(" "),
                Span::styled("↑↓←→", bold),
                Span::raw(":move  "),
                Span::styled("PgUp/Dn", bold),
                Span::raw(":half-page  "),
                Span::styled("Space", bold),
                Span::raw(":select  "),
                Span::styled("g", bold),
                Span::raw(":depth  "),
                Span::styled("Enter", bold),
                Span::raw(":copy  "),
                Span::styled("Esc", bold),
                Span::raw(":close"),
            ])
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
    if pct >= 100 { return Some("0%".to_string()); }
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
    if line < sl || line > el { return None; }
    let col_start = if line == sl { sc } else { 0 };
    let col_end   = if line == el { ec } else { line_len.saturating_sub(1) };
    Some((col_start, col_end))
}

/// Build ratatui spans for one line of content, applying selection highlight
/// and cursor cell. Selection range is (start_col, end_col) inclusive in
/// char indices. cursor_col is Some only for the cursor line.
fn build_line_spans(
    chars: &[char],
    sel: Option<(usize, usize)>,
    cursor_col: Option<usize>,
) -> Vec<Span<'static>> {
    let sel_style    = Style::default().bg(Color::Blue).fg(Color::White);
    let cursor_style = Style::default().bg(Color::White).fg(Color::Black);

    let char_style = |i: usize| -> Style {
        if cursor_col == Some(i) {
            cursor_style
        } else if let Some((s, e)) = sel {
            if i >= s && i <= e { sel_style } else { Style::default() }
        } else {
            Style::default()
        }
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::default();

    for (i, &ch) in chars.iter().enumerate() {
        let s = char_style(i);
        if s != run_style {
            if !run.is_empty() {
                spans.push(Span::styled(run.clone(), run_style));
                run.clear();
            }
            run_style = s;
        }
        run.push(ch);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }

    // Cursor past end of line (empty line, or cursor at line_len position).
    let past_end = cursor_col.map(|c| c >= chars.len()).unwrap_or(false);
    if past_end {
        // If the past-end position is also inside the selection, show it selected.
        let style = if let Some((s, e)) = sel {
            if chars.len() >= s && chars.len() <= e { sel_style } else { cursor_style }
        } else {
            cursor_style
        };
        spans.push(Span::styled(" ", style));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    spans
}
