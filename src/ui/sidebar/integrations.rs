//! Source-attributed plain text, never native pane/agent targets.
//! Attribution is plaintext, not authentication.
use std::time::Instant;

use ratatui::{
    layout::{Position, Rect},
    style::Style,
    widgets::Paragraph,
    Frame,
};

use crate::app::{state::ViewLayout, AppState};

const ROW_HEIGHT: u16 = 2;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IntegrationViewsSnapshot {
    source: String,
    epoch: Option<String>,
    revision: u64,
    sidebar: Rect,
    terminal: Rect,
    area: Rect,
    collapsed: bool,
    split: f32,
    scroll: usize,
    max_scroll: usize,
    rows: Vec<IntegrationRow>,
    sources: Vec<(String, String, Rect)>,
    configured: Vec<String>,
    message: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
struct IntegrationRow {
    id: String,
    rect: Rect,
    label: String,
    detail: String,
}

impl AppState {
    pub(crate) fn integration_views_area(&self) -> Rect {
        if self.view.layout != ViewLayout::Desktop {
            return Rect::default();
        }
        if self.sidebar_collapsed {
            let (_, _, area) = super::collapsed_sidebar_sections(self.view.sidebar_rect);
            Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1))
        } else {
            super::expanded_sidebar_sections(self.view.sidebar_rect, self.sidebar_section_split).1
        }
    }

    pub(crate) fn compute_integration_views_snapshot(&mut self, now: Instant) {
        self.reconcile_integration_views();
        let configured = self.integration_views.configured_sources().to_vec();
        let source = self.integration_view_selected.clone().unwrap_or_default();
        let area = self.integration_views_area();
        let projection = self.integration_views.project(&source, now);
        let descriptor = self.integration_views.descriptor(now);
        let live_epoch = projection.as_ref().map(|p| p.epoch.to_owned()).or_else(|| {
            descriptor
                .sources
                .iter()
                .find(|s| s.id == source)
                .and_then(|s| s.epoch.clone())
        });
        // Keep a complete item reachable even when the source selector must scroll.
        let reserved_content = if area.height >= 5 { ROW_HEIGHT } else { 0 };
        let selector_height = configured
            .len()
            .min(usize::from(
                area.height.saturating_sub(2 + reserved_content),
            ))
            .min(8);
        let header_height = 2 + selector_height as u16;
        let capacity = usize::from(area.height.saturating_sub(header_height) / ROW_HEIGHT);
        let count = projection.as_ref().map_or(0, |p| p.items.len().min(128));
        let max_scroll = count.saturating_sub(capacity.max(1));
        let scroll = self
            .integration_view_scrolls
            .get(&source)
            .copied()
            .unwrap_or(0)
            .min(max_scroll);
        if !source.is_empty() {
            self.integration_view_scrolls.insert(source.clone(), scroll);
        }
        let selected_index = configured.iter().position(|id| id == &source).unwrap_or(0);
        let start = selected_index.saturating_sub(selector_height.saturating_sub(1));
        let sources = configured
            .iter()
            .skip(start)
            .take(selector_height)
            .enumerate()
            .map(|(i, id)| {
                let status = match self.integration_views.project(id, now) {
                    None if descriptor
                        .sources
                        .iter()
                        .any(|s| &s.id == id && s.epoch.is_some()) =>
                    {
                        "Waiting for data"
                    }
                    None => "Unavailable",
                    Some(p) if p.revision == 0 => "Waiting for data",
                    Some(p) if p.items.is_empty() => "No items",
                    Some(_) => "Available",
                };
                (
                    id.clone(),
                    status.to_owned(),
                    Rect::new(area.x, area.y + 1 + i as u16, area.width, 1),
                )
            })
            .collect();
        let mut snapshot = IntegrationViewsSnapshot {
            source,
            epoch: live_epoch.clone(),
            revision: projection.as_ref().map_or(0, |p| p.revision),
            sidebar: self.view.sidebar_rect,
            terminal: self.view.terminal_area,
            area,
            collapsed: self.sidebar_collapsed,
            split: self.sidebar_section_split,
            scroll,
            max_scroll,
            rows: Vec::new(),
            sources,
            configured,
            message: if self.integration_view_selected.is_none() {
                "No integrations configured"
            } else if live_epoch.is_none() {
                "Unavailable"
            } else if projection.as_ref().is_none_or(|p| p.revision == 0) {
                "Waiting for data"
            } else if count == 0 {
                "No items"
            } else {
                ""
            },
        };
        if let Some(projection) = projection.filter(|_| area.width > 0) {
            for (index, item) in projection
                .items
                .iter()
                .take(128)
                .skip(snapshot.scroll)
                .take(capacity)
                .enumerate()
            {
                snapshot.rows.push(IntegrationRow {
                    id: item.id.clone(),
                    rect: Rect::new(
                        area.x,
                        area.y + header_height + index as u16 * ROW_HEIGHT,
                        area.width,
                        ROW_HEIGHT,
                    ),
                    label: crate::annotations::render_text(&item.label, area.width as usize),
                    detail: crate::annotations::render_text(&item.detail, area.width as usize),
                });
            }
        }
        if self.integration_views_displayed.as_ref() != Some(&snapshot) {
            self.integration_views_displayed = None;
        }
        self.integration_views_computed = Some(snapshot);
    }

    /// Commits a successful server/local draw, not an acknowledgement of physical
    /// client pixels. Thin clients can still have an older frame in flight.
    pub(crate) fn commit_integration_views_render(&mut self) {
        self.integration_views_displayed = self.integration_views_computed.clone();
    }

    pub(crate) fn activate_integration_view_at(
        &mut self,
        col: u16,
        row: u16,
        now: Instant,
    ) -> bool {
        let Some(snapshot) = self.integration_views_displayed.as_ref() else {
            return false;
        };
        if snapshot.sidebar != self.view.sidebar_rect
            || snapshot.terminal != self.view.terminal_area
            || snapshot.area != self.integration_views_area()
            || snapshot.collapsed != self.sidebar_collapsed
            || snapshot.split != self.sidebar_section_split
            || snapshot.scroll
                != self
                    .integration_view_scrolls
                    .get(&snapshot.source)
                    .copied()
                    .unwrap_or(0)
            || self.integration_view_selected.as_deref() != Some(snapshot.source.as_str())
            || snapshot.configured != self.integration_views.configured_sources()
        {
            return false;
        }
        if let Some((source, _, _)) = snapshot
            .sources
            .iter()
            .find(|(_, _, rect)| rect.contains(Position::new(col, row)))
        {
            self.integration_view_selected = Some(source.clone());
            self.integration_views_displayed = None;
            return true;
        }
        let Some(hit) = snapshot
            .rows
            .iter()
            .find(|hit| hit.rect.contains(Position::new(col, row)))
        else {
            return false;
        };
        let Some(epoch) = snapshot.epoch.as_deref() else {
            return false;
        };
        self.integration_views
            .activate(&snapshot.source, epoch, snapshot.revision, &hit.id, now)
    }

    pub(crate) fn scroll_integration_view(&mut self, delta: i16) {
        let max_scroll = self
            .integration_views_computed
            .as_ref()
            .filter(|s| self.integration_view_selected.as_deref() == Some(s.source.as_str()))
            .map_or(0, |s| s.max_scroll);
        if let Some(source) = self.integration_view_selected.as_ref() {
            let scroll = self
                .integration_view_scrolls
                .entry(source.clone())
                .or_default();
            *scroll = scroll.saturating_add_signed(delta as isize).min(max_scroll);
        }
        self.integration_views_displayed = None;
    }

    pub(crate) fn scroll_integration_views_at(&mut self, col: u16, row: u16, delta: i16) {
        let on_selector = self.integration_views_computed.as_ref().is_some_and(|s| {
            s.sources
                .iter()
                .any(|(_, _, rect)| rect.contains(Position::new(col, row)))
        });
        if on_selector {
            let sources = self.integration_views.configured_sources();
            if let Some(index) = sources
                .iter()
                .position(|id| Some(id) == self.integration_view_selected.as_ref())
            {
                let next = index
                    .saturating_add_signed(delta as isize)
                    .min(sources.len().saturating_sub(1));
                self.integration_view_selected = sources.get(next).cloned();
                self.integration_views_displayed = None;
            }
        } else {
            self.scroll_integration_view(delta);
        }
    }

    pub(crate) fn reconcile_integration_views(&mut self) {
        let sources = self.integration_views.configured_sources();
        self.integration_view_scrolls
            .retain(|id, _| sources.contains(id));
        if !self
            .integration_view_selected
            .as_ref()
            .is_some_and(|id| sources.contains(id))
        {
            self.integration_view_selected = sources.first().cloned();
        }
    }
}

