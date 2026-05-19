mod render;
mod source_pane;

use std::collections::BTreeMap;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use zellij_tile::prelude::*;

const DEFAULT_LINES: usize = 200;

struct State {
    source_pane: Option<u32>,
    last_focused_non_plugin: Option<u32>,
    active_tab_index: Option<usize>,
    own_plugin_id: u32,
    lines: Vec<String>,
    extraction_done: bool,
    /// Logical cursor position: (line index, char col) into `lines`.
    cursor: (usize, usize),
    /// Index of the first visible line in the content viewport.
    scroll_y: usize,
    /// Content area height in rows — updated each render, used by key handlers
    /// to compute half-page jumps without passing rows through the call chain.
    content_rows: usize,
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
            cursor: (0, 0),
            scroll_y: 0,
            content_rows: 24,
            render_buffer: None,
        }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
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
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(_) => {
                rename_plugin_pane(self.own_plugin_id, "zellij-flash");
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
                // Update hint BEFORE pick — the terminal pane briefly appears
                // focused in the transitional PaneUpdate as the plugin opens.
                // Missing this window means falling back to a lower-priority tier.
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

        // Footer = 2 content lines + 2 border lines = 4 rows.
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
    fn try_grab(&mut self) {
        if self.extraction_done {
            return;
        }
        let Some(source) = self.source_pane else {
            return;
        };

        let Ok(contents) = get_pane_scrollback(PaneId::Terminal(source), true) else {
            return;
        };

        let mut all: Vec<String> = contents
            .lines_above_viewport
            .into_iter()
            .chain(contents.viewport)
            .collect();

        if all.len() > DEFAULT_LINES {
            let start = all.len() - DEFAULT_LINES;
            all.drain(..start);
        }

        self.lines = all;
        self.extraction_done = true;

        // Place cursor at the last line, col 0.
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
        } else if self.cursor.0 > 0 {
            // Wrap to end of previous line.
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
            // Wrap to start of next line.
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

    /// Keep scroll_y minimal to show the cursor — called after single-line moves.
    fn scroll_cursor_into_view(&mut self) {
        if self.cursor.0 < self.scroll_y {
            self.scroll_y = self.cursor.0;
        } else if self.cursor.0 >= self.scroll_y + self.content_rows {
            self.scroll_y = self.cursor.0 + 1 - self.content_rows;
        }
    }

    /// Center scroll_y on the cursor — called after half-page jumps.
    fn recenter_scroll(&mut self) {
        let half = self.content_rows / 2;
        self.scroll_y = self.cursor.0.saturating_sub(half);
    }

    // ── Key handling ──────────────────────────────────────────────────────────

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Esc => {
                close_self();
                false
            }
            BareKey::Up => { self.move_up(); true }
            BareKey::Down => { self.move_down(); true }
            BareKey::Left => { self.move_left(); true }
            BareKey::Right => { self.move_right(); true }
            BareKey::PageUp => { self.page_up(); true }
            BareKey::PageDown => { self.page_down(); true }
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

        // Gutter width: digits of the largest distance visible + marker chars.
        let max_dist = viewport_h.saturating_sub(1);
        let num_w = max_dist.to_string().len().max(1);
        let gutter_w = num_w + 2; // number + "► " or "  "
        let avail_w = (inner.width as usize).saturating_sub(gutter_w);

        let gutter_dim = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM);
        let gutter_cursor_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let cursor_cell_style = Style::default()
            .bg(Color::White)
            .fg(Color::Black);

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

                // Build content span(s). On the cursor line split at cursor_col
                // to inject the highlighted cursor cell.
                let chars: Vec<char> = text.chars().take(avail_w).collect();

                if !is_cursor_line {
                    let s: String = chars.into_iter().collect();
                    return Line::from(vec![gutter, Span::raw(s)]);
                }

                // Cursor line: split into before / cursor-char / after.
                let col = cursor_col.min(chars.len());
                let before: String = chars[..col].iter().collect();
                let cursor_ch = chars.get(col).copied().unwrap_or(' ');
                let after: String = if col + 1 <= chars.len() {
                    chars[col + 1..].iter().collect()
                } else {
                    String::new()
                };

                let mut spans = vec![gutter];
                if !before.is_empty() {
                    spans.push(Span::raw(before));
                }
                spans.push(Span::styled(cursor_ch.to_string(), cursor_cell_style));
                if !after.is_empty() {
                    spans.push(Span::raw(after));
                }
                Line::from(spans)
            })
            .collect();

        Paragraph::new(content_lines).render(inner, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::DarkGray);

        let (cline, ccol) = self.cursor;
        let line1 = Line::from(vec![
            Span::raw(" "),
            Span::styled("[200]", dim),
            Span::raw("  "),
            Span::styled(format!("{} lines", self.lines.len()), dim),
            Span::raw("  "),
            Span::styled(format!("{}:{}", cline + 1, ccol + 1), dim),
        ]);

        let line2 = Line::from(vec![
            Span::raw(" "),
            Span::styled("↑↓←→", bold),
            Span::raw(":move  "),
            Span::styled("PgUp/Dn", bold),
            Span::raw(":half-page  "),
            Span::styled("Space", bold),
            Span::raw(":select  "),
            Span::styled("Enter", bold),
            Span::raw(":copy  "),
            Span::styled("Esc", bold),
            Span::raw(":close"),
        ]);

        Paragraph::new(vec![line1, line2])
            .block(Block::default().borders(Borders::ALL))
            .render(area, buf);
    }
}
