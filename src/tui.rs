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

pub fn run(store: &Store) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, store);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, store: &Store) -> Result<()> {
    let workspaces = store.list(ItemKind::Workspace)?;
    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(frame.area());

            let items: Vec<ListItem> = if workspaces.is_empty() {
                vec![ListItem::new("no workspaces captured yet")]
            } else {
                workspaces.iter().map(|name| ListItem::new(name.as_str())).collect()
            };
            let list = List::new(items).block(Block::default().title("Kitsune workspaces").borders(Borders::ALL));
            frame.render_widget(list, chunks[0]);

            let help = Paragraph::new(
                "q / Esc  quit\n\nCurrent TUI foundation:\n- browse saved workspaces\n- capture/restore/edit flows come next\n\nCLI now supports:\n  kit doctor\n  kit capture <name>\n  kit list\n  kit show workspace <name>\n  kit restore <name> --dry-run\n  kit nav left ctrl+h",
            )
            .block(Block::default().title("Help").borders(Borders::ALL))
            .style(Style::default().add_modifier(Modifier::DIM));
            frame.render_widget(help, chunks[1]);
        })?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    break;
                }
            }
        }
    }
    Ok(())
}
