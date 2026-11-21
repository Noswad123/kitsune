//! Client-only inspection of the existing bounded integration projection.
use crate::{
    api::schema::{IntegrationViewItem, IntegrationViewStatus},
    app::{state::Mode, AppState},
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Inspector {
    source: Option<String>,
    epoch: Option<String>,
    item: Option<String>,
    query: String,
    search: bool,
    focus: u8,
    source_scroll: usize,
    item_scroll: usize,
    detail_scroll: usize,
    expanded: bool,
    pub(crate) computed: Option<Capture>,
    pub(crate) displayed: Option<Capture>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Source {
    id: String,
    epoch: Option<String>,
    revision: u64,
    items: Vec<IntegrationViewItem>,
}

// Spinner phase is deliberately absent. Every semantic/input/layout change is fenced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Capture {
    area: Rect,
    viewport: Rect,
    layout: crate::app::state::ViewLayout,
    sidebar: Rect,
    terminal: Rect,
    source: Option<String>,
    epoch: Option<String>,
    item: Option<String>,
    query: String,
    search: bool,
    focus: u8,
    scrolls: [usize; 3],
    sources: Vec<Source>,
    source_hits: Vec<(Rect, String)>,
    item_hits: Vec<(Rect, String)>,
    details: Rect,
    detail_lines: Vec<String>,
    expanded: bool,
    structured: bool,
    expander_hit: Option<Rect>,
}

fn text(value: &str, width: usize) -> String {
    crate::annotations::render_text(value, width)
}

fn wrapped(value: &str, width: u16) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let escaped = text(value, usize::MAX);
    let mut lines = vec![String::new()];
    for ch in escaped.chars() {
        if let Some(line) = lines.last_mut() {
            line.push(ch);
            // Measure the candidate, including emoji variation/join effects.
            if line.width() > width as usize {
                line.pop();
                let next = ch.to_string();
                if next.width() <= width as usize {
                    lines.push(next);
                }
            }
        }
    }
    lines
}

impl AppState {
    fn inspector_sources(&self, now: Instant) -> Vec<Source> {
        self.integration_views
            .descriptor(now)
            .sources
            .into_iter()
            .map(|s| {
                let projection = self.integration_views.project(&s.id, now);
                Source {
                    id: s.id,
                    epoch: s.epoch,
                    revision: projection.as_ref().map_or(0, |p| p.revision),
                    items: projection.map_or_else(Vec::new, |p| p.items.to_vec()),
                }
            })
            .collect()
    }

    pub(crate) fn open_integration_inspector(&mut self) {
        let sources = self.integration_views.configured_sources();
        let source = sources
            .iter()
            .find(|s| s.as_str() == "coven")
            .cloned()
            .or_else(|| match &self.sidebar_section_focus {
                super::SectionId::Integration(id) if sources.contains(id) => Some(id.clone()),
                _ => sources.first().cloned(),
            });
        self.integration_inspector = Inspector {
            source,
            focus: 1,
            ..Default::default()
        };
        self.inspector_select_source(Instant::now());
        self.mode = Mode::IntegrationInspector;
    }

    fn inspector_select_source(&mut self, now: Instant) {
        let sources = self.inspector_sources(now);
        let s = sources
            .iter()
            .find(|s| Some(&s.id) == self.integration_inspector.source.as_ref());
        let i = &mut self.integration_inspector;
        let previous = (i.source.clone(), i.epoch.clone(), i.item.clone());
        i.epoch = s.and_then(|s| s.epoch.clone());
        i.item = s
            .and_then(|s| s.items.iter().find(|item| matches_query(item, &i.query)))
            .map(|item| item.id.clone());
        if previous != (i.source.clone(), i.epoch.clone(), i.item.clone()) {
            i.item_scroll = 0;
            i.detail_scroll = 0;
            i.expanded = false;
        }
    }

