use crate::backend::detect_backend;
use crate::model::{BackendKind, WorkspaceCapture};
use crate::store::{ItemKind, Store};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs};
use std::io;
use std::time::Duration;

pub fn run(store: &Store, requested_backend: Option<BackendKind>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, store, requested_backend);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    store: &Store,
    requested_backend: Option<BackendKind>,
) -> Result<()> {
    let mut app = TuiSnapshot::load(store, requested_backend);
    loop {
        terminal.draw(|frame| {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(7),
                ])
                .split(frame.area());

            let tabs = Tabs::new(["Live", "Templates"])
                .select(app.active_tab.index())
                .block(Block::default().borders(Borders::BOTTOM))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            frame.render_widget(tabs, rows[0]);

            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(rows[1]);

            let list = List::new(app.active_list_items())
                .block(
                    Block::default()
                        .title(app.active_list_title())
                        .borders(Borders::ALL),
                )
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol("▶ ");
            let mut list_state = app.active_list_state();
            frame.render_stateful_widget(list, columns[0], &mut list_state);

            let details = Paragraph::new(app.detail_text(store))
                .block(Block::default().title(app.detail_title()).borders(Borders::ALL))
                .scroll((app.detail_scroll, 0));
            frame.render_widget(details, columns[1]);

            let help = Paragraph::new(
                "q/Esc quit    r refresh    Tab switch Live/Templates\nh/l focus list/metadata    ↑/↓ or j/k select or scroll\n\nMetadata scrolls when the right pane is focused.",
            )
            .block(Block::default().title("Help").borders(Borders::ALL))
            .style(Style::default().add_modifier(Modifier::DIM));
            frame.render_widget(help, rows[2]);
        })?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('r') => app = TuiSnapshot::load(store, requested_backend),
                    KeyCode::Tab => app.select_next_tab(),
                    KeyCode::BackTab => app.select_previous_tab(),
                    KeyCode::Char('h') | KeyCode::Left => app.focus_browser(),
                    KeyCode::Char('l') | KeyCode::Right => app.focus_detail(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::PageDown => app.scroll_detail_down(10),
                    KeyCode::PageUp => app.scroll_detail_up(10),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct TuiSnapshot {
    active_tab: ActiveTab,
    active_pane: ActivePane,
    live_items: Vec<LiveItem>,
    selected_live: usize,
    saved_items: Vec<SavedTemplateItem>,
    selected_saved: usize,
    detail_scroll: u16,
}

impl TuiSnapshot {
    fn load(store: &Store, requested_backend: Option<BackendKind>) -> Self {
        Self {
            active_tab: ActiveTab::Live,
            active_pane: ActivePane::Browser,
            live_items: live_state_items(requested_backend),
            selected_live: 0,
            saved_items: saved_template_items(store),
            selected_saved: 0,
            detail_scroll: 0,
        }
    }

    fn active_list_items(&self) -> Vec<ListItem<'_>> {
        match self.active_tab {
            ActiveTab::Live => {
                if self.live_items.is_empty() {
                    vec![ListItem::new("no live backend state available")]
                } else {
                    self.live_items
                        .iter()
                        .map(|item| ListItem::new(item.display.as_str()))
                        .collect()
                }
            }
            ActiveTab::Templates => {
                if self.saved_items.is_empty() {
                    vec![ListItem::new("no saved templates yet")]
                } else {
                    self.saved_items
                        .iter()
                        .map(|item| ListItem::new(item.display.as_str()))
                        .collect()
                }
            }
        }
    }

    fn active_list_title(&self) -> String {
        let title = match self.active_tab {
            ActiveTab::Live => "Live backend state",
            ActiveTab::Templates => "Saved Kitsune templates",
        };
        if self.active_pane == ActivePane::Browser {
            format!("{title} *")
        } else {
            title.into()
        }
    }

    fn detail_title(&self) -> &'static str {
        if self.active_pane == ActivePane::Detail {
            "Metadata *"
        } else {
            "Metadata"
        }
    }

    fn active_list_state(&self) -> ListState {
        let mut state = ListState::default();
        match self.active_tab {
            ActiveTab::Live if !self.live_items.is_empty() => {
                state.select(Some(self.selected_live.min(self.live_items.len() - 1)));
            }
            ActiveTab::Templates if !self.saved_items.is_empty() => {
                state.select(Some(self.selected_saved.min(self.saved_items.len() - 1)));
            }
            _ => {}
        }
        state
    }

    fn select_next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
        self.detail_scroll = 0;
    }

    fn select_previous_tab(&mut self) {
        self.active_tab = self.active_tab.previous();
        self.detail_scroll = 0;
    }

    fn focus_browser(&mut self) {
        self.active_pane = ActivePane::Browser;
    }

    fn focus_detail(&mut self) {
        self.active_pane = ActivePane::Detail;
    }

    fn move_down(&mut self) {
        match self.active_pane {
            ActivePane::Browser => self.select_next(),
            ActivePane::Detail => self.scroll_detail_down(1),
        }
    }

    fn move_up(&mut self) {
        match self.active_pane {
            ActivePane::Browser => self.select_previous(),
            ActivePane::Detail => self.scroll_detail_up(1),
        }
    }

    fn select_next(&mut self) {
        match self.active_tab {
            ActiveTab::Live if !self.live_items.is_empty() => {
                self.selected_live = (self.selected_live + 1) % self.live_items.len();
                self.detail_scroll = 0;
            }
            ActiveTab::Templates if !self.saved_items.is_empty() => {
                self.selected_saved = (self.selected_saved + 1) % self.saved_items.len();
                self.detail_scroll = 0;
            }
            _ => {}
        }
    }

    fn select_previous(&mut self) {
        match self.active_tab {
            ActiveTab::Live if !self.live_items.is_empty() => {
                self.selected_live = if self.selected_live == 0 {
                    self.live_items.len() - 1
                } else {
                    self.selected_live - 1
                };
                self.detail_scroll = 0;
            }
            ActiveTab::Templates if !self.saved_items.is_empty() => {
                self.selected_saved = if self.selected_saved == 0 {
                    self.saved_items.len() - 1
                } else {
                    self.selected_saved - 1
                };
                self.detail_scroll = 0;
            }
            _ => {}
        };
    }

    fn scroll_detail_down(&mut self, amount: u16) {
        self.detail_scroll = self.detail_scroll.saturating_add(amount);
    }

    fn scroll_detail_up(&mut self, amount: u16) {
        self.detail_scroll = self.detail_scroll.saturating_sub(amount);
    }

    fn detail_text(&self, store: &Store) -> String {
        match self.active_tab {
            ActiveTab::Live => self
                .live_items
                .get(self.selected_live)
                .map(|item| item.detail.clone())
                .unwrap_or_else(|| "No live backend state available.".into()),
            ActiveTab::Templates => {
                let Some(item) = self.saved_items.get(self.selected_saved) else {
                    return "No saved template selected.".into();
                };
                store
                    .show(item.kind, &item.name)
                    .unwrap_or_else(|err| format!("failed to read {}: {err}", item.display))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ActiveTab {
    Live,
    Templates,
}

impl ActiveTab {
    fn index(self) -> usize {
        match self {
            ActiveTab::Live => 0,
            ActiveTab::Templates => 1,
        }
    }

    fn next(self) -> Self {
        match self {
            ActiveTab::Live => ActiveTab::Templates,
            ActiveTab::Templates => ActiveTab::Live,
        }
    }

    fn previous(self) -> Self {
        self.next()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivePane {
    Browser,
    Detail,
}

#[derive(Debug, Clone)]
struct SavedTemplateItem {
    kind: ItemKind,
    name: String,
    display: String,
}

#[derive(Debug, Clone)]
struct LiveItem {
    display: String,
    detail: String,
}

fn live_state_items(requested_backend: Option<BackendKind>) -> Vec<LiveItem> {
    let backend = match detect_backend(requested_backend) {
        Ok(backend) => backend,
        Err(err) => {
            return vec![LiveItem {
                display: "backend unavailable".into(),
                detail: format!("backend unavailable: {err}"),
            }];
        }
    };

    match backend.capture_all_workspaces() {
        Ok(workspaces) => summarize_workspaces(&workspaces),
        Err(err) => vec![LiveItem {
            display: format!("{} live capture failed", backend.kind()),
            detail: format!("{} live capture failed: {err}", backend.kind()),
        }],
    }
}

fn summarize_workspaces(workspaces: &[WorkspaceCapture]) -> Vec<LiveItem> {
    let mut items = vec![];
    for workspace in workspaces {
        items.push(LiveItem {
            display: format!(
                "workspace {} [{}]",
                workspace.workspace.label_or_name(),
                count_label(workspace.tabs.len(), "tab")
            ),
            detail: to_yaml(workspace),
        });
        for tab in &workspace.tabs {
            items.push(LiveItem {
                display: format!(
                    "  tab {} [{}]",
                    tab.tab.label_or_name(),
                    count_label(tab.panes.len(), "pane")
                ),
                detail: to_yaml(tab),
            });
            for pane in &tab.panes {
                items.push(LiveItem {
                    display: format!("    pane {}", pane.label_or_name()),
                    detail: to_yaml(pane),
                });
            }
        }
    }
    items
}

fn to_yaml<T: serde::Serialize>(value: &T) -> String {
    serde_yaml::to_string(value)
        .unwrap_or_else(|err| format!("failed to serialize metadata: {err}"))
}

fn saved_template_items(store: &Store) -> Vec<SavedTemplateItem> {
    let mut items = vec![];
    for kind in [
        ItemKind::Workspace,
        ItemKind::Tab,
        ItemKind::Pane,
        ItemKind::Stack,
        ItemKind::Snapshot,
    ] {
        if let Ok(names) = store.list(kind) {
            for name in names {
                items.push(SavedTemplateItem {
                    kind,
                    display: format!("{}/{}", kind.singular_name(), name),
                    name,
                });
            }
        }
    }
    items
}

fn count_label(count: usize, singular: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {singular}s")
    }
}
