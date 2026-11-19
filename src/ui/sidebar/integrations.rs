//! Plain-text peer projection and successful-frame activation fence.
use super::sections::{self, SectionId, SectionLayout};
use crate::app::{state::ViewLayout, AppState};
use ratatui::{
    layout::{Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IntegrationViewsSnapshot {
    sidebar: Rect,
    terminal: Rect,
    compact: bool,
    layout: SectionLayout,
    collapsed: std::collections::HashSet<SectionId>,
    offset: usize,
    focus: SectionId,
    configured: Vec<String>,
    scrolls: std::collections::HashMap<String, usize>,
    workspace_scroll: usize,
    peers: Vec<Peer>,
}

#[derive(Clone, Debug, PartialEq)]
struct Peer {
    source: String,
    epoch: Option<String>,
    revision: u64,
    body: Rect,
    max_scroll: usize,
    message: &'static str,
    rows: Vec<Row>,
}

#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: String,
    rect: Rect,
    label: String,
    detail: String,
    presentation: Option<crate::api::schema::IntegrationViewPresentation>,
}

/// A background render may compute a smaller/mobile view, but must not adopt
/// any of its clamps or replace the foreground client's successful-frame fence.
pub(crate) struct SidebarPresentation {
    view: crate::app::state::ViewState,
    layout: SectionLayout,
    collapsed: std::collections::HashSet<SectionId>,
    focus: SectionId,
    offset: usize,
    scrolls: std::collections::HashMap<String, usize>,
    workspace_scroll: usize,
    tab_scroll: usize,
    mobile_scroll: usize,
    computed: Option<IntegrationViewsSnapshot>,
    displayed: Option<IntegrationViewsSnapshot>,
    inspector: crate::ui::integration_inspector::Inspector,
}

impl AppState {
    pub(crate) fn save_sidebar_presentation(&self) -> SidebarPresentation {
        SidebarPresentation {
            view: self.view.clone(),
            layout: self.sidebar_sections.clone(),
            collapsed: self.collapsed_sidebar_sections.clone(),
            focus: self.sidebar_section_focus.clone(),
            offset: self.sidebar_section_offset,
            scrolls: self.integration_view_scrolls.clone(),
            workspace_scroll: self.workspace_scroll,
            tab_scroll: self.tab_scroll,
            mobile_scroll: self.mobile_switcher_scroll,
            computed: self.integration_views_computed.clone(),
            displayed: self.integration_views_displayed.clone(),
            inspector: self.integration_inspector.clone(),
        }
    }

    pub(crate) fn restore_sidebar_presentation(&mut self, saved: SidebarPresentation) {
        self.view = saved.view;
        self.sidebar_sections = saved.layout;
        self.collapsed_sidebar_sections = saved.collapsed;
        self.sidebar_section_focus = saved.focus;
        self.sidebar_section_offset = saved.offset;
        self.integration_view_scrolls = saved.scrolls;
        self.workspace_scroll = saved.workspace_scroll;
        self.tab_scroll = saved.tab_scroll;
        self.mobile_switcher_scroll = saved.mobile_scroll;
        self.integration_views_computed = saved.computed;
        self.integration_views_displayed = saved.displayed;
        self.integration_inspector = saved.inspector;
    }

    pub(crate) fn compute_sidebar_sections(&mut self, area: Rect) {
        self.reconcile_integration_views();
        self.sidebar_sections = sections::compute(
            area,
            &sections::identities(self.integration_views.configured_sources()),
            &self.collapsed_sidebar_sections,
            self.sidebar_section_offset,
        );
    }

    pub(crate) fn spaces_body(&self) -> Rect {
        if self.collapsed_sidebar_sections.contains(&SectionId::Spaces) {
            return Rect::default();
        }
        self.sidebar_sections.body(&SectionId::Spaces)
    }

    // A convenience surface for tests; input routes by typed section rectangles.
    #[cfg(test)]
    pub(crate) fn integration_views_area(&self) -> Rect {
        if self.view.layout != ViewLayout::Desktop {
            return Rect::default();
        }
        let area = self.view.sidebar_rect;
        Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width.saturating_sub(1),
            area.height.saturating_sub(2),
        )
    }

    pub(crate) fn integration_section_at(&self, col: u16, row: u16) -> bool {
        self.sidebar_sections.sections.iter().any(|s| {
            matches!(s.id, SectionId::Integration(_))
                && (s.header.contains(Position::new(col, row))
                    || s.body.contains(Position::new(col, row)))
        })
    }

    pub(crate) fn toggle_sidebar_section_at(&mut self, col: u16, row: u16) -> bool {
        if self
            .sidebar_new_button_rect()
            .contains(Position::new(col, row))
        {
            return false;
        }
        let Some(section) = self
            .sidebar_sections
            .sections
            .iter()
            .find(|s| s.header.contains(Position::new(col, row)))
        else {
            return false;
        };
        let id = section.id.clone();
        if !self.collapsed_sidebar_sections.remove(&id) {
            self.collapsed_sidebar_sections.insert(id.clone());
        }
        self.sidebar_section_focus = id;
        self.integration_views_displayed = None;
        self.mark_session_dirty();
        true
    }

    pub(crate) fn compute_integration_views_snapshot(&mut self, now: Instant) {
        let configured = self.integration_views.configured_sources().to_vec();
        let descriptor = self.integration_views.descriptor(now);
        let mut peers = Vec::new();
        if self.view.layout == ViewLayout::Desktop {
            for section in &self.sidebar_sections.sections {
                let SectionId::Integration(source) = &section.id else {
                    continue;
                };
                let body = section.body;
                let projection = self.integration_views.project(source, now);
                let epoch = projection.as_ref().map(|p| p.epoch.to_owned()).or_else(|| {
                    descriptor
                        .sources
                        .iter()
                        .find(|s| &s.id == source)
                        .and_then(|s| s.epoch.clone())
                });
                let row_height = 1;
                let capacity = usize::from(body.height / row_height);
                let count = projection.as_ref().map_or(0, |p| p.items.len().min(128));
                let max_scroll = count.saturating_sub(capacity.max(1));
                let requested = self
                    .integration_view_scrolls
                    .get(source)
                    .copied()
                    .unwrap_or(0);
                let scroll = if count > 0 && body.height > 0 {
                    requested.min(max_scroll)
                } else {
                    requested
                };
                if count > 0 && body.height > 0 {
                    self.integration_view_scrolls.insert(source.clone(), scroll);
                }
                let mut peer = Peer {
                    source: source.clone(),
                    epoch: epoch.clone(),
                    revision: projection.as_ref().map_or(0, |p| p.revision),
                    body,
                    max_scroll,
                    message: if epoch.is_none() {
                        "Unavailable"
                    } else if projection.as_ref().is_none_or(|p| p.revision == 0) {
                        "Waiting for data"
                    } else if count == 0 {
                        "No items"
                    } else {
                        ""
                    },
                    rows: Vec::new(),
                };
                if body.width > 0 {
                    if let Some(projection) = projection {
                        for (index, item) in projection
                            .items
                            .iter()
                            .take(128)
                            .skip(scroll)
                            .take(capacity)
                            .enumerate()
                        {
                            peer.rows.push(Row {
                                id: item.id.clone(),
                                rect: Rect::new(
                                    body.x,
                                    body.y + index as u16 * row_height,
                                    body.width,
                                    row_height,
                                ),
                                label: crate::annotations::render_text(
                                    &crate::ui::integration_visual::item_label(item),
                                    body.width.saturating_sub(2) as usize,
                                ),
                                detail: crate::annotations::render_text(
                                    &item.detail,
                                    body.width as usize,
                                ),
                                presentation: item.presentation.clone(),
                            });
                        }
                    }
                }
                peers.push(peer);
            }
        }
        let snapshot = IntegrationViewsSnapshot {
            sidebar: self.view.sidebar_rect,
            terminal: self.view.terminal_area,
            compact: self.sidebar_collapsed,
            layout: self.sidebar_sections.clone(),
            collapsed: self.collapsed_sidebar_sections.clone(),
            offset: self.sidebar_section_offset,
            focus: self.sidebar_section_focus.clone(),
            configured,
            scrolls: self.integration_view_scrolls.clone(),
            workspace_scroll: self.workspace_scroll,
            peers,
        };
        if self.integration_views_displayed.as_ref() != Some(&snapshot) {
            self.integration_views_displayed = None;
        }
        self.integration_views_computed = Some(snapshot);
    }

    pub(crate) fn commit_integration_views_render(&mut self) {
        self.integration_views_displayed = self.integration_views_computed.clone();
        self.integration_inspector.displayed = self.integration_inspector.computed.clone();
    }

    pub(crate) fn activate_integration_view_at(
        &mut self,
        col: u16,
        row: u16,
        now: Instant,
    ) -> bool {
        let Some(s) = self.integration_views_displayed.as_ref() else {
            return false;
        };
        if self.view.layout != ViewLayout::Desktop
            || s.sidebar != self.view.sidebar_rect
            || s.terminal != self.view.terminal_area
            || s.compact != self.sidebar_collapsed
            || s.layout != self.sidebar_sections
            || s.collapsed != self.collapsed_sidebar_sections
            || s.offset != self.sidebar_section_offset
            || s.focus != self.sidebar_section_focus
            || s.scrolls != self.integration_view_scrolls
            || s.workspace_scroll != self.workspace_scroll
            || s.configured != self.integration_views.configured_sources()
        {
            return false;
        }
        for peer in &s.peers {
            if let Some(hit) = peer
                .rows
                .iter()
                .find(|hit| hit.rect.contains(Position::new(col, row)))
            {
                let Some(epoch) = peer.epoch.as_deref() else {
                    return false;
                };
                return self.integration_views.activate(
                    &peer.source,
                    epoch,
                    peer.revision,
                    &hit.id,
                    now,
                );
            }
        }
        false
    }

    pub(crate) fn scroll_integration_views_at(&mut self, col: u16, row: u16, delta: i16) {
        let point = Position::new(col, row);
        if let Some(section) = self
            .sidebar_sections
            .sections
            .iter()
            .find(|s| s.header.contains(point))
        {
            let ids = sections::identities(self.integration_views.configured_sources());
            let current = ids.iter().position(|id| id == &section.id).unwrap_or(0);
            let next = current
                .saturating_add_signed(delta as isize)
                .min(ids.len().saturating_sub(1));
            self.sidebar_section_focus = ids[next].clone();
            self.sidebar_section_offset = next;
        } else if let Some(peer) = self
            .integration_views_computed
            .as_ref()
            .and_then(|s| s.peers.iter().find(|p| p.body.contains(point)))
        {
            let scroll = self
                .integration_view_scrolls
                .entry(peer.source.clone())
                .or_default();
            *scroll = scroll
                .saturating_add_signed(delta as isize)
                .min(peer.max_scroll);
        }
        self.integration_views_displayed = None;
    }

    pub(crate) fn reconcile_integration_views(&mut self) {
        let sources = self.integration_views.configured_sources();
        if self
            .integration_views_computed
            .as_ref()
            .is_some_and(|s| s.configured != sources)
        {
            let ids = sections::identities(sources);
            self.sidebar_section_offset = ids
                .iter()
                .position(|id| id == &self.sidebar_section_focus)
                .unwrap_or(0);
            self.integration_views_displayed = None;
        }
        self.integration_view_scrolls
            .retain(|id, _| sources.contains(id));
        self.collapsed_sidebar_sections.retain(|id| match id {
            SectionId::Spaces => true,
            SectionId::Integration(id) => sources.contains(id),
        });
        let ids = sections::identities(sources);
        if !ids.contains(&self.sidebar_section_focus) {
            self.sidebar_section_focus = SectionId::Spaces;
        }
        self.sidebar_section_offset = self.sidebar_section_offset.min(ids.len().saturating_sub(1));
    }
}