    fn inspector_capture(&self, area: Rect, now: Instant) -> Capture {
        let i = &self.integration_inspector;
        let viewport = area;
        let width = area.width.min(100);
        let height = area.height.min(32);
        let area = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let inner = Block::default().borders(Borders::ALL).inner(area);
        // Vertical bands also work on narrow/mobile clients, without off-panel hits.
        let available = inner.height.saturating_sub(2);
        let source_height = (available / 4).min(8);
        let item_height = available / 3;
        let source_area = Rect::new(
            inner.x,
            inner.y.saturating_add(2).min(inner.bottom()),
            inner.width,
            source_height,
        );
        let item_area = Rect::new(inner.x, source_area.bottom(), inner.width, item_height);
        let details = Rect::new(
            inner.x,
            item_area.bottom(),
            inner.width,
            inner.bottom().saturating_sub(item_area.bottom()),
        );
        let sources = self.inspector_sources(now);
        let source_hits = if inner.width == 0 {
            Vec::new()
        } else {
            sources
                .iter()
                .skip(i.source_scroll)
                .take(source_height as usize)
                .enumerate()
                .map(|(n, s)| {
                    (
                        Rect::new(inner.x, source_area.y + n as u16, inner.width, 1),
                        s.id.clone(),
                    )
                })
                .collect()
        };
        let source = sources
            .iter()
            .find(|s| Some(&s.id) == i.source.as_ref() && s.epoch == i.epoch);
        let item_hits = if inner.width == 0 {
            Vec::new()
        } else {
            source
                .into_iter()
                .flat_map(|s| &s.items)
                .filter(|item| matches_query(item, &i.query))
                .skip(i.item_scroll)
                .take(item_height as usize)
                .enumerate()
                .map(|(n, item)| {
                    (
                        Rect::new(inner.x, item_area.y + n as u16, inner.width, 1),
                        item.id.clone(),
                    )
                })
                .collect()
        };
        let selected = source.and_then(|s| {
            s.items
                .iter()
                .find(|item| Some(&item.id) == i.item.as_ref() && matches_query(item, &i.query))
        });
        let mut detail_lines = Vec::new();
        let mut expander_hit = None;
        let structured = selected.is_some_and(|item| item.information.is_some());
        if let Some(item) = selected {
            if let Some(info) = &item.information {
                for (heading, values, empty) in [
                    (
                        "Attention",
                        &info.attention,
                        "No attention information supplied.",
                    ),
                    (
                        "Current activity",
                        &info.activity,
                        "No activity information supplied.",
                    ),
                    (
                        "Working context",
                        &info.context,
                        "No working context supplied.",
                    ),
                ] {
                    detail_lines.extend(wrapped(heading, details.width));
                    if values.is_empty() {
                        detail_lines.extend(wrapped(empty, details.width));
                    } else {
                        for value in values {
                            detail_lines.extend(wrapped(value, details.width));
                        }
                    }
                }
                if !i.expanded && !i.query.is_empty() && !matches_main(item, &i.query) {
                    detail_lines.extend(wrapped(
                        "Match in Diagnostics — expand to view.",
                        details.width,
                    ));
                }
                let start = detail_lines.len();
                detail_lines.extend(wrapped(
                    if i.expanded {
                        "▾ Diagnostics [d]"
                    } else {
                        "▸ Diagnostics [d]"
                    },
                    details.width,
                ));
                let visible_start = start.max(i.detail_scroll);
                let visible_end = detail_lines
                    .len()
                    .min(i.detail_scroll.saturating_add(details.height as usize));
                if details.width > 0 && visible_start < visible_end {
                    expander_hit = Some(Rect::new(
                        details.x,
                        details.y + (visible_start - i.detail_scroll) as u16,
                        details.width,
                        (visible_end - visible_start) as u16,
                    ));
                }
                if i.expanded {
                    detail_lines.extend(wrapped(&item.detail, details.width));
                }
            } else {
                detail_lines.extend(wrapped("Details", details.width));
                detail_lines.extend(wrapped(&item.detail, details.width));
            }
        } else {
            detail_lines.extend(wrapped(
                if sources.is_empty() {
                    "No integration sources configured."
                } else {
                    "Select an item. Unavailable sources have no current data."
                },
                details.width,
            ));
        }
        Capture {
            area,
            viewport,
            layout: self.view.layout,
            sidebar: self.view.sidebar_rect,
            terminal: self.view.terminal_area,
            source: i.source.clone(),
            epoch: i.epoch.clone(),
            item: i.item.clone(),
            query: i.query.clone(),
            search: i.search,
            focus: i.focus,
            scrolls: [i.source_scroll, i.item_scroll, i.detail_scroll],
            sources,
            source_hits,
            item_hits,
            details,
            detail_lines,
            expanded: i.expanded,
            structured,
            expander_hit,
        }
    }