pub(super) fn render(app: &AppState, frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let Some(snapshot) = app
        .integration_views_computed
        .as_ref()
        .filter(|s| s.area == area)
    else {
        return;
    };
    let p = &app.palette;
    let mut lines = vec!["Integrations".to_owned()];
    lines.extend(snapshot.sources.iter().map(|(id, status, _)| {
        format!(
            "{}{}: {}",
            if id == &snapshot.source { "> " } else { "  " },
            id,
            status
        )
    }));
    lines.push(snapshot.message.to_owned());
    for (offset, text) in lines.into_iter().enumerate() {
        if offset as u16 >= area.height {
            break;
        }
        frame.render_widget(
            Paragraph::new(crate::annotations::render_text(&text, area.width as usize))
                .style(Style::default().fg(p.overlay0)),
            Rect::new(area.x, area.y + offset as u16, area.width, 1),
        );
    }
    for hit in &snapshot.rows {
        frame.render_widget(
            Paragraph::new(hit.label.as_str()).style(Style::default().fg(p.text)),
            Rect::new(hit.rect.x, hit.rect.y, hit.rect.width, 1),
        );
        frame.render_widget(
            Paragraph::new(hit.detail.as_str()).style(Style::default().fg(p.subtext0)),
            Rect::new(hit.rect.x, hit.rect.y + 1, hit.rect.width, 1),
        );
    }
}