pub(super) fn render(app: &AppState, frame: &mut Frame, _: Rect) {
    let Some(snapshot) = app.integration_views_computed.as_ref() else {
        return;
    };
    for peer in &snapshot.peers {
        if peer.body.width == 0 || peer.body.height == 0 {
            continue;
        }
        if !peer.message.is_empty() {
            frame.render_widget(
                Paragraph::new(peer.message).style(Style::default().fg(app.palette.overlay0)),
                Rect::new(peer.body.x, peer.body.y, peer.body.width, 1),
            );
        }
        for hit in &peer.rows {
            let (glyph, color) = crate::ui::integration_visual::visual(
                hit.presentation.as_ref().and_then(|p| p.status),
                app.spinner_tick,
                app.integration_animations,
                &app.palette,
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(glyph, Style::default().fg(color)),
                    Span::raw(" "),
                    Span::styled(hit.label.as_str(), Style::default().fg(app.palette.text)),
                ])),
                Rect::new(hit.rect.x, hit.rect.y, hit.rect.width, 1),
            );
        }
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

    #[test]
    fn empty_captions_render_only_icon_and_name_and_duplicate_names_activate_by_id() {
        use crate::api::schema::{IntegrationViewPresentation, IntegrationViewStatus};
        let now = Instant::now();
        let mut state = state(&["coven"]);
        let opened = state
            .integration_views
            .open(
                &ViewOpenParams {
                    source: "coven".into(),
                    version: 2,
                },
                now,
            )
            .unwrap();
        let owner = ViewOwnerParams {
            source: "coven".into(),
            lease: opened.lease,
            epoch: opened.epoch,
        };
        state
            .integration_views
            .replace(
                &ViewReplaceParams {
                    source: owner.source.clone(),
                    lease: owner.lease.clone(),
                    epoch: owner.epoch.clone(),
                    revision: 1,
                    items: ["Djinn", "Djinn", "My pane"]
                        .into_iter()
                        .enumerate()
                        .map(|(n, label)| IntegrationViewItem {
                            information: None,
                            id: format!("session-hash-{n}"),
                            label: label.into(),
                            detail: "provenance remains in details".into(),
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
        let buffer = draw(&mut state, now, 30);
        let rows = state.integration_views_displayed.as_ref().unwrap().peers[0]
            .rows
            .clone();
        for (row, expected) in rows.iter().zip(["⠋ Djinn", "⠋ Djinn", "· My pane"]) {
            let rendered: String = (row.rect.x..row.rect.right())
                .map(|x| buffer[(x, row.rect.y)].symbol())
                .collect();
            assert_eq!(rendered.trim_end(), expected);
            assert!(state.activate_integration_view_at(row.rect.x, row.rect.y, now));
            let intents = state.integration_views.poll(&owner, now).unwrap().intents;
            assert_eq!(intents.len(), 1);
            assert_eq!(intents[0].item_id, row.id);
        }
    }

    #[test]
    fn declared_statuses_render_one_line_with_host_palette_and_no_inline_detail() {
        use crate::api::schema::{IntegrationViewPresentation, IntegrationViewStatus::*};
        let now = Instant::now();
        let mut state = state(&["coven"]);
        let owner = state
            .integration_views
            .open(
                &ViewOpenParams {
                    source: "coven".into(),
                    version: 2,
                },
                now,
            )
            .unwrap();
        state
            .integration_views
            .replace(
                &ViewReplaceParams {
                    source: "coven".into(),
                    lease: owner.lease,
                    epoch: owner.epoch,
                    revision: 1,
                    items: [
                        Some(Pending),
                        Some(Running),
                        Some(Attention),
                        Some(Error),
                        Some(Success),
                        Some(Unknown),
                        None,
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(n, status)| IntegrationViewItem {
                        information: None,
                        id: n.to_string(),
                        label: "idle ended".into(),
                        detail: "DETAIL ONLY IN INSPECTOR".into(),
                        presentation: Some(IntegrationViewPresentation {
                            label: "Djinn".into(),
                            status,
                        }),
                    })
                    .collect(),
                },
                now,
            )
            .unwrap();
        let first = draw(&mut state, now, 30);
        let peer = &state.integration_views_displayed.as_ref().unwrap().peers[0];
        assert_eq!(peer.rows.len(), 7);
        for (row, (glyph, color)) in peer.rows.iter().zip([
            ("◌", state.palette.subtext0),
            ("⠋", state.palette.accent),
            ("!", state.palette.yellow),
            ("×", state.palette.red),
            ("✓", state.palette.green),
            ("?", state.palette.overlay0),
            ("·", state.palette.overlay0),
        ]) {
            assert_eq!(row.rect.height, 1);
            assert_eq!(first[(row.rect.x, row.rect.y)].symbol(), glyph);
            assert_eq!(first[(row.rect.x, row.rect.y)].fg, color);
        }
        let rendered = text(&first);
        assert!(rendered.contains("Djinn  idle ended"));
        assert!(!rendered.contains("DETAIL ONLY"));
        let snapshot = state.integration_views_displayed.clone();
        state.spinner_tick += 8;
        let second = draw(&mut state, now, 30);
        assert_eq!(snapshot, state.integration_views_displayed);
        assert_ne!(first, second);
        state.integration_animations = false;
        let fixed = draw(&mut state, now, 30);
        state.spinner_tick += 8;
        assert_eq!(fixed, draw(&mut state, now, 30));
    }

    fn state(sources: &[&str]) -> AppState {
        let mut state = AppState::test_new();
        state.workspaces = vec![crate::workspace::Workspace::test_new("native-workspace")];
        state.ensure_test_terminals();
        state.mouse_capture = true;
        state
            .integration_views
            .reconfigure(sources.iter().map(|s| s.to_string()).collect())
            .unwrap();
        state
    }

    fn open(state: &mut AppState, source: &str, now: Instant) -> ViewOwnerParams {
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
                            information: None,
                            id: format!("opaque-{i}"),
                            label: format!("{} label {i}", owner.source),
                            detail: format!("{} detail {i}", owner.source),
                            presentation: None,
                        })
                        .collect(),
                },
                now,
            )
            .unwrap();
    }

    fn compute(state: &mut AppState, now: Instant, width: u16, height: u16) {
        state.view.sidebar_rect = Rect::new(0, 0, width, height);
        state.view.terminal_area = Rect::new(width, 0, 80, height);
        state.compute_sidebar_sections(state.view.sidebar_rect);
        state.workspace_scroll = super::super::normalized_workspace_scroll(
            state,
            state.view.sidebar_rect,
            state.workspace_scroll,
        );
        state.view.workspace_card_areas =
            super::super::compute_workspace_card_areas(state, state.view.sidebar_rect);
        state.compute_integration_views_snapshot(now);
    }

    fn draw(state: &mut AppState, now: Instant, height: u16) -> Buffer {
        compute(state, now, 36, height);
        let buffer = render_buffer(state);
        state.commit_integration_views_render();
        buffer
    }

    fn render_buffer(state: &AppState) -> Buffer {
        let mut terminal =
            Terminal::new(TestBackend::new(116, state.view.sidebar_rect.height.max(1))).unwrap();
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
        terminal.backend().buffer().clone()
    }

    fn text(buffer: &Buffer) -> String {
        buffer.content.iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn actual_compact_headers_name_spaces_coven_and_build_without_ellipsis_or_selector() {
        let now = Instant::now();
        let mut state = state(&["coven", "build"]);
        let coven = open(&mut state, "coven", now);
        let build = open(&mut state, "build", now);
        replace(&mut state, &coven, 1, 1, now);
        replace(&mut state, &build, 1, 1, now);
        state.sidebar_collapsed = true;
        crate::ui::compute_view(&mut state, Rect::new(0, 0, 106, 30));
        assert_eq!(state.view.sidebar_rect.width, 4);
        let buffer = render_buffer(&state);
        let headers = state
            .sidebar_sections
            .sections
            .iter()
            .map(|s| {
                (s.header.x..s.header.right())
                    .map(|x| buffer[(x, s.header.y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert_eq!(headers, ["▾Sp", "▾Co", "▾bu"]);
        let rendered = text(&buffer);
        for old in [
            "Integrations",
            "Available",
            "> coven:",
            "No integrations configured",
            "▾ …",
        ] {
            assert!(!rendered.contains(old));
        }
        let header = state.sidebar_sections.sections[1].header;
        assert!(state.toggle_sidebar_section_at(header.x, header.y));
        crate::ui::compute_view(&mut state, Rect::new(0, 0, 106, 30));
        let buffer = render_buffer(&state);
        let header = state.sidebar_sections.sections[1].header;
        let collapsed = (header.x..header.right())
            .map(|x| buffer[(x, header.y)].symbol())
            .collect::<String>();
        assert_eq!(collapsed, "▸Co");
        assert_eq!(
            state.collapsed_sidebar_sections,
            std::collections::HashSet::from([SectionId::Integration("coven".into())])
        );
        assert!(state
            .integration_views
            .poll(&coven, now)
            .unwrap()
            .intents
            .is_empty());
        assert!(state
            .integration_views
            .poll(&build, now)
            .unwrap()
            .intents
            .is_empty());
    }

    #[test]
    fn compact_collision_tokens_stay_unique_across_all_nine_headers_and_overflow_windows() {
        let now = Instant::now();
        let sources = [
            "spaces", "Spaces", "Coven", "build", "BUILD", "coal", "spare", "coven",
        ];
        let mut state = state(&sources);
        state.sidebar_collapsed = true;
        for width in [2, 3, 4] {
            for usable_height in [1, 3, 8] {
                state.sidebar_section_offset = 0;
                state.sidebar_section_focus = SectionId::Spaces;
                let configured = state.integration_views.configured_sources().to_vec();
                let ids = sections::identities(&configured);
                let tokens = sections::compact_labels(&configured, width - 1);
                let mut seen = std::collections::HashMap::new();
                for step in 0..9 {
                    compute(&mut state, now, width, usable_height + 2);
                    let buffer = render_buffer(&state);
                    for section in &state.sidebar_sections.sections {
                        let label = (section.header.x..section.header.right())
                            .map(|x| buffer[(x, section.header.y)].symbol())
                            .collect::<String>();
                        assert!(
                            !label.trim().is_empty() && !label.contains('…'),
                            "{label:?}"
                        );
                        let index = ids.iter().position(|id| id == &section.id).unwrap();
                        let expected = if width == 2 {
                            tokens[index].clone()
                        } else {
                            format!("▾{}", tokens[index])
                        };
                        assert_eq!(label.trim_end(), expected);
                        if let Some(previous) = seen.insert(section.id.clone(), label.clone()) {
                            assert_eq!(previous, label);
                        }
                    }
                    if step < 8 {
                        let header = state.sidebar_sections.sections[0].header;
                        state.scroll_integration_views_at(header.x, header.y, 1);
                    }
                }
                assert_eq!(seen.len(), 9);
                assert_eq!(
                    seen.values()
                        .map(|label| label.to_ascii_lowercase())
                        .collect::<std::collections::HashSet<_>>()
                        .len(),
                    9
                );
                assert_eq!(state.integration_views.configured_sources(), configured);
            }
        }
    }

    fn hit(state: &AppState, source: &str) -> Rect {
        state
            .integration_views_displayed
            .as_ref()
            .unwrap()
            .peers
            .iter()
            .find(|p| p.source == source)
            .unwrap()
            .rows[0]
            .rect
    }

    #[test]
    fn spaces_coven_and_build_render_concurrently_without_selector_or_status_preamble() {
        let now = Instant::now();
        let mut state = state(&["coven", "build"]);
        for source in ["coven", "build"] {
            let owner = open(&mut state, source, now);
            replace(&mut state, &owner, 1, 1, now);
        }
        let buffer = draw(&mut state, now, 32);
        let text = text(&buffer);
        for label in [
            "Spaces",
            "Coven",
            "build",
            "native-workspace",
            "coven label 0",
            "build label 0",
            "new",
            "menu",
        ] {
            assert!(text.contains(label), "missing {label}: {text}");
        }
        for old in [
            "Integrations",
            "Available",
            "No integrations configured",
            "> coven:",
        ] {
            assert!(!text.contains(old));
        }
        let heights = state
            .sidebar_sections
            .sections
            .iter()
            .map(|s| s.body.height)
            .collect::<Vec<_>>();
        assert!(heights.iter().max().unwrap() - heights.iter().min().unwrap() <= 1);
        assert_eq!(
            state.view.workspace_card_areas[0].rect.y,
            state.spaces_body().y
        );
        let before = state.integration_views_computed.clone();
        let mut terminal = Terminal::new(TestBackend::new(116, 32)).unwrap();
        terminal
            .draw(|f| {
                super::super::render_sidebar(
                    &state,
                    &crate::terminal::TerminalRuntimeRegistry::new(),
                    f,
                    state.view.sidebar_rect,
                )
            })
            .unwrap();
        assert_eq!(before, state.integration_views_computed);
    }

    #[test]
    fn no_sources_only_spaces_and_messages_only_in_expanded_peer_bodies() {
        let now = Instant::now();
        let mut state = state(&[]);
        assert!(!text(&draw(&mut state, now, 20)).contains("Integrations"));
        assert_eq!(state.sidebar_sections.sections.len(), 1);
        state
            .integration_views
            .reconfigure(vec!["coven".into()])
            .unwrap();
        assert!(text(&draw(&mut state, now, 20)).contains("Unavailable"));
        let owner = open(&mut state, "coven", now);
        assert!(text(&draw(&mut state, now, 20)).contains("Waiting for data"));
        replace(&mut state, &owner, 1, 0, now);
        assert!(text(&draw(&mut state, now, 20)).contains("No items"));
        let header = state.sidebar_sections.sections[1].header;
        assert!(state.toggle_sidebar_section_at(header.x, header.y));
        assert!(!text(&draw(&mut state, now, 20)).contains("No items"));
        assert!(state
            .integration_views
            .poll(&owner, now)
            .unwrap()
            .intents
            .is_empty());
    }

    #[test]
    fn identical_item_ids_are_isolated_and_both_lines_activate_exact_epoch_revision() {
        let now = Instant::now();
        let mut state = state(&["alpha", "beta"]);
        let alpha = open(&mut state, "alpha", now);
        let beta = open(&mut state, "beta", now + Duration::from_millis(1));
        replace(&mut state, &alpha, 7, 1, now);
        replace(&mut state, &beta, 3, 1, now);
        draw(&mut state, now, 30);
        let native = (
            state.active,
            state.selected,
            state.mode,
            state.workspaces[0].focused_pane_id(),
        );
        for (source, owner, revision, line) in [("alpha", &alpha, 7, 0), ("beta", &beta, 3, 0)] {
            let hit = hit(&state, source);
            assert!(state.activate_integration_view_at(hit.x, hit.y + line, now));
            let intents = state.integration_views.poll(owner, now).unwrap().intents;
            assert_eq!(intents.len(), 1);
            assert_eq!(intents[0].item_id, "opaque-0");
            assert_eq!(intents[0].epoch, owner.epoch);
            assert_eq!(intents[0].revision, revision);
        }
        assert_eq!(
            native,
            (
                state.active,
                state.selected,
                state.mode,
                state.workspaces[0].focused_pane_id()
            )
        );
    }

    #[test]
    fn collapse_scroll_window_resize_and_config_changes_fence_old_hits() {
        let now = Instant::now();
        let mut state = state(&["alpha", "beta"]);
        let alpha = open(&mut state, "alpha", now);
        let beta = open(&mut state, "beta", now);
        replace(&mut state, &alpha, 1, 128, now);
        replace(&mut state, &beta, 1, 128, now);
        draw(&mut state, now, 30);
        let old = hit(&state, "beta");
        state.scroll_integration_views_at(old.x, old.y, 2);
        assert_eq!(state.integration_view_scrolls["beta"], 2);
        assert_eq!(state.integration_view_scrolls["alpha"], 0);
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
        compute(&mut state, now, 36, 30);
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
        draw(&mut state, now, 30);
        let header = state.sidebar_sections.sections[2].header;
        assert!(state.toggle_sidebar_section_at(header.x, header.y));
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
        draw(&mut state, now, 30);
        assert_eq!(state.integration_view_scrolls["beta"], 2);
        let header = state.sidebar_sections.sections[2].header;
        assert!(state.toggle_sidebar_section_at(header.x, header.y));
        draw(&mut state, now, 30);
        let old = hit(&state, "beta");
        state
            .integration_views
            .reconfigure(vec!["beta".into(), "alpha".into()])
            .unwrap();
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
        draw(&mut state, now, 30);
        assert_eq!(state.integration_view_scrolls["beta"], 2);
        let old = hit(&state, "beta");
        state.view.sidebar_rect.width += 1;
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
        draw(&mut state, now, 30);
        let old = hit(&state, "beta");
        state.sidebar_section_offset += 1;
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
        draw(&mut state, now, 30);
        state.sidebar_collapsed = true;
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
    }

    #[test]
    fn replace_clear_expiry_and_missing_draw_reject_activation() {
        let now = Instant::now();
        let mut state = state(&["coven"]);
        let owner = open(&mut state, "coven", now);
        replace(&mut state, &owner, 1, 1, now);
        draw(&mut state, now, 30);
        let old = hit(&state, "coven");
        replace(&mut state, &owner, 2, 1, now);
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
        compute(&mut state, now, 36, 30);
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
        draw(&mut state, now, 30);
        assert!(state.activate_integration_view_at(old.x, old.y, now));
        assert_eq!(
            state.integration_views.poll(&owner, now).unwrap().intents[0].revision,
            2
        );
        replace(&mut state, &owner, 3, 0, now);
        assert!(!state.activate_integration_view_at(old.x, old.y, now));
        replace(&mut state, &owner, 4, 1, now);
        draw(&mut state, now, 30);
        assert!(!state.activate_integration_view_at(old.x, old.y, now + Duration::from_secs(5)));
        let text = text(&draw(&mut state, now + Duration::from_secs(5), 30));
        assert!(text.contains("Unavailable"));
        assert!(!text.contains("coven label"));
    }

    #[test]
    fn every_body_row_is_one_item_including_odd_spare_capacity() {
        let now = Instant::now();
        let mut state = state(&["coven"]);
        let owner = open(&mut state, "coven", now);
        replace(&mut state, &owner, 1, 128, now);
        for body_height in 1..=7 {
            draw(&mut state, now, 4 + body_height * 2);
            let peer = &state.integration_views_displayed.as_ref().unwrap().peers[0];
            assert_eq!(peer.body.height, body_height);
            assert!(peer.rows.iter().all(|r| r.rect.height == 1));
            let body = peer.body;
            if body_height > 1 && body_height % 2 == 1 {
                assert!(state.activate_integration_view_at(body.x, body.bottom() - 1, now));
            }
            let first = hit(&state, "coven");
            assert!(state.activate_integration_view_at(first.x, first.y, now));
            state.integration_views.poll(&owner, now).unwrap();
        }
    }

    #[test]
    fn source_identity_removal_prunes_only_removed_ui_state_and_readd_is_expanded() {
        let now = Instant::now();
        let mut state = state(&["spaces", "Coven", "coven"]);
        draw(&mut state, now, 30);
        state.collapsed_sidebar_sections.extend([
            SectionId::Spaces,
            SectionId::Integration("spaces".into()),
            SectionId::Integration("coven".into()),
        ]);
        state
            .integration_view_scrolls
            .extend([("spaces".into(), 4), ("coven".into(), 2)]);
        state
            .integration_views
            .reconfigure(vec!["spaces".into()])
            .unwrap();
        state.reconcile_integration_views();
        assert_eq!(state.collapsed_sidebar_sections.len(), 2);
        assert_eq!(state.integration_view_scrolls.len(), 1);
        state
            .integration_views
            .reconfigure(vec!["spaces".into(), "coven".into()])
            .unwrap();
        state.reconcile_integration_views();
        assert!(!state
            .collapsed_sidebar_sections
            .contains(&SectionId::Integration("coven".into())));
        assert!(state
            .collapsed_sidebar_sections
            .contains(&SectionId::Spaces));
    }

    #[test]
    fn literal_source_labels_and_item_text_are_escaped_without_provider_style() {
        let now = Instant::now();
        let mut state = state(&["spaces", "Spaces", "Coven", "coven", "build"]);
        let labels = text(&draw(&mut state, now, 40));
        for label in [
            "spaces [source:spaces]",
            "Spaces [source:Spaces]",
            "Coven [coven]",
            "▾ Coven",
            "▾ build",
        ] {
            assert!(labels.contains(label), "{labels}");
        }
        assert!(!labels.contains("Build"));
        let owner = open(&mut state, "build", now);
        state
            .integration_views
            .replace(
                &ViewReplaceParams {
                    source: owner.source.clone(),
                    lease: owner.lease.clone(),
                    epoch: owner.epoch.clone(),
                    revision: 1,
                    items: vec![IntegrationViewItem {
                        information: None,
                        id: "opaque".into(),
                        label: "label\u{202e}".into(),
                        detail: "detail".into(),
                        presentation: None,
                    }],
                },
                now,
            )
            .unwrap();
        let buffer = draw(&mut state, now, 40);
        let rendered = text(&buffer);
        assert!(rendered.contains("\\u{202e}"));
        assert_eq!(
            crate::annotations::render_text("\u{1b}[31m", 40),
            "\\u{1b}[31m"
        );
        assert!(!rendered.contains('\u{202e}'));
        for width in 0..=4 {
            for height in 0..=8 {
                compute(&mut state, now, width, height);
                state.commit_integration_views_render();
                let snapshot = state.integration_views_displayed.as_ref().unwrap();
                for peer in &snapshot.peers {
                    for row in &peer.rows {
                        assert!(row.rect.width > 0);
                        assert!(
                            row.rect.bottom() <= state.view.sidebar_rect.bottom().saturating_sub(1)
                        );
                    }
                }
                if width < 2 || height < 4 {
                    assert!(snapshot.peers.iter().all(|p| p.rows.is_empty()));
                }
            }
        }
    }
}