    pub(crate) fn compute_integration_inspector(&mut self, area: Rect, now: Instant) {
        if self.mode != Mode::IntegrationInspector {
            self.integration_inspector.computed = None;
            self.integration_inspector.displayed = None;
            return;
        }
        let sources = self.inspector_sources(now);
        let i = &mut self.integration_inspector;
        let source = sources.iter().find(|s| Some(&s.id) == i.source.as_ref());
        if source.is_none() || source.is_some_and(|s| s.epoch != i.epoch) {
            i.epoch = source.and_then(|s| s.epoch.clone());
            i.item = None;
            i.detail_scroll = 0;
            i.item_scroll = 0;
            i.expanded = false;
        }
        if i.item.as_ref().is_some_and(|id| {
            !source.is_some_and(|s| {
                s.items
                    .iter()
                    .any(|item| &item.id == id && matches_query(item, &i.query))
            })
        }) {
            i.item = None;
            i.detail_scroll = 0;
            i.expanded = false;
        }
        let capture = self.inspector_capture(area, now);
        let i = &mut self.integration_inspector;
        i.source_scroll = i.source_scroll.min(
            capture
                .sources
                .len()
                .saturating_sub(capture.source_hits.len().max(1)),
        );
        if let Some(index) = capture
            .sources
            .iter()
            .position(|s| Some(&s.id) == i.source.as_ref())
        {
            i.source_scroll = follow(index, i.source_scroll, capture.source_hits.len().max(1));
        }
        i.item_scroll = i.item_scroll.min(
            source
                .map_or(0, |s| {
                    s.items
                        .iter()
                        .filter(|item| matches_query(item, &i.query))
                        .count()
                })
                .saturating_sub(capture.item_hits.len().max(1)),
        );
        i.detail_scroll = i.detail_scroll.min(
            capture
                .detail_lines
                .len()
                .saturating_sub(capture.details.height as usize),
        );
        let capture = self.inspector_capture(area, now);
        if self.integration_inspector.displayed.as_ref() != Some(&capture) {
            self.integration_inspector.displayed = None;
        }
        self.integration_inspector.computed = Some(capture);
    }

    fn inspector_current_capture(&self, now: Instant) -> Option<&Capture> {
        let displayed = self.integration_inspector.displayed.as_ref()?;
        (self.mode == Mode::IntegrationInspector
            && *displayed == self.inspector_capture(displayed.viewport, now))
        .then_some(displayed)
    }

    #[cfg(test)]
    pub(crate) fn integration_inspector_selected_rect(&self) -> Option<Rect> {
        let c = self.integration_inspector.displayed.as_ref()?;
        c.item_hits
            .iter()
            .find(|(_, id)| Some(id) == c.item.as_ref())
            .map(|(rect, _)| *rect)
    }

    fn inspector_activate(&mut self, now: Instant) -> bool {
        let Some(c) = self.inspector_current_capture(now) else {
            return false;
        };
        if c.search || c.focus != 1 {
            return false;
        }
        let Some(source) = c.sources.iter().find(|s| Some(&s.id) == c.source.as_ref()) else {
            return false;
        };
        if !c.source_hits.iter().any(|(_, id)| id == &source.id) {
            return false;
        }
        let (Some(epoch), Some(id)) = (source.epoch.clone(), c.item.clone()) else {
            return false;
        };
        if !c.item_hits.iter().any(|(_, hit)| hit == &id) {
            return false;
        }
        let source_id = source.id.clone();
        let revision = source.revision;
        self.integration_views
            .activate(&source_id, &epoch, revision, &id, now)
    }

    fn inspector_move(&mut self, delta: isize, now: Instant) {
        let sources = self.inspector_sources(now);
        let i = &mut self.integration_inspector;
        match i.focus {
            0 => {
                let previous = i.source.clone();
                let current = sources
                    .iter()
                    .position(|s| Some(&s.id) == i.source.as_ref());
                let next = current
                    .map_or(0, |n| n.saturating_add_signed(delta))
                    .min(sources.len().saturating_sub(1));
                i.source = sources.get(next).map(|s| s.id.clone());
                if i.source != previous {
                    i.expanded = false;
                    i.detail_scroll = 0;
                }
                let capacity = i
                    .computed
                    .as_ref()
                    .map_or(1, |c| c.source_hits.len().max(1));
                i.source_scroll = follow(next, i.source_scroll, capacity);
                self.inspector_select_source(now);
            }
            1 => {
                let items: Vec<_> = sources
                    .iter()
                    .find(|s| Some(&s.id) == i.source.as_ref() && s.epoch == i.epoch)
                    .into_iter()
                    .flat_map(|s| &s.items)
                    .filter(|item| matches_query(item, &i.query))
                    .collect();
                let current = items
                    .iter()
                    .position(|item| Some(&item.id) == i.item.as_ref());
                let next = current
                    .map_or(0, |n| n.saturating_add_signed(delta))
                    .min(items.len().saturating_sub(1));
                let next_item = items.get(next).map(|item| item.id.clone());
                if i.item != next_item {
                    i.expanded = false;
                    i.detail_scroll = 0;
                }
                i.item = next_item;
                let capacity = i.computed.as_ref().map_or(1, |c| c.item_hits.len().max(1));
                i.item_scroll = follow(next, i.item_scroll, capacity);
            }
            _ => i.detail_scroll = i.detail_scroll.saturating_add_signed(delta),
        }
    }

