use crate::backend::detect_backend;
use crate::model::{
    BackendKind, PaneTemplate, StackTemplate, TabCapture, TabTemplate, WorkspaceCapture,
    WorkspaceTemplate,
};
use crate::store::{ItemKind, Store};
use anyhow::{Result, bail};
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
use std::collections::HashSet;
use std::fs;
use std::io;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
                format!(
                    "q/Esc quit    r refresh    s save    Space select    Enter restore/apply    c compare\ne open/edit temp metadata    y/n confirm save    Tab switch Live/Templates\nh/l focus    ↑/↓ or j/k select or scroll\n\n{}",
                    app.status
                ),
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
                    KeyCode::Char('s') => {
                        match crate::save_current_workspace_to_store(store, requested_backend) {
                            Ok(message) => {
                                app = TuiSnapshot::load(store, requested_backend);
                                app.status = message;
                            }
                            Err(err) => app.status = format!("save failed: {err}"),
                        }
                    }
                    KeyCode::Tab => app.select_next_tab(),
                    KeyCode::BackTab => app.select_previous_tab(),
                    KeyCode::Char('h') | KeyCode::Left => app.focus_browser(),
                    KeyCode::Char('l') | KeyCode::Right => app.focus_detail(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::PageDown => app.scroll_detail_down(10),
                    KeyCode::PageUp => app.scroll_detail_up(10),
                    KeyCode::Char(' ') => app.toggle_saved_selection(),
                    KeyCode::Enter => app.restore_selected_template(store, requested_backend),
                    KeyCode::Char('c') => app.diff_selected_template(store),
                    KeyCode::Char('e') => {
                        app.edit_current_metadata(terminal, store, requested_backend)?
                    }
                    KeyCode::Char('y') => app.confirm_pending_edit(store, requested_backend),
                    KeyCode::Char('n') => app.discard_pending_edit(),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum TempEditOutcome {
    Unchanged,
    Changed,
}

fn edit_contents_in_nvim(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    original: &str,
) -> Result<(TempEditOutcome, String)> {
    let temp_path = temp_metadata_path();
    fs::write(&temp_path, original)?;

    let edit_result = run_nvim_for_path(terminal, &temp_path);
    if let Err(err) = edit_result {
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }

    let edited = fs::read_to_string(&temp_path)?;
    let _ = fs::remove_file(&temp_path);

    if edited == original {
        Ok((TempEditOutcome::Unchanged, edited))
    } else {
        Ok((TempEditOutcome::Changed, edited))
    }
}

fn persist_saved_edit(store: &Store, edit: &PendingEdit) -> Result<()> {
    let path = store.path(edit.item.kind, &edit.item.name);
    validate_yaml_for_kind(edit.item.kind, &edit.edited)?;
    fs::write(&path, &edit.edited)?;

    let report = crate::validate::validate_store(store)?;
    if report.passes(false) {
        return Ok(());
    }

    fs::write(&path, &edit.original)?;
    bail!(
        "validation failed; restored original. {}",
        validation_error_summary(&report)
    );
}

fn apply_live_metadata(
    edit: &PendingLiveEdit,
    requested_backend: Option<BackendKind>,
) -> Result<String> {
    let backend = detect_backend(requested_backend)?;
    match edit {
        PendingLiveEdit::Workspace(workspace) => {
            backend.apply_workspace_metadata(workspace, false)?;
            Ok(format!(
                "applied live workspace metadata '{}' via {}",
                workspace.label_or_name(),
                backend.kind()
            ))
        }
        PendingLiveEdit::Tab(tab) => {
            backend.apply_tab_metadata(tab, false)?;
            Ok(format!(
                "applied live tab metadata '{}' via {}",
                tab.label_or_name(),
                backend.kind()
            ))
        }
        PendingLiveEdit::Pane(pane) => {
            backend.apply_pane_metadata(pane, false)?;
            Ok(format!(
                "applied live pane metadata '{}' via {}",
                pane.label_or_name(),
                backend.kind()
            ))
        }
    }
}

fn diff_saved_template_against_live(
    store: &Store,
    live_workspaces: &[WorkspaceCapture],
    item: &SavedTemplateItem,
) -> Result<String> {
    match item.kind {
        ItemKind::Workspace => {
            let saved = store.load_workspace_capture(&item.name)?;
            let workspace_id = saved
                .workspace
                .backend_ref
                .as_ref()
                .and_then(|ref_| ref_.workspace_id.as_deref())
                .ok_or_else(|| {
                    anyhow::anyhow!("saved workspace has no backend_ref.workspace_id")
                })?;
            let live = live_workspaces
                .iter()
                .find(|workspace| {
                    workspace
                        .workspace
                        .backend_ref
                        .as_ref()
                        .and_then(|ref_| ref_.workspace_id.as_deref())
                        == Some(workspace_id)
                })
                .ok_or_else(|| anyhow::anyhow!("no live workspace matches {workspace_id}"))?;
            Ok(line_diff(&to_yaml(&saved), &to_yaml(live)))
        }
        ItemKind::Tab => {
            let saved = store.load_tab_capture(&item.name)?;
            let tab_id = saved
                .tab
                .backend_ref
                .as_ref()
                .and_then(|ref_| ref_.tab_id.as_deref())
                .ok_or_else(|| anyhow::anyhow!("saved tab has no backend_ref.tab_id"))?;
            let live = live_workspaces
                .iter()
                .flat_map(|workspace| workspace.tabs.iter())
                .find(|tab| {
                    tab.tab
                        .backend_ref
                        .as_ref()
                        .and_then(|ref_| ref_.tab_id.as_deref())
                        == Some(tab_id)
                })
                .ok_or_else(|| anyhow::anyhow!("no live tab matches {tab_id}"))?;
            Ok(line_diff(&to_yaml(&saved), &to_yaml(live)))
        }
        ItemKind::Pane => {
            let saved = store.load_pane(&item.name)?;
            let pane_id = saved
                .backend_ref
                .as_ref()
                .and_then(|ref_| ref_.pane_id.as_deref())
                .ok_or_else(|| anyhow::anyhow!("saved pane has no backend_ref.pane_id"))?;
            let live = live_workspaces
                .iter()
                .flat_map(|workspace| workspace.tabs.iter())
                .flat_map(|tab| tab.panes.iter())
                .find(|pane| {
                    pane.backend_ref
                        .as_ref()
                        .and_then(|ref_| ref_.pane_id.as_deref())
                        == Some(pane_id)
                })
                .ok_or_else(|| anyhow::anyhow!("no live pane matches {pane_id}"))?;
            Ok(line_diff(&to_yaml(&saved), &to_yaml(live)))
        }
        ItemKind::Stack | ItemKind::Snapshot => bail!(
            "{} templates do not have direct live metadata to diff",
            item.kind.singular_name()
        ),
    }
}

fn line_diff(saved: &str, live: &str) -> String {
    if saved == live {
        return "saved and live metadata match".into();
    }

    let saved_lines: Vec<&str> = saved.lines().collect();
    let live_lines: Vec<&str> = live.lines().collect();
    let max = saved_lines.len().max(live_lines.len());
    let mut out = vec![
        "--- saved".to_string(),
        "+++ live".to_string(),
        "".to_string(),
    ];
    for idx in 0..max {
        match (saved_lines.get(idx), live_lines.get(idx)) {
            (Some(saved), Some(live)) if saved == live => out.push(format!("  {saved}")),
            (Some(saved), Some(live)) => {
                out.push(format!("- {saved}"));
                out.push(format!("+ {live}"));
            }
            (Some(saved), None) => out.push(format!("- {saved}")),
            (None, Some(live)) => out.push(format!("+ {live}")),
            (None, None) => {}
        }
    }
    out.join("\n")
}

fn restore_template_selection_to_live(
    store: &Store,
    requested_backend: Option<BackendKind>,
    selection: &[SavedTemplateKey],
) -> Result<String> {
    let mut completed = vec![];
    for key in selection {
        match crate::restore_saved_template_to_live(store, requested_backend, key.kind, &key.name) {
            Ok(message) => completed.push(message),
            Err(err) => {
                if completed.is_empty() {
                    bail!("{} '{}' failed: {err}", key.kind.singular_name(), key.name);
                }
                bail!(
                    "{} '{}' failed after {} completed: {err}",
                    key.kind.singular_name(),
                    key.name,
                    completed.len()
                );
            }
        }
    }

    if completed.len() == 1 {
        Ok(completed.remove(0))
    } else {
        Ok(format!(
            "restored/applied {} selected templates",
            completed.len()
        ))
    }
}

fn restore_arm_message(selection: &[SavedTemplateKey]) -> String {
    if selection.len() == 1 {
        let item = &selection[0];
        format!(
            "Press Enter again to restore/apply {} '{}'.",
            item.kind.singular_name(),
            item.name
        )
    } else {
        format!(
            "Press Enter again to restore/apply {} selected templates.",
            selection.len()
        )
    }
}

fn validate_yaml_for_kind(kind: ItemKind, contents: &str) -> Result<()> {
    match kind {
        ItemKind::Workspace => {
            serde_yaml::from_str::<WorkspaceTemplate>(contents)?;
        }
        ItemKind::Tab => {
            serde_yaml::from_str::<TabTemplate>(contents)?;
        }
        ItemKind::Pane => {
            serde_yaml::from_str::<PaneTemplate>(contents)?;
        }
        ItemKind::Stack => {
            serde_yaml::from_str::<StackTemplate>(contents)?;
        }
        ItemKind::Snapshot => {
            serde_yaml::from_str::<serde_yaml::Value>(contents)?;
        }
    }
    Ok(())
}

fn validation_error_summary(report: &crate::validate::ValidationReport) -> String {
    let errors: Vec<String> = report
        .issues
        .iter()
        .filter(|issue| issue.severity == crate::validate::Severity::Error)
        .take(3)
        .map(|issue| issue.message.clone())
        .collect();
    if errors.is_empty() {
        "no error details available".into()
    } else {
        errors.join("; ")
    }
}

fn run_nvim_for_path(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    path: &std::path::Path,
) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    let mut command = Command::new("nvim");
    command.arg("+setlocal noswapfile filetype=yaml");
    let status = command.arg(path).status();

    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;

    let status = status?;
    if !status.success() {
        bail!("nvim exited with status {status}");
    }
    Ok(())
}

