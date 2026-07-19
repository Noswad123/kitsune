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
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
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
                .constraints([Constraint::Min(5), Constraint::Length(7)])
                .split(frame.area());
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(rows[0]);

            let live = List::new(lines_or_empty(&app.live_lines, "no live backend state available"))
                .block(Block::default().title("Live backend state").borders(Borders::ALL));
            frame.render_widget(live, columns[0]);

            let saved = List::new(lines_or_empty(&app.saved_lines, "no saved templates yet"))
                .block(Block::default().title("Saved Kitsune templates").borders(Borders::ALL));
            frame.render_widget(saved, columns[1]);

            let help = Paragraph::new(
                "q / Esc  quit    r  refresh\n\nRead-only TUI foundation:\n- live backend topology\n- saved template inventory\n\nCapture/restore/view/edit actions come next.",
            )
            .block(Block::default().title("Help").borders(Borders::ALL))
            .style(Style::default().add_modifier(Modifier::DIM));
            frame.render_widget(help, rows[1]);
        })?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('r') => app = TuiSnapshot::load(store, requested_backend),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct TuiSnapshot {
    live_lines: Vec<String>,
    saved_lines: Vec<String>,
}

impl TuiSnapshot {
    fn load(store: &Store, requested_backend: Option<BackendKind>) -> Self {
        Self {
            live_lines: live_state_lines(requested_backend),
            saved_lines: saved_template_lines(store),
        }
    }
}

fn live_state_lines(requested_backend: Option<BackendKind>) -> Vec<String> {
    let backend = match detect_backend(requested_backend) {
        Ok(backend) => backend,
        Err(err) => return vec![format!("backend unavailable: {err}")],
    };

    match backend.capture_all_workspaces() {
        Ok(workspaces) => summarize_workspaces(&workspaces),
        Err(err) => vec![format!("{} live capture failed: {err}", backend.kind())],
    }
}

fn summarize_workspaces(workspaces: &[WorkspaceCapture]) -> Vec<String> {
    let mut lines = vec![];
    for workspace in workspaces {
        lines.push(format!(
            "workspace {} [{}]",
            workspace.workspace.label_or_name(),
            count_label(workspace.tabs.len(), "tab")
        ));
        for tab in &workspace.tabs {
            lines.push(format!(
                "  tab {} [{}]",
                tab.tab.label_or_name(),
                count_label(tab.panes.len(), "pane")
            ));
        }
    }
    lines
}

fn saved_template_lines(store: &Store) -> Vec<String> {
    let mut lines = vec![];
    for kind in [
        ItemKind::Workspace,
        ItemKind::Tab,
        ItemKind::Pane,
        ItemKind::Stack,
        ItemKind::Snapshot,
    ] {
        match store.list(kind) {
            Ok(items) => {
                lines.push(format!(
                    "{} [{}]",
                    kind.dir_name(),
                    count_label(items.len(), kind.singular_name())
                ));
                for item in items {
                    lines.push(format!("  {item}"));
                }
            }
            Err(err) => lines.push(format!("{} unavailable: {err}", kind.dir_name())),
        }
    }
    lines
}

fn lines_or_empty<'a>(lines: &'a [String], empty: &'a str) -> Vec<ListItem<'a>> {
    if lines.is_empty() {
        vec![ListItem::new(empty)]
    } else {
        lines
            .iter()
            .map(|line| ListItem::new(line.as_str()))
            .collect()
    }
}

fn count_label(count: usize, singular: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {singular}s")
    }
}