    pub(crate) fn integration_inspector_key(&mut self, key: KeyEvent) {
        let now = Instant::now();
        let i = &mut self.integration_inspector;
        if i.search {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => i.search = false,
                KeyCode::Backspace => {
                    i.query.pop();
                    i.item_scroll = 0;
                    i.detail_scroll = 0;
                }
                KeyCode::Char(ch)
                    if !ch.is_control()
                        && !key.modifiers.intersects(
                            crossterm::event::KeyModifiers::CONTROL
                                | crossterm::event::KeyModifiers::ALT,
                        ) =>
                {
                    if i.query.len() < 512 {
                        i.query.push(ch);
                    }
                    i.item_scroll = 0;
                    i.detail_scroll = 0;
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('d')
                if !key.modifiers.intersects(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::ALT,
                ) =>
            {
                if self
                    .inspector_current_capture(now)
                    .is_some_and(|c| c.focus == 2 && c.structured)
                {
                    self.integration_inspector.expanded = !self.integration_inspector.expanded;
                }
            }
            KeyCode::Esc => {
                self.mode = if self.active.is_some() {
                    Mode::Terminal
                } else {
                    Mode::Navigate
                };
                i.displayed = None;
            }
            KeyCode::Tab => {
                i.focus = (i.focus + 1) % 3;
            }
            KeyCode::BackTab => {
                i.focus = (i.focus + 2) % 3;
            }
            KeyCode::Char('/') => {
                i.search = true;
            }
            KeyCode::Down | KeyCode::Char('j') => self.inspector_move(1, now),
            KeyCode::Up | KeyCode::Char('k') => self.inspector_move(-1, now),
            KeyCode::PageDown => self.inspector_move(8, now),
            KeyCode::PageUp => self.inspector_move(-8, now),
            KeyCode::Enter => {
                self.inspector_activate(now);
            }
            _ => {}
        }
    }

    pub(crate) fn integration_inspector_mouse(&mut self, mouse: MouseEvent) {
        let now = Instant::now();
        let Some(c) = self.inspector_current_capture(now).cloned() else {
            return;
        };
        let point = Position::new(mouse.column, mouse.row);
        if !c.area.contains(point) {
            return;
        }
        let focus = if c.details.contains(point) {
            Some(2)
        } else if c.source_hits.iter().any(|(r, _)| r.contains(point)) {
            Some(0)
        } else if c.item_hits.iter().any(|(r, _)| r.contains(point)) {
            Some(1)
        } else {
            None
        };
        let Some(focus) = focus else {
            return;
        };
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.integration_inspector.focus = focus;
                if c.expander_hit.is_some_and(|r| r.contains(point)) {
                    self.integration_inspector.expanded = !c.expanded;
                } else if let Some((_, source)) =
                    c.source_hits.iter().find(|(r, _)| r.contains(point))
                {
                    if self.integration_inspector.source.as_ref() != Some(source) {
                        self.integration_inspector.expanded = false;
                        self.integration_inspector.detail_scroll = 0;
                    }
                    self.integration_inspector.source = Some(source.clone());
                    self.inspector_select_source(now);
                } else if let Some((_, id)) = c.item_hits.iter().find(|(r, _)| r.contains(point)) {
                    if self.integration_inspector.item.as_ref() != Some(id) {
                        self.integration_inspector.expanded = false;
                        self.integration_inspector.detail_scroll = 0;
                    }
                    self.integration_inspector.item = Some(id.clone());
                }
            }
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                self.integration_inspector.focus = focus;
                self.inspector_move(
                    if mouse.kind == MouseEventKind::ScrollDown {
                        3
                    } else {
                        -3
                    },
                    now,
                );
            }
            _ => {}
        }
    }

    pub(crate) fn integration_has_animation(&self, now: Instant) -> bool {
        if !self.integration_animations {
            return false;
        }
        let inspector = (self.mode == Mode::IntegrationInspector)
            .then_some(self.integration_inspector.computed.as_ref())
            .flatten();
        let inspector_running = inspector.is_some_and(|c| {
            c.sources
                .iter()
                .find(|s| Some(&s.id) == c.source.as_ref())
                .and_then(|s| self.integration_views.project(&s.id, now))
                .is_some_and(|p| {
                    p.items.iter().any(|item| {
                        c.item_hits.iter().any(|(_, id)| id == &item.id) && running(item)
                    })
                })
        });
        inspector_running
            || (self.view.layout == crate::app::state::ViewLayout::Desktop
                && self.sidebar_sections.sections.iter().any(|s| {
                    let super::SectionId::Integration(id) = &s.id else {
                        return false;
                    };
                    s.body.width > 0
                        && s.body.height > 0
                        && !self.collapsed_sidebar_sections.contains(&s.id)
                        && self.integration_views.project(id, now).is_some_and(|p| {
                            p.items
                                .iter()
                                .skip(self.integration_view_scrolls.get(id).copied().unwrap_or(0))
                                .take(s.body.height as usize)
                                .enumerate()
                                .any(|(index, item)| {
                                    let row = Rect::new(
                                        s.body.x,
                                        s.body.y + index as u16,
                                        s.body.width,
                                        1,
                                    );
                                    running(item)
                                        && inspector.is_none_or(|c| row.intersection(c.area) != row)
                                })
                        })
                }))
    }
}