fn temp_metadata_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "kitsune-metadata-{}-{nanos}.yaml",
        std::process::id()
    ))
}

#[derive(Debug, Clone)]
struct TuiSnapshot {
    active_tab: ActiveTab,
    active_pane: ActivePane,
    live_workspaces: Vec<WorkspaceCapture>,
    live_items: Vec<LiveItem>,
    selected_live: usize,
    saved_items: Vec<SavedTemplateItem>,
    selected_saved: usize,
    selected_saved_items: HashSet<SavedTemplateKey>,
    detail_scroll: u16,
    status: String,
    detail_override: Option<String>,
    pending_restore: Option<Vec<SavedTemplateKey>>,
    pending_edit: Option<PendingEdit>,
    pending_live_edit: Option<PendingLiveEdit>,
}

impl TuiSnapshot {
    fn load(store: &Store, requested_backend: Option<BackendKind>) -> Self {
        let (live_workspaces, live_items) = live_state_items(requested_backend);
        Self {
            active_tab: ActiveTab::Live,
            active_pane: ActivePane::Browser,
            live_workspaces,
            live_items,
            selected_live: 0,
            saved_items: saved_template_items(store),
            selected_saved: 0,
            selected_saved_items: HashSet::new(),
            detail_scroll: 0,
            status: "Metadata scrolls when the right pane is focused.".into(),
            detail_override: None,
            pending_restore: None,
            pending_edit: None,
            pending_live_edit: None,
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
                        .map(|item| {
                            let mark = if self.selected_saved_items.contains(&item.key()) {
                                "[x]"
                            } else {
                                "[ ]"
                            };
                            ListItem::new(format!("{mark} {}", item.display))
                        })
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
        let title =
            if self.active_tab == ActiveTab::Templates && !self.selected_saved_items.is_empty() {
                format!("{title} ({} selected)", self.selected_saved_items.len())
            } else {
                title.into()
            };
        if self.active_pane == ActivePane::Browser {
            format!("{title} *")
        } else {
            title
        }
    }

    fn detail_title(&self) -> &'static str {
        if self.detail_override.is_some() {
            return if self.active_pane == ActivePane::Detail {
                "Diff *"
            } else {
                "Diff"
            };
        }
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
        self.detail_override = None;
        self.pending_restore = None;
        self.pending_edit = None;
        self.pending_live_edit = None;
    }