#[cfg(test)]
mod integration_views_sidebar {
    use super::*;
    use crate::api::schema::{
        IntegrationViewItem, ViewOpenParams, ViewOwnerParams, ViewReplaceParams,
    };
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use std::time::Duration;

    fn state() -> AppState {
        let mut state = AppState::test_new();
        state.workspaces = vec![crate::workspace::Workspace::test_new("native-workspace")];
        state.ensure_test_terminals();
        state.view.sidebar_rect = Rect::new(0, 0, 36, 30);
        state.view.workspace_card_areas =
            super::super::compute_workspace_card_areas(&state, state.view.sidebar_rect);
        state
    }

    fn open(state: &mut AppState, now: Instant) -> ViewOwnerParams {
        state.integration_views =
            crate::integration_views::IntegrationViewsState::new(vec!["test.source".into()]);
        let opened = state
            .integration_views
            .open(
                &ViewOpenParams {
                    source: "test.source".into(),
                    version: 1,
                },
                now,
            )
            .unwrap();
        ViewOwnerParams {
            source: "test.source".into(),
            lease: opened.lease,
            epoch: opened.epoch,
        }
    }

    fn replace(
        state: &mut AppState,
        owner: &ViewOwnerParams,
        revision: u64,
        count: usize,
        now: Instant,
    ) {
        state
            .integration_views
            .replace(
                &ViewReplaceParams {
                    source: owner.source.clone(),
                    lease: owner.lease.clone(),
                    epoch: owner.epoch.clone(),
                    revision,
                    items: (0..count)
                        .map(|i| IntegrationViewItem {
                            id: format!("opaque-{i}"),
                            label: format!("plain row {i}"),
                            detail: format!("detail {i}"),
                        })
                        .collect(),
                },
                now,
            )
            .unwrap();
    }

