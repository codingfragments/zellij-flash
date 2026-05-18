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
    }

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        if matches!(key.bare_key, BareKey::Esc) {
            close_self();
        }
        false
    }

    fn render_all(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 5 {
            Paragraph::new("too small")
                .style(Style::default().fg(Color::DarkGray))
                .render(area, buf);
            return;
        }

        // Footer is 2 content lines + 2 border lines = 4 rows total.
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

        // Phase 1a: cursor is pinned to the last line.
        let cursor_line = total.saturating_sub(1);

        // Scroll so the cursor sits at the bottom of the viewport.
        let scroll_y = total.saturating_sub(viewport_h);
        let visible = &self.lines[scroll_y..];

        // Gutter: right-aligned relative number + marker ("► " or "  ").
        // Width is driven by the largest distance that can appear on screen.
        let max_dist = viewport_h.saturating_sub(1);
        let num_w = max_dist.to_string().len().max(1);
        let gutter_w = num_w + 2; // digits + marker
        let avail_w = (inner.width as usize).saturating_sub(gutter_w);

        let gutter_dim = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM);
        let gutter_cursor = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        let content_lines: Vec<Line<'static>> = visible
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let abs = scroll_y + i;
                let is_cursor = abs == cursor_line;
                let dist = (abs as isize - cursor_line as isize).unsigned_abs();

                let gutter_str = format!(
                    "{:>w$}{}",
                    dist,
                    if is_cursor { "► " } else { "  " },
                    w = num_w
                );
                let gutter = Span::styled(
                    gutter_str,
                    if is_cursor { gutter_cursor } else { gutter_dim },
                );

                // Truncate to available width (horizontal scroll comes in phase 4).
                let text_display: String = text.chars().take(avail_w).collect();
                let content = Span::raw(text_display);

                Line::from(vec![gutter, content])
            })
            .collect();

        Paragraph::new(content_lines).render(inner, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::DarkGray);

        let line1 = Line::from(vec![
            Span::raw(" "),
            Span::styled("[200]", dim),
            Span::raw("  "),
            Span::styled(
                format!("{} lines", self.lines.len()),
                dim,
            ),
        ]);

        let line2 = Line::from(vec![
            Span::raw(" "),
            Span::styled("Space", bold),
            Span::raw(":select  "),
            Span::styled("s", bold),
            Span::raw(":jump  "),
            Span::styled("l", bold),
            Span::raw(":line  "),
            Span::styled("Enter", bold),
            Span::raw(":copy  "),
            Span::styled("g", bold),
            Span::raw(":depth  "),
            Span::styled("Esc", bold),
            Span::raw(":close"),
        ]);

        Paragraph::new(vec![line1, line2])
            .block(Block::default().borders(Borders::ALL))
            .render(area, buf);
    }
}