fn running(item: &IntegrationViewItem) -> bool {
    item.presentation
        .as_ref()
        .is_some_and(|p| p.status == Some(IntegrationViewStatus::Running))
}

fn matches_query(item: &IntegrationViewItem, query: &str) -> bool {
    matches_main(item, query) || item.detail.to_lowercase().contains(&query.to_lowercase())
}

fn matches_main(item: &IntegrationViewItem, query: &str) -> bool {
    let query = query.to_lowercase();
    item.label.to_lowercase().contains(&query)
        || item.information.as_ref().is_some_and(|info| {
            info.attention
                .iter()
                .chain(&info.activity)
                .chain(&info.context)
                .any(|value| value.to_lowercase().contains(&query))
        })
        || item
            .presentation
            .as_ref()
            .is_some_and(|p| p.label.to_lowercase().contains(&query))
}

fn follow(index: usize, scroll: usize, capacity: usize) -> usize {
    if index < scroll {
        index
    } else if index >= scroll + capacity {
        index + 1 - capacity
    } else {
        scroll
    }
}

pub(super) fn render(app: &AppState, frame: &mut Frame) {
    let Some(c) = &app.integration_inspector.computed else {
        return;
    };
    frame.render_widget(Clear, c.area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Integration details ")
        .title_bottom(if c.structured { " d: diagnostics " } else { "" })
        .style(
            Style::default()
                .fg(app.palette.text)
                .bg(app.palette.panel_bg),
        );
    let inner = block.inner(c.area);
    frame.render_widget(block, c.area);
    for (n, value) in [
        format!(
            "Tab: {}  / search  Enter: activate  Esc: close",
            ["Sources", "Items", "Details"][c.focus as usize]
        ),
        format!("{}{}", if c.search { "/ " } else { "Filter: " }, c.query),
    ]
    .iter()
    .enumerate()
    {
        if n < inner.height as usize {
            frame.render_widget(
                Paragraph::new(text(value, inner.width as usize)),
                Rect::new(inner.x, inner.y + n as u16, inner.width, 1),
            );
        }
    }
    for (rect, id) in &c.source_hits {
        let source = c.sources.iter().find(|s| &s.id == id);
        let label = format!(
            "{} {}{}",
            if Some(id) == c.source.as_ref() {
                ">"
            } else {
                " "
            },
            id,
            if source.is_some_and(|s| s.epoch.is_none()) {
                " (unavailable)"
            } else {
                ""
            }
        );
        frame.render_widget(
            Paragraph::new(text(&label, rect.width as usize))
                .style(Style::default().fg(app.palette.accent)),
            *rect,
        );
    }
    if let Some(source) = c.sources.iter().find(|s| Some(&s.id) == c.source.as_ref()) {
        for (rect, id) in &c.item_hits {
            let Some(item) = source.items.iter().find(|item| &item.id == id) else {
                continue;
            };
            let (glyph, color) = super::integration_visual::visual(
                item.presentation.as_ref().and_then(|p| p.status),
                app.spinner_tick,
                app.integration_animations,
                &app.palette,
            );
            let label = super::integration_visual::item_label(item);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw(if Some(id) == c.item.as_ref() {
                        "> "
                    } else {
                        "  "
                    }),
                    Span::styled(glyph, Style::default().fg(color)),
                    Span::raw(format!(
                        " {}",
                        text(&label, rect.width.saturating_sub(4) as usize)
                    )),
                ])),
                *rect,
            );
        }
    }
    let lines: Vec<_> = c
        .detail_lines
        .iter()
        .skip(c.scrolls[2])
        .take(c.details.height as usize)
        .map(|s| Line::raw(s.as_str()))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().fg(app.palette.subtext0)),
        c.details,
    );
}

