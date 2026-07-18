mod backend;
mod cli;
mod model;
mod store;
mod tui;

use anyhow::Result;
use backend::detect_backend;
use clap::Parser;
use cli::{Cli, Command, KindArg};
use store::{ItemKind, Store};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = Store::new(cli.store.clone())?;

    match cli.command.unwrap_or(Command::Tui) {
        Command::Init => {
            store.ensure()?;
            println!("{}", store.root().display());
        }
        Command::Doctor => {
            let backend = detect_backend(cli.backend.map(Into::into))?;
            let report = backend.doctor()?;
            println!("backend: {}", report.backend);
            println!("detected: {}", report.detected);
            println!("detail: {}", report.detail);
            println!("features:");
            for (name, supported) in report.features {
                println!("  {name}: {}", if supported { "yes" } else { "no" });
            }
        }
        Command::Capture(args) => {
            let backend = detect_backend(cli.backend.map(Into::into))?;
            let workspace = backend.capture_current_workspace(args.name)?;
            let path = store.save_workspace(&workspace)?;
            println!(
                "captured workspace '{}' from {} -> {}",
                workspace.name,
                backend.kind(),
                path.display()
            );
        }
        Command::Restore(args) => {
            let workspace = store.load_workspace(&args.name)?;
            let backend_kind = cli.backend.map(Into::into).or(Some(workspace.backend));
            let backend = detect_backend(backend_kind)?;
            backend.restore_workspace(&workspace, args.dry_run, args.skip_commands)?;
        }
        Command::List(args) => list_items(&store, args.kind, args.json)?,
        Command::Show(args) => {
            let contents = store.show(args.kind.into(), &args.name)?;
            print!("{contents}");
        }
        Command::Nav(args) => {
            let backend = detect_backend(cli.backend.map(Into::into))?;
            backend.smart_nav(args.direction.into(), &args.key)?;
        }
        Command::Tui => {
            store.ensure()?;
            tui::run(&store)?;
        }
    }
    Ok(())
}

fn list_items(store: &Store, kind: KindArg, json: bool) -> Result<()> {
    store.ensure()?;
    let kinds: Vec<ItemKind> = match kind.item_kind() {
        Some(kind) => vec![kind],
        None => vec![
            ItemKind::Workspace,
            ItemKind::Tab,
            ItemKind::Pane,
            ItemKind::Stack,
            ItemKind::Snapshot,
        ],
    };

    if json {
        let mut map = serde_json::Map::new();
        for kind in kinds {
            map.insert(
                kind.dir_name().to_string(),
                serde_json::json!(store.list(kind)?),
            );
        }
        println!("{}", serde_json::to_string_pretty(&map)?);
        return Ok(());
    }

    for kind in kinds {
        let names = store.list(kind)?;
        println!("{}", title(kind.dir_name()));
        if names.is_empty() {
            println!("  (none)");
        } else {
            for name in names {
                println!("  {name}");
            }
        }
    }
    Ok(())
}

fn title(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