    fn select_previous_tab(&mut self) {
        self.active_tab = self.active_tab.previous();
        self.detail_scroll = 0;
        self.detail_override = None;
        self.pending_restore = None;
        self.pending_edit = None;
        self.pending_live_edit = None;
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
                self.detail_override = None;
                self.pending_restore = None;
                self.pending_edit = None;
                self.pending_live_edit = None;
            }
            ActiveTab::Templates if !self.saved_items.is_empty() => {
                self.selected_saved = (self.selected_saved + 1) % self.saved_items.len();
                self.detail_scroll = 0;
                self.detail_override = None;
                self.pending_restore = None;
                self.pending_edit = None;
                self.pending_live_edit = None;
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
                self.detail_override = None;
                self.pending_restore = None;
                self.pending_edit = None;
                self.pending_live_edit = None;
            }
            ActiveTab::Templates if !self.saved_items.is_empty() => {
                self.selected_saved = if self.selected_saved == 0 {
                    self.saved_items.len() - 1
                } else {
                    self.selected_saved - 1
                };
                self.detail_scroll = 0;
                self.detail_override = None;
                self.pending_restore = None;
                self.pending_edit = None;
                self.pending_live_edit = None;
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

    fn toggle_saved_selection(&mut self) {
        if self.active_tab != ActiveTab::Templates {
            self.status = "Switch to Templates to multi-select saved templates.".into();
            return;
        }

        let Some(item) = self.saved_items.get(self.selected_saved) else {
            self.status = "No saved template selected.".into();
            return;
        };

        if !item.is_restorable() {
            self.status = format!(
                "{} templates cannot be selected for restore/apply.",
                item.kind.singular_name()
            );
            self.pending_restore = None;
            return;
        }

        let key = item.key();
        if self.selected_saved_items.remove(&key) {
            self.status = format!("unselected {}", item.display);
        } else {
            self.selected_saved_items.insert(key);
            self.status = format!("selected {}", item.display);
        }
        self.pending_restore = None;
    }

    fn restore_selected_template(&mut self, store: &Store, requested_backend: Option<BackendKind>) {
        if self.active_tab != ActiveTab::Templates {
            self.status =
                "Switch to Templates and select a workspace, tab, or stack to restore.".into();
            return;
        }

        let Some(item) = self.saved_items.get(self.selected_saved).cloned() else {
            self.status = "No saved template selected.".into();
            return;
        };

        let selection = self.restore_selection(item.clone());

        if selection.is_empty() {
            self.status = format!(
                "{} templates cannot be restored directly.",
                item.kind.singular_name()
            );
            self.pending_restore = None;
            return;
        }

        if self.pending_restore.as_ref() != Some(&selection) {
            self.pending_restore = Some(selection.clone());
            self.status = restore_arm_message(&selection);
            return;
        }

        let clear_selection_after_success = !self.selected_saved_items.is_empty();
        match restore_template_selection_to_live(store, requested_backend, &selection) {
            Ok(message) => {
                self.status = message;
                if clear_selection_after_success {
                    self.selected_saved_items.clear();
                }
            }
            Err(err) => self.status = format!("restore failed: {err}"),
        }
        self.pending_restore = None;
    }

    fn restore_selection(&self, fallback: SavedTemplateItem) -> Vec<SavedTemplateKey> {
        if self.selected_saved_items.is_empty() {
            return fallback
                .is_restorable()
                .then(|| fallback.key())
                .into_iter()
                .collect();
        }

        self.saved_items
            .iter()
            .filter(|item| self.selected_saved_items.contains(&item.key()) && item.is_restorable())
            .map(SavedTemplateItem::key)
            .collect()
    }

    fn diff_selected_template(&mut self, store: &Store) {
        if self.active_tab != ActiveTab::Templates {
            self.status =
                "Switch to Templates and select a saved template to diff against live state."
                    .into();
            return;
        }

        let Some(item) = self.saved_items.get(self.selected_saved).cloned() else {
            self.status = "No saved template selected.".into();
            return;
        };

        match diff_saved_template_against_live(store, &self.live_workspaces, &item) {
            Ok(diff) => {
                self.detail_override = Some(diff);
                self.detail_scroll = 0;
                self.status = format!("diff: saved {} vs matching live metadata", item.display);
            }
            Err(err) => {
                self.detail_override = None;
                self.status = format!("diff failed: {err}");
            }
        }
    }

    fn edit_current_metadata(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        store: &Store,
        _requested_backend: Option<BackendKind>,
    ) -> Result<()> {
        if self.detail_override.is_some() {
            self.status =
                "Diff view is read-only; move selection or refresh to return to metadata.".into();
            return Ok(());
        }

        let original = self.detail_text(store);
        let outcome = edit_contents_in_nvim(terminal, &original);
        let (outcome, edited) = match outcome {
            Ok(outcome) => outcome,
            Err(err) => {
                self.status = format!("edit failed: {err}");
                return Ok(());
            }
        };

        if matches!(outcome, TempEditOutcome::Unchanged) {
            self.status = "unchanged".into();
            self.pending_edit = None;
            self.pending_live_edit = None;
            return Ok(());
        }

        if self.active_tab != ActiveTab::Templates {
            self.pending_live_edit = match self.selected_live_target() {
                Some(LiveTarget::Workspace(_)) => {
                    match serde_yaml::from_str::<WorkspaceCapture>(&edited) {
                        Ok(capture) => Some(PendingLiveEdit::Workspace(capture.workspace)),
                        Err(err) => {
                            self.status = format!("live workspace edit failed validation: {err}");
                            None
                        }
                    }
                }
                Some(LiveTarget::Tab(_, _)) => match serde_yaml::from_str::<TabCapture>(&edited) {
                    Ok(capture) => Some(PendingLiveEdit::Tab(capture.tab)),
                    Err(err) => {
                        self.status = format!("live tab edit failed validation: {err}");
                        None
                    }
                },
                Some(LiveTarget::Pane(_, _, _)) => {
                    match serde_yaml::from_str::<PaneTemplate>(&edited) {
                        Ok(pane) => Some(PendingLiveEdit::Pane(pane)),
                        Err(err) => {
                            self.status = format!("live pane edit failed validation: {err}");
                            None
                        }
                    }
                }
                Some(LiveTarget::Message(_)) | None => {
                    self.status = "No editable live component selected.".into();
                    None
                }
            };
            if self.pending_live_edit.is_some() {
                self.status =
                    "Live metadata change detected. Press y to apply to Herdr, n to discard."
                        .into();
            }
            self.pending_edit = None;
            return Ok(());
        }

        let Some(item) = self.saved_items.get(self.selected_saved).cloned() else {
            self.status = "changed metadata has no saved template target; not persisted".into();
            self.pending_edit = None;
            self.pending_live_edit = None;
            return Ok(());
        };

        self.pending_edit = Some(PendingEdit {
            item: item.clone(),
            original,
            edited,
        });
        self.status = format!(
            "Changes detected for {}. Press y to save after validation, n to discard.",
            item.display
        );
        self.pending_live_edit = None;
        Ok(())
    }

    fn confirm_pending_edit(&mut self, store: &Store, requested_backend: Option<BackendKind>) {
        if let Some(edit) = self.pending_live_edit.take() {
            match apply_live_metadata(&edit, requested_backend) {
                Ok(message) => {
                    let selected_live = self.selected_live;
                    let (live_workspaces, live_items) = live_state_items(requested_backend);
                    self.live_workspaces = live_workspaces;
                    self.live_items = live_items;
                    if !self.live_items.is_empty() {
                        self.selected_live = selected_live.min(self.live_items.len() - 1);
                    }
                    self.detail_scroll = 0;
                    self.detail_override = None;
                    self.status = message;
                }
                Err(err) => self.status = format!("live metadata sync failed: {err}"),
            }
            return;
        }

        let Some(edit) = self.pending_edit.take() else {
            self.status = "No pending edit to save.".into();
            return;
        };

        match persist_saved_edit(store, &edit) {
            Ok(()) => {
                self.saved_items = saved_template_items(store);
                if !self.saved_items.is_empty() {
                    self.selected_saved = self.selected_saved.min(self.saved_items.len() - 1);
                }
                self.detail_scroll = 0;
                self.detail_override = None;
                self.status = format!("saved and validated: {}", edit.item.display);
                if matches!(
                    edit.item.kind,
                    ItemKind::Workspace | ItemKind::Tab | ItemKind::Pane
                ) {
                    match crate::apply_saved_template_metadata_to_live(
                        store,
                        requested_backend,
                        edit.item.kind,
                        &edit.item.name,
                    ) {
                        Ok(message) => self.status = format!("{}; {message}", self.status),
                        Err(err) => {
                            self.status =
                                format!("{}; live metadata sync failed: {err}", self.status)
                        }
                    }
                }
            }
            Err(err) => self.status = format!("edit failed: {err}"),
        }
    }

    fn discard_pending_edit(&mut self) {
        if self.pending_edit.take().is_some() || self.pending_live_edit.take().is_some() {
            self.status = "discarded pending edit".into();
        } else {
            self.status = "No pending edit to discard.".into();
        }
    }

    fn selected_live_target(&self) -> Option<&LiveTarget> {
        self.live_items
            .get(self.selected_live)
            .map(|item| &item.target)
    }

    fn detail_text(&self, store: &Store) -> String {
        if let Some(detail) = &self.detail_override {
            return detail.clone();
        }
        match self.active_tab {
            ActiveTab::Live => self
                .live_items
                .get(self.selected_live)
                .map(|item| item.detail_text(&self.live_workspaces))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl SavedTemplateItem {
    fn key(&self) -> SavedTemplateKey {
        SavedTemplateKey {
            kind: self.kind,
            name: self.name.clone(),
        }
    }

    fn is_restorable(&self) -> bool {
        matches!(
            self.kind,
            ItemKind::Workspace | ItemKind::Tab | ItemKind::Stack
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SavedTemplateKey {
    kind: ItemKind,
    name: String,
}

#[derive(Debug, Clone)]
struct PendingEdit {
    item: SavedTemplateItem,
    original: String,
    edited: String,
}

#[derive(Debug, Clone)]
enum PendingLiveEdit {
    Workspace(WorkspaceTemplate),
    Tab(TabTemplate),
    Pane(PaneTemplate),
}

#[derive(Debug, Clone)]
struct LiveItem {
    display: String,
    target: LiveTarget,
}

#[derive(Debug, Clone)]
enum LiveTarget {
    Workspace(usize),
    Tab(usize, usize),
    Pane(usize, usize, usize),
    Message(String),
}

impl LiveItem {
    fn detail_text(&self, workspaces: &[WorkspaceCapture]) -> String {
        match &self.target {
            LiveTarget::Workspace(workspace_idx) => workspaces
                .get(*workspace_idx)
                .map(to_yaml)
                .unwrap_or_else(|| "live workspace no longer available".into()),
            LiveTarget::Tab(workspace_idx, tab_idx) => workspaces
                .get(*workspace_idx)
                .and_then(|workspace| workspace.tabs.get(*tab_idx))
                .map(to_yaml)
                .unwrap_or_else(|| "live tab no longer available".into()),
            LiveTarget::Pane(workspace_idx, tab_idx, pane_idx) => workspaces
                .get(*workspace_idx)
                .and_then(|workspace| workspace.tabs.get(*tab_idx))
                .and_then(|tab| tab.panes.get(*pane_idx))
                .map(to_yaml)
                .unwrap_or_else(|| "live pane no longer available".into()),
            LiveTarget::Message(message) => message.clone(),
        }
    }
}

fn live_state_items(
    requested_backend: Option<BackendKind>,
) -> (Vec<WorkspaceCapture>, Vec<LiveItem>) {
    let backend = match detect_backend(requested_backend) {
        Ok(backend) => backend,
        Err(err) => {
            return (
                vec![],
                vec![LiveItem {
                    display: "backend unavailable".into(),
                    target: LiveTarget::Message(format!("backend unavailable: {err}")),
                }],
            );
        }
    };

    match backend.capture_all_workspaces() {
        Ok(workspaces) => {
            let items = summarize_workspaces(&workspaces);
            (workspaces, items)
        }
        Err(err) => (
            vec![],
            vec![LiveItem {
                display: format!("{} live load failed", backend.kind()),
                target: LiveTarget::Message(format!("{} live load failed: {err}", backend.kind())),
            }],
        ),
    }
}

fn summarize_workspaces(workspaces: &[WorkspaceCapture]) -> Vec<LiveItem> {
    let mut items = vec![];
    for (workspace_idx, workspace) in workspaces.iter().enumerate() {
        items.push(LiveItem {
            display: format!(
                "workspace {} [{}]",
                workspace.workspace.label_or_name(),
                count_label(workspace.tabs.len(), "tab")
            ),
            target: LiveTarget::Workspace(workspace_idx),
        });
        for (tab_idx, tab) in workspace.tabs.iter().enumerate() {
            items.push(LiveItem {
                display: format!(
                    "  tab {} [{}]",
                    tab.tab.label_or_name(),
                    count_label(tab.panes.len(), "pane")
                ),
                target: LiveTarget::Tab(workspace_idx, tab_idx),
            });
            for (pane_idx, pane) in tab.panes.iter().enumerate() {
                items.push(LiveItem {
                    display: format!("    pane {}", pane.label_or_name()),
                    target: LiveTarget::Pane(workspace_idx, tab_idx, pane_idx),
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