    fn draw(state: &mut AppState, now: Instant) -> Buffer {
        state.compute_integration_views_snapshot(now);
        let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
        terminal
            .draw(|frame| {
                super::super::render_sidebar(
                    state,
                    &crate::terminal::TerminalRuntimeRegistry::new(),
                    frame,
                    state.view.sidebar_rect,
                )
            })
            .unwrap();
        state.commit_integration_views_render();
        terminal.backend().buffer().clone()
    }

    fn text(buffer: &Buffer, area: Rect) -> String {
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn first_hit(state: &AppState) -> Rect {
        state.integration_views_displayed.as_ref().unwrap().rows[0].rect
    }

    fn open_source(state: &mut AppState, source: &str, now: Instant) -> ViewOwnerParams {
        let opened = state
            .integration_views
            .open(
                &ViewOpenParams {
                    source: source.into(),
                    version: 1,
                },
                now,
            )
            .unwrap();
        ViewOwnerParams {
            source: source.into(),
            lease: opened.lease,
            epoch: opened.epoch,
        }
    }

    fn select_source(state: &mut AppState, source: &str, now: Instant) {
        let rect = state
            .integration_views_displayed
            .as_ref()
            .unwrap()
            .sources
            .iter()
            .find(|(id, _, _)| id == source)
            .unwrap()
            .2;
        assert!(state.activate_integration_view_at(rect.x, rect.y, now));
        assert_eq!(state.integration_view_selected.as_deref(), Some(source));
        assert!(state.integration_views_displayed.is_none());
    }

    #[test]
    fn integration_views_sidebar_two_sources_route_same_ids_with_mixed_epochs_and_independent_status(
    ) {
        let now = Instant::now();
        let mut state = state();
        state
            .integration_views
            .reconfigure(vec!["alpha".into(), "beta".into()])
            .unwrap();
        let alpha = open_source(&mut state, "alpha", now);
        let later = now + Duration::from_secs(1);
        let beta = open_source(&mut state, "beta", later);
        assert_ne!(alpha.epoch, beta.epoch);
        replace(&mut state, &alpha, 7, 1, now);
        replace(&mut state, &beta, 3, 1, later);
        draw(&mut state, later);
        let hit = first_hit(&state);
        assert!(state.activate_integration_view_at(hit.x, hit.y, later));
        let intents = state.integration_views.poll(&alpha, now).unwrap().intents;
        assert_eq!(intents[0].item_id, "opaque-0");
        assert_eq!(intents[0].epoch, alpha.epoch);
        assert_eq!(intents[0].revision, 7);
        assert!(state
            .integration_views
            .poll(&beta, later)
            .unwrap()
            .intents
            .is_empty());
        select_source(&mut state, "beta", later);
        assert!(!state.activate_integration_view_at(hit.x, hit.y, later));
        draw(&mut state, later);
        let hit = first_hit(&state);
        assert!(state.activate_integration_view_at(hit.x, hit.y, later));
        let intents = state.integration_views.poll(&beta, later).unwrap().intents;
        assert_eq!(intents[0].item_id, "opaque-0");
        assert_eq!(intents[0].epoch, beta.epoch);
        assert_eq!(intents[0].revision, 3);
        let expired = now + Duration::from_secs(5);
        state.integration_views.expire(expired);
        let buffer = draw(&mut state, expired);
        let text = text(&buffer, state.integration_views_area());
        assert!(text.contains("alpha: Unavailable"));
        assert!(text.contains("beta: Available"));
        assert!(text.contains("plain row 0"));
        assert_eq!(state.integration_view_selected.as_deref(), Some("beta"));
    }

    #[test]
    fn integration_views_sidebar_reorder_preserves_selection_leases_and_scroll_and_rejects_stale_hit(
    ) {
        let now = Instant::now();
        let mut state = state();
        state
            .integration_views
            .reconfigure(vec!["alpha".into(), "beta".into()])
            .unwrap();
        let alpha = open_source(&mut state, "alpha", now);
        let beta = open_source(&mut state, "beta", now);
        replace(&mut state, &alpha, 1, 128, now);
        replace(&mut state, &beta, 1, 128, now);
        draw(&mut state, now);
        state.scroll_integration_view(4);
        draw(&mut state, now);
        select_source(&mut state, "beta", now);
        draw(&mut state, now);
        state.scroll_integration_view(2);
        draw(&mut state, now);
        let hit = first_hit(&state);
        let before = state.integration_view_scrolls.clone();
        state
            .integration_views
            .reconfigure(vec!["beta".into(), "alpha".into()])
            .unwrap();
        state.reconcile_integration_views();
        assert!(!state.activate_integration_view_at(hit.x, hit.y, now));
        draw(&mut state, now);
        assert_eq!(state.integration_view_scrolls, before);
        assert_eq!(state.integration_view_selected.as_deref(), Some("beta"));
        assert_eq!(
            state.integration_views.project("alpha", now).unwrap().epoch,
            alpha.epoch
        );
        assert_eq!(
            state.integration_views.project("beta", now).unwrap().epoch,
            beta.epoch
        );
        select_source(&mut state, "alpha", now);
        draw(&mut state, now);
        assert_eq!(state.integration_view_scrolls["alpha"], 4);
        state
            .integration_views
            .reconfigure(vec!["beta".into()])
            .unwrap();
        state.reconcile_integration_views();
        assert_eq!(state.integration_view_selected.as_deref(), Some("beta"));
        assert_eq!(state.integration_view_scrolls.len(), 1);
        assert_eq!(state.integration_view_scrolls["beta"], 2);
    }

    #[test]
    fn integration_views_sidebar_eight_sources_keep_content_reachable_in_short_panels() {
        for height in [6, 8, 10, 12] {
            let now = Instant::now();
            let mut state = state();
            state.sidebar_section_split = 0.5;
            state.view.sidebar_rect.height = height * 2;
            state.view.workspace_card_areas =
                super::super::compute_workspace_card_areas(&state, state.view.sidebar_rect);
            state.mouse_capture = true;
            state.mode = crate::app::Mode::Terminal;
            state.active = Some(0);
            let sources = (0..8).map(|i| format!("source{i}")).collect::<Vec<_>>();
            state
                .integration_views
                .reconfigure(sources.clone())
                .unwrap();
            let owners = sources
                .iter()
                .map(|source| {
                    let owner = open_source(&mut state, source, now);
                    replace(&mut state, &owner, 1, 1, now);
                    owner
                })
                .collect::<Vec<_>>();
            draw(&mut state, now);
            assert_eq!(state.integration_views_area().height, height);

            for (selected, source) in sources.iter().enumerate() {
                let snapshot = state.integration_views_displayed.as_ref().unwrap();
                assert!(snapshot.sources.iter().any(|(id, _, _)| id == source));
                assert!(
                    !snapshot.rows.is_empty(),
                    "no item at height {height}, source {selected}"
                );
                if selected < 7 {
                    let selector = snapshot.sources.last().unwrap().2;
                    state.scroll_integration_views_at(selector.x, selector.y, 1);
                    assert!(state.integration_views_displayed.is_none());
                    draw(&mut state, now);
                }
            }

            assert_eq!(state.integration_view_selected.as_deref(), Some("source7"));
            let buffer = draw(&mut state, now);
            let snapshot = state.integration_views_displayed.as_ref().unwrap();
            assert_eq!(snapshot.source, "source7");
            assert_eq!(snapshot.epoch.as_deref(), Some(owners[7].epoch.as_str()));
            let hit = snapshot.rows[0].rect;
            assert!(hit.bottom() <= snapshot.area.bottom());
            assert!(text(&buffer, snapshot.area).contains("plain row 0"));
            assert!(text(&buffer, snapshot.area).contains("detail 0"));
            assert!(state.activate_integration_view_at(hit.x, hit.y + 1, now));
            for (index, owner) in owners.iter().enumerate() {
                let intents = state.integration_views.poll(owner, now).unwrap().intents;
                if index == 7 {
                    assert_eq!(intents.len(), 1);
                    assert_eq!(intents[0].item_id, "opaque-0");
                    assert_eq!(intents[0].epoch, owner.epoch);
                } else {
                    assert!(
                        intents.is_empty(),
                        "source {index} received another source's click"
                    );
                }
            }
        }
    }

    #[test]
    fn integration_views_sidebar_waiting_and_empty_are_distinct_and_selector_is_bounded() {
        let now = Instant::now();
        let mut state = state();
        state
            .integration_views
            .reconfigure((0..8).map(|i| format!("source{i}")).collect())
            .unwrap();
        let owner = open_source(&mut state, "source0", now);
        let buffer = draw(&mut state, now);
        assert!(text(&buffer, state.integration_views_area()).contains("Waiting for data"));
        replace(&mut state, &owner, 1, 0, now);
        let buffer = draw(&mut state, now);
        assert!(text(&buffer, state.integration_views_area()).contains("No items"));
        state.view.sidebar_rect.height = 12;
        draw(&mut state, now);
        for _ in 0..7 {
            let rect = state.integration_views_computed.as_ref().unwrap().sources[0].2;
            state.scroll_integration_views_at(rect.x, rect.y, 1);
            draw(&mut state, now);
        }
        assert_eq!(state.integration_view_selected.as_deref(), Some("source7"));
        let snapshot = state.integration_views_displayed.as_ref().unwrap();
        assert!(snapshot.sources.len() <= 8);
        assert!(snapshot.sources.iter().any(|(id, _, _)| id == "source7"));
        assert!(state.integration_view_scrolls.len() <= 8);
        assert!(state
            .integration_views
            .poll(&owner, now)
            .unwrap()
            .intents
            .is_empty());
    }

    #[test]
    fn integration_views_sidebar_default_is_generic_empty_without_native_rows() {
        let now = Instant::now();
        let mut state = state();
        let actual = draw(&mut state, now);
        let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
        terminal
            .draw(|frame| {
                super::super::render_sidebar(
                    &state,
                    &crate::terminal::TerminalRuntimeRegistry::new(),
                    frame,
                    state.view.sidebar_rect,
                )
            })
            .unwrap();
        assert_eq!(&actual, terminal.backend().buffer());
        assert!(state.integration_views_computed.is_some());
        let lower = text(&actual, state.integration_views_area());
        assert!(lower.contains("Integrations"));
        assert!(lower.contains("No integrations configured"));
        assert!(!lower.contains("agents"));
    }

    #[test]
    fn integration_views_sidebar_inactive_is_unavailable_without_native_fallback() {
        let now = Instant::now();
        let mut state = state();
        state.integration_views =
            crate::integration_views::IntegrationViewsState::new(vec!["test.source".into()]);
        let buffer = draw(&mut state, now);
        let lower = text(&buffer, state.integration_views_area());
        assert!(lower.contains("Integrations"));
        assert!(lower.contains("test.source"));
        assert!(lower.contains("Unavailable"));
        for native in ["agents", "grouped", "priority", "native-workspace"] {
            assert!(!lower.contains(native), "{native}");
        }
        assert!(state
            .integration_views_displayed
            .as_ref()
            .unwrap()
            .rows
            .is_empty());
    }

    #[test]
    fn integration_views_sidebar_plain_rows_need_no_detected_agents_and_keep_upper_workspace() {
        let now = Instant::now();
        let mut state = state();
        let baseline = draw(&mut state, now);
        let upper = super::super::expanded_sidebar_sections(
            state.view.sidebar_rect,
            state.sidebar_section_split,
        )
        .0;
        let cards = state.view.workspace_card_areas.clone();
        let owner = open(&mut state, now);
        replace(&mut state, &owner, 1, 1, now);
        let buffer = draw(&mut state, now);
        assert_eq!(text(&baseline, upper), text(&buffer, upper));
        assert_eq!(
            cards.iter().map(|c| c.rect).collect::<Vec<_>>(),
            state
                .view
                .workspace_card_areas
                .iter()
                .map(|c| c.rect)
                .collect::<Vec<_>>()
        );
        let lower = text(&buffer, state.integration_views_area());
        assert!(lower.contains("plain row 0"));
        assert!(lower.contains("detail 0"));
        assert!(!lower.contains("agents"));
    }

    #[test]
    fn integration_views_sidebar_activation_is_only_a_polled_opaque_intent() {
        let now = Instant::now();
        let mut state = state();
        let owner = open(&mut state, now);
        replace(&mut state, &owner, 1, 1, now);
        draw(&mut state, now);
        let target = (
            state.active,
            state.selected,
            state.mode,
            state.workspaces[0].active_tab,
            state.workspaces[0].focused_pane_id(),
        );
        let hit = first_hit(&state);
        assert!(state.activate_integration_view_at(hit.x, hit.y + 1, now));
        assert_eq!(
            target,
            (
                state.active,
                state.selected,
                state.mode,
                state.workspaces[0].active_tab,
                state.workspaces[0].focused_pane_id()
            )
        );
        let polled = state.integration_views.poll(&owner, now).unwrap();
        assert_eq!(polled.intents.len(), 1);
        assert_eq!(polled.intents[0].item_id, "opaque-0");
        assert!(state
            .integration_views
            .poll(&owner, now)
            .unwrap()
            .intents
            .is_empty());
    }

    #[test]
    fn integration_views_sidebar_replace_rejects_old_hits_until_a_successful_draw() {
        let now = Instant::now();
        let mut state = state();
        let owner = open(&mut state, now);
        replace(&mut state, &owner, 1, 1, now);
        draw(&mut state, now);
        let hit = first_hit(&state);
        replace(&mut state, &owner, 2, 1, now);
        assert!(!state.activate_integration_view_at(hit.x, hit.y, now));
        state.compute_integration_views_snapshot(now);
        assert!(!state.activate_integration_view_at(hit.x, hit.y, now));
        draw(&mut state, now);
        assert!(state.activate_integration_view_at(hit.x, hit.y, now));
        assert_eq!(
            state.integration_views.poll(&owner, now).unwrap().intents[0].revision,
            2
        );
    }

    #[test]
    fn integration_views_sidebar_clear_expiry_and_source_changes_have_no_overlay_or_hits() {
        let now = Instant::now();
        let mut state = state();
        let owner = open(&mut state, now);
        replace(&mut state, &owner, 1, 1, now);
        draw(&mut state, now);
        let hit = first_hit(&state);
        replace(&mut state, &owner, 2, 0, now);
        assert!(!state.activate_integration_view_at(hit.x, hit.y, now));
        let clear = draw(&mut state, now);
        assert!(!text(&clear, state.integration_views_area()).contains("plain row"));
        replace(&mut state, &owner, 3, 1, now);
        draw(&mut state, now);
        let expired = now + Duration::from_secs(5);
        assert!(!state.activate_integration_view_at(hit.x, hit.y, expired));
        let buffer = draw(&mut state, expired);
        let lower = text(&buffer, state.integration_views_area());
        assert!(lower.contains("Unavailable"));
        assert!(!lower.contains("plain row"));
        assert!(state
            .integration_views_displayed
            .as_ref()
            .unwrap()
            .rows
            .is_empty());
        state
            .integration_views
            .reconfigure(vec!["another.source".into()])
            .unwrap();
        assert!(!state.activate_integration_view_at(hit.x, hit.y, now));
        assert_eq!(state.mode, crate::app::Mode::Navigate);
    }

    #[test]
    fn integration_views_sidebar_scroll_and_geometry_changes_invalidate_hits() {
        let now = Instant::now();
        let mut state = state();
        let owner = open(&mut state, now);
        replace(&mut state, &owner, 1, 128, now);
        draw(&mut state, now);
        let hit = first_hit(&state);
        state.scroll_integration_view(i16::MAX);
        assert!(state.integration_view_scrolls["test.source"] < 128);
        assert!(!state.activate_integration_view_at(hit.x, hit.y, now));
        draw(&mut state, now);
        assert_eq!(
            state
                .integration_views_displayed
                .as_ref()
                .unwrap()
                .rows
                .last()
                .unwrap()
                .id,
            "opaque-127"
        );
        state.view.sidebar_rect.x += 3;
        assert!(!state.activate_integration_view_at(hit.x, hit.y, now));
        state.compute_integration_views_snapshot(now);
        assert!(state.integration_views_displayed.is_none());
        draw(&mut state, now);
        let moved = first_hit(&state);
        assert_eq!(moved.x, hit.x + 3);
        assert!(state.activate_integration_view_at(moved.x, moved.y, now));
        state.view.terminal_area.width += 1;
        assert!(!state.activate_integration_view_at(moved.x, moved.y, now));
        draw(&mut state, now);
        state.sidebar_section_split = 0.8;
        assert!(!state.activate_integration_view_at(moved.x, moved.y, now));
        draw(&mut state, now);
        state.sidebar_collapsed = true;
        assert!(!state.activate_integration_view_at(moved.x, moved.y, now));
    }

    #[test]
    fn integration_views_sidebar_tiny_unicode_bidi_and_control_text_are_bounded() {
        let now = Instant::now();
        let mut state = state();
        let owner = open(&mut state, now);
        state
            .integration_views
            .replace(
                &ViewReplaceParams {
                    source: owner.source.clone(),
                    lease: owner.lease.clone(),
                    epoch: owner.epoch.clone(),
                    revision: 1,
                    items: vec![IntegrationViewItem {
                        id: "opaque".into(),
                        label: "\u{65e5}\u{672c}\u{202e}label".into(),
                        detail: "\u{e9}\u{1f980} detail".into(),
                    }],
                },
                now,
            )
            .unwrap();
        let buffer = draw(&mut state, now);
        let lower = text(&buffer, state.integration_views_area());
        assert!(lower.contains("\\u{202e}"));
        assert!(!lower.contains('\u{202e}'));
        assert_eq!(
            crate::annotations::render_text("\n\u{1b}", 100),
            "\\u{a}\\u{1b}"
        );
        for width in 0..5 {
            for height in 0..8 {
                state.view.sidebar_rect = Rect::new(0, 0, width, height);
                state.view.workspace_card_areas.clear();
                draw(&mut state, now);
                assert!(state
                    .integration_views_displayed
                    .as_ref()
                    .unwrap()
                    .rows
                    .is_empty());
            }
        }
    }

    #[test]
    fn integration_views_sidebar_render_is_pure() {
        let now = Instant::now();
        let mut state = state();
        let owner = open(&mut state, now);
        replace(&mut state, &owner, 1, 1, now);
        state.compute_integration_views_snapshot(now);
        let before = state.integration_views_computed.clone();
        let deadline = state.integration_views.next_deadline();
        let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
        terminal
            .draw(|frame| render(&state, frame, state.integration_views_area()))
            .unwrap();
        assert_eq!(state.integration_views_computed, before);
        assert!(state.integration_views_displayed.is_none());
        assert_eq!(state.integration_views.next_deadline(), deadline);
        assert!(state
            .integration_views
            .poll(&owner, now)
            .unwrap()
            .intents
            .is_empty());
    }
}