#[cfg(test)]
mod profile_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::schema::{
        IntegrationViewPresentation, ViewOpenParams, ViewOwnerParams, ViewReplaceParams,
    };
    use crossterm::event::KeyModifiers;
    use ratatui::{backend::TestBackend, Terminal};
    use std::time::Duration;

    #[test]
    fn inspector_empty_captions_keep_exact_names_distinct_hits_and_readable_details() {
        let (mut app, owners, now) = setup();
        app.integration_views
            .replace(
                &ViewReplaceParams {
                    source: owners[1].source.clone(),
                    lease: owners[1].lease.clone(),
                    epoch: owners[1].epoch.clone(),
                    revision: 2,
                    items: ["Djinn", "Djinn", "My pane"]
                        .into_iter()
                        .enumerate()
                        .map(|(n, label)| IntegrationViewItem {
                            information: None,
                            id: format!("session-hash-{n}"),
                            label: label.into(),
                            detail: "full provenance: session-hash".into(),
                            presentation: Some(IntegrationViewPresentation {
                                label: String::new(),
                                status: (n < 2).then_some(IntegrationViewStatus::Running),
                            }),
                        })
                        .collect(),
                },
                now,
            )
            .unwrap();
        app.inspector_select_source(now);
        draw(&mut app, now);
        let capture = app.integration_inspector.displayed.clone().unwrap();
        let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
        terminal.draw(|f| render(&app, f)).unwrap();
        let buffer = terminal.backend().buffer();
        for ((rect, _), expected) in
            capture
                .item_hits
                .iter()
                .zip(["> ⠋ Djinn", "  ⠋ Djinn", "  · My pane"])
        {
            let rendered: String = (rect.x..rect.right())
                .map(|x| buffer[(x, rect.y)].symbol())
                .collect();
            assert_eq!(rendered.trim_end(), expected);
        }
        assert_eq!(capture.detail_lines[0], "Details");
        assert!(capture
            .detail_lines
            .join("")
            .contains("full provenance: session-hash"));
        for (rect, id) in &capture.item_hits {
            app.integration_inspector_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: rect.x,
                row: rect.y,
                modifiers: KeyModifiers::NONE,
            });
            assert_eq!(app.integration_inspector.item.as_ref(), Some(id));
            draw(&mut app, now);
            assert!(app.inspector_activate(now));
            let intents = app.integration_views.poll(&owners[1], now).unwrap().intents;
            assert_eq!(intents.len(), 1);
            assert_eq!(&intents[0].item_id, id);
            assert_eq!(
                app.integration_inspector
                    .displayed
                    .as_ref()
                    .unwrap()
                    .detail_lines[0],
                "Details"
            );
        }
    }

    #[test]
    fn inspector_wrapping_bounds_emoji_variations_and_escapes_controls() {
        for width in 1..=12 {
            let lines = wrapped("a©️界👩‍💻\u{202e}suffix", width);
            assert!(lines.iter().all(|line| line.width() <= width as usize));
            assert!(!lines.join("").contains('\u{202e}'));
        }
    }

    pub(super) fn setup() -> (AppState, Vec<ViewOwnerParams>, Instant) {
        setup_version(2)
    }

    pub(super) fn setup_v3() -> (AppState, Vec<ViewOwnerParams>, Instant) {
        setup_version(3)
    }

    fn setup_version(version: u64) -> (AppState, Vec<ViewOwnerParams>, Instant) {
        let now = Instant::now();
        let mut app = AppState::test_new();
        app.integration_views
            .reconfigure(vec!["build".into(), "coven".into()])
            .unwrap();
        let mut owners = Vec::new();
        for source in ["build", "coven"] {
            let opened = app
                .integration_views
                .open(
                    &ViewOpenParams {
                        source: source.into(),
                        version,
                    },
                    now,
                )
                .unwrap();
            let owner = ViewOwnerParams {
                source: source.into(),
                epoch: opened.epoch,
                lease: opened.lease,
            };
            replace(&mut app, &owner, 1, now);
            owners.push(owner);
        }
        app.open_integration_inspector();
        (app, owners, now)
    }

    fn replace(app: &mut AppState, owner: &ViewOwnerParams, revision: u64, now: Instant) {
        app.integration_views
            .replace(
                &ViewReplaceParams {
                    source: owner.source.clone(),
                    lease: owner.lease.clone(),
                    epoch: owner.epoch.clone(),
                    revision,
                    items: (0..24)
                        .map(|n| IntegrationViewItem {
                            information: None,
                            id: format!("same-{n}"),
                            label: format!("Item {n}"),
                            detail: "long detail with wide 界 text ".repeat(12),
                            presentation: Some(IntegrationViewPresentation {
                                label: "Djinn".into(),
                                status: Some(IntegrationViewStatus::Running),
                            }),
                        })
                        .collect(),
                },
                now,
            )
            .unwrap();
    }

    pub(super) fn draw(app: &mut AppState, now: Instant) {
        app.compute_integration_inspector(Rect::new(0, 0, 40, 24), now);
        let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
        terminal.draw(|f| render(app, f)).unwrap();
        app.commit_integration_views_render();
    }

    pub(super) fn key(app: &mut AppState, code: KeyCode) {
        app.integration_inspector_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn inspector_animation_unions_unoccluded_sidebar_and_modal_rows() {
        let (mut app, owners, now) = setup();
        let mut items = app
            .integration_views
            .project("build", now)
            .unwrap()
            .items
            .to_vec();
        for item in &mut items {
            item.presentation.as_mut().unwrap().status = None;
        }
        app.integration_views
            .replace(
                &ViewReplaceParams {
                    source: "build".into(),
                    lease: owners[0].lease.clone(),
                    epoch: owners[0].epoch.clone(),
                    revision: 2,
                    items,
                },
                now,
            )
            .unwrap();
        app.integration_inspector.source = Some("build".into());
        app.inspector_select_source(now);
        app.sidebar_width = 36;
        super::super::compute_view(&mut app, Rect::new(0, 0, 200, 30));
        assert!(app.integration_has_animation(now)); // Coven remains visible left of the overlay.
        app.collapsed_sidebar_sections
            .insert(super::super::SectionId::Integration("coven".into()));
        super::super::compute_view(&mut app, Rect::new(0, 0, 200, 30));
        assert!(!app.integration_has_animation(now));
        app.collapsed_sidebar_sections.clear();
        app.sidebar_collapsed = true;
        app.sidebar_collapsed_mode = crate::config::SidebarCollapsedModeConfig::Hidden;
        super::super::compute_view(&mut app, Rect::new(0, 0, 200, 30));
        assert!(!app.integration_has_animation(now));
        app.sidebar_collapsed = false;
        super::super::compute_view(&mut app, Rect::new(0, 0, 80, 30));
        assert!(!app.integration_has_animation(now)); // The entire sidebar is occluded.
        app.integration_inspector.source = Some("coven".into());
        app.inspector_select_source(now);
        super::super::compute_view(&mut app, Rect::new(0, 0, 80, 30));
        assert!(app.integration_has_animation(now)); // Modal rows themselves are running.
        app.integration_animations = false;
        assert!(!app.integration_has_animation(now));
        app.integration_animations = true;
        assert!(!app.integration_has_animation(now + Duration::from_secs(5)));
    }

    #[test]
    fn inspector_reads_collapsed_sources_without_native_actions_and_activates_only_explicitly() {
        let (mut app, owners, now) = setup();
        app.collapsed_sidebar_sections
            .insert(super::super::SectionId::Integration("coven".into()));
        let collapsed = app.collapsed_sidebar_sections.clone();
        let native = (app.active, app.selected, app.workspaces.len());
        assert_eq!(app.integration_inspector.source.as_deref(), Some("coven"));
        draw(&mut app, now);
        assert!(!app
            .integration_inspector
            .displayed
            .as_ref()
            .unwrap()
            .detail_lines
            .is_empty());
        let hit = app
            .integration_inspector
            .displayed
            .as_ref()
            .unwrap()
            .item_hits[1]
            .0;
        app.integration_inspector_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.x,
            row: hit.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.integration_inspector.item.as_deref(), Some("same-1"));
        assert!(app
            .integration_views
            .poll(&owners[1], now)
            .unwrap()
            .intents
            .is_empty());
        key(&mut app, KeyCode::Enter); // selection not yet displayed
        assert!(app
            .integration_views
            .poll(&owners[1], now)
            .unwrap()
            .intents
            .is_empty());
        draw(&mut app, now);
        key(&mut app, KeyCode::Enter);
        let intents = app.integration_views.poll(&owners[1], now).unwrap().intents;
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].item_id, "same-1");
        assert!(app
            .integration_views
            .poll(&owners[0], now)
            .unwrap()
            .intents
            .is_empty());
        assert_eq!(native, (app.active, app.selected, app.workspaces.len()));
        assert_eq!(collapsed, app.collapsed_sidebar_sections);
    }

    #[test]
    fn inspector_keyboard_and_mouse_fence_revisions_failed_draw_and_expiry() {
        let (mut app, owners, now) = setup();
        draw(&mut app, now);
        let hit = app
            .integration_inspector
            .displayed
            .as_ref()
            .unwrap()
            .item_hits[1]
            .0;
        replace(&mut app, &owners[1], 2, now);
        assert!(!app.inspector_activate(now));
        app.integration_inspector_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.x,
            row: hit.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.integration_inspector.item.as_deref(), Some("same-0"));
        app.compute_integration_inspector(Rect::new(0, 0, 80, 24), now);
        assert!(!app.inspector_activate(now)); // computed is not displayed
        draw(&mut app, now);
        assert!(app.inspector_activate(now));
        app.integration_inspector.displayed = None; // unsuccessful frame enqueue
        assert!(!app.inspector_activate(now));
        draw(&mut app, now);
        app.spinner_tick += 8;
        assert!(app.inspector_activate(now)); // paint-only phase is not a fence
        assert!(!app.inspector_activate(now + Duration::from_secs(5)));
        draw(&mut app, now + Duration::from_secs(5));
        assert!(app.integration_inspector.item.is_none());
    }

    #[test]
    fn inspector_identity_survives_reorder_not_removal_or_epoch_replacement() {
        let (mut app, owners, now) = setup();
        key(&mut app, KeyCode::Down);
        draw(&mut app, now);
        app.integration_views
            .reconfigure(vec!["coven".into(), "build".into()])
            .unwrap();
        draw(&mut app, now);
        assert_eq!(app.integration_inspector.item.as_deref(), Some("same-1"));
        app.integration_views.close(&owners[1], now).unwrap();
        let reopened = app
            .integration_views
            .open(
                &ViewOpenParams {
                    source: "coven".into(),
                    version: 1,
                },
                now,
            )
            .unwrap();
        draw(&mut app, now);
        assert_eq!(app.integration_inspector.epoch, Some(reopened.epoch));
        assert!(app.integration_inspector.item.is_none());
        assert!(!app.inspector_activate(now));
        app.integration_views
            .reconfigure(vec!["build".into()])
            .unwrap();
        draw(&mut app, now);
        assert!(app.integration_inspector.item.is_none());
        key(&mut app, KeyCode::BackTab); // sources
        key(&mut app, KeyCode::Down);
        assert_eq!(app.integration_inspector.source.as_deref(), Some("build"));
        assert_eq!(app.integration_inspector.item.as_deref(), Some("same-0"));
    }

    #[test]
    fn inspector_search_focus_and_wrapped_detail_scroll_are_local_and_independent() {
        let (mut app, owners, now) = setup();
        draw(&mut app, now);
        key(&mut app, KeyCode::Tab);
        key(&mut app, KeyCode::PageDown);
        draw(&mut app, now);
        assert!(app.integration_inspector.detail_scroll > 0);
        assert_eq!(app.integration_inspector.item_scroll, 0);
        let c = app.integration_inspector.displayed.as_ref().unwrap();
        assert!(c
            .detail_lines
            .iter()
            .all(|line| super::super::text::display_width(line) <= c.details.width as usize));
        key(&mut app, KeyCode::Char('/'));
        for ch in "Item 20".chars() {
            key(&mut app, KeyCode::Char(ch));
        }
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::IntegrationInspector);
        draw(&mut app, now);
        assert!(app.integration_inspector.item.is_none()); // never rebind by old index
        key(&mut app, KeyCode::BackTab);
        key(&mut app, KeyCode::Down);
        assert_eq!(app.integration_inspector.item.as_deref(), Some("same-20"));
        key(&mut app, KeyCode::Char('x')); // consumed, never terminal input
        assert!(app
            .integration_views
            .poll(&owners[1], now)
            .unwrap()
            .intents
            .is_empty());
        key(&mut app, KeyCode::Esc);
        assert_ne!(app.mode, Mode::IntegrationInspector);
    }

    #[test]
    fn inspector_background_capture_and_tiny_geometry_preserve_foreground_presentation() {
        let (mut app, _, now) = setup();
        draw(&mut app, now);
        app.integration_inspector.detail_scroll = 3;
        draw(&mut app, now);
        let original = app.integration_inspector.clone();
        for width in 0..=12 {
            for height in 0..=12 {
                let saved = app.save_sidebar_presentation();
                app.compute_integration_inspector(Rect::new(0, 0, width, height), now);
                let c = app.integration_inspector.computed.as_ref().unwrap();
                for (r, _) in c.source_hits.iter().chain(&c.item_hits) {
                    assert!(r.width > 0 && r.height > 0);
                    assert!(
                        r.x >= c.area.x
                            && r.y >= c.area.y
                            && r.right() <= c.area.right()
                            && r.bottom() <= c.area.bottom()
                    );
                }
                app.restore_sidebar_presentation(saved);
                assert_eq!(app.integration_inspector, original);
            }
        }
    }

    #[test]
    fn inspector_empty_sources_and_removed_selected_row_do_not_rebind() {
        let (mut app, owners, now) = setup();
        let mut items = app
            .integration_views
            .project("coven", now)
            .unwrap()
            .items
            .to_vec();
        items.remove(0);
        app.integration_views
            .replace(
                &ViewReplaceParams {
                    source: owners[1].source.clone(),
                    lease: owners[1].lease.clone(),
                    epoch: owners[1].epoch.clone(),
                    revision: 2,
                    items,
                },
                now,
            )
            .unwrap();
        draw(&mut app, now);
        assert!(app.integration_inspector.item.is_none());
        app.integration_views.reconfigure(Vec::new()).unwrap();
        app.open_integration_inspector();
        draw(&mut app, now);
        assert!(app
            .integration_inspector
            .computed
            .as_ref()
            .unwrap()
            .detail_lines
            .join("")
            .contains("No integration sources configured"));
    }
}
