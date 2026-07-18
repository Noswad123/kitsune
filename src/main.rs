mod backend;
mod cli;
mod fingerprint;
mod model;
mod store;
mod tui;
mod validate;

use anyhow::Result;
use backend::detect_backend;
use clap::Parser;
use cli::{CaptureScope, Cli, Command, KindArg, StoreCommand};
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
            let (scope, name) = args.resolve()?;
            match scope {
                CaptureScope::Workspace => {
                    let mut workspace = backend.capture_current_workspace(name)?;
                    fingerprint::annotate_workspace(&mut workspace);
                    print_workspace_matches(&store, &workspace)?;
                    let path = store.save_workspace(&workspace)?;
                    println!(
                        "captured workspace '{}' from {} -> {}",
                        workspace.name,
                        backend.kind(),
                        path.display()
                    );
                }
                CaptureScope::Tab => {
                    let mut tab = backend.capture_current_tab(name)?;
                    fingerprint::annotate_tab(&mut tab);
                    print_tab_matches(&store, &tab)?;
                    let path = store.save_tab(&tab)?;
                    println!(
                        "captured tab '{}' from {} -> {}",
                        tab.name,
                        backend.kind(),
                        path.display()
                    );
                }
                CaptureScope::Pane => {
                    let mut pane = backend.capture_current_pane(name)?;
                    fingerprint::annotate_pane(&mut pane);
                    print_pane_matches(&store, &pane)?;
                    let path = store.save_pane(&pane)?;
                    println!(
                        "captured pane '{}' from {} -> {}",
                        pane.name,
                        backend.kind(),
                        path.display()
                    );
                }
            }
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
        Command::Validate(args) => {
            let report = validate::validate_store(&store)?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_validation_report(&report, args.strict);
            }
            if !report.passes(args.strict) {
                std::process::exit(1);
            }
        }
        Command::Nav(args) => {
            let backend = detect_backend(cli.backend.map(Into::into))?;
            backend.smart_nav(args.direction.into(), &args.key)?;
        }
        Command::Store(args) => match args.command {
            StoreCommand::Path(args) => {
                if args.real {
                    let real = std::fs::canonicalize(store.root())?;
                    println!("{}", real.display());
                } else {
                    println!("{}", store.root().display());
                }
            }
            StoreCommand::Doctor(args) => {
                let report = store.doctor();
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print_store_doctor(&report);
                }
            }
            StoreCommand::Init => {
                store.ensure()?;
                println!("initialized store: {}", store.root().display());
            }
        },
        Command::Tui => {
            store.ensure()?;
            tui::run(&store)?;
        }
    }
    Ok(())
}

fn print_workspace_matches(store: &Store, workspace: &model::WorkspaceTemplate) -> Result<()> {
    for tab in &workspace.tabs {
        print_tab_matches(store, tab)?;
        for pane in &tab.panes {
            print_pane_matches(store, pane)?;
        }
    }
    Ok(())
}

fn print_tab_matches(store: &Store, tab: &model::TabTemplate) -> Result<()> {
    let Some(fingerprint) = tab.identity.as_ref().map(|identity| &identity.fingerprint) else {
        return Ok(());
    };
    let matches = matching_tabs(store, fingerprint)?;
    if !matches.is_empty() {
        println!(
            "matched saved tab '{}' -> {}",
            tab.label_or_name(),
            matches.join(", ")
        );
    }
    Ok(())
}

fn print_pane_matches(store: &Store, pane: &model::PaneTemplate) -> Result<()> {
    let Some(fingerprint) = pane.identity.as_ref().map(|identity| &identity.fingerprint) else {
        return Ok(());
    };
    let matches = matching_panes(store, fingerprint)?;
    if !matches.is_empty() {
        println!(
            "matched saved pane '{}' -> {}",
            pane.label_or_name(),
            matches.join(", ")
        );
    }
    Ok(())
}

fn matching_tabs(store: &Store, fingerprint: &str) -> Result<Vec<String>> {
    let mut matches = vec![];
    for name in store.list(ItemKind::Tab)? {
        let mut tab = store.load_tab(&name)?;
        if tab.identity.is_none() {
            fingerprint::annotate_tab(&mut tab);
        }
        if tab
            .identity
            .as_ref()
            .map(|identity| identity.fingerprint.as_str())
            == Some(fingerprint)
        {
            matches.push(name);
        }
    }
    Ok(matches)
}

fn matching_panes(store: &Store, fingerprint: &str) -> Result<Vec<String>> {
    let mut matches = vec![];
    for name in store.list(ItemKind::Pane)? {
        let mut pane = store.load_pane(&name)?;
        if pane.identity.is_none() {
            fingerprint::annotate_pane(&mut pane);
        }
        if pane
            .identity
            .as_ref()
            .map(|identity| identity.fingerprint.as_str())
            == Some(fingerprint)
        {
            matches.push(name);
        }
    }
    Ok(matches)
}

fn print_store_doctor(report: &store::StoreDoctor) {
    println!("store: {}", report.root.display());
    match &report.real_root {
        Some(real) => println!("real:  {}", real.display()),
        None => println!("real:  (unresolved; store may not exist yet)"),
    }
    println!("exists: {}", yes_no(report.root_exists));
    println!("symlink: {}", yes_no(report.root_is_symlink));
    println!("directories:");
    for dir in &report.directories {
        println!(
            "  {:<10} {} {}",
            dir.name,
            if dir.exists { "ok" } else { "missing" },
            dir.path.display()
        );
    }
    println!("status: {}", if report.ok() { "ok" } else { "needs init" });
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn print_validation_report(report: &validate::ValidationReport, strict: bool) {
    println!("store: {}", report.store.display());
    if report.issues.is_empty() {
        println!("status: ok");
        return;
    }

    for issue in &report.issues {
        let severity = match issue.severity {
            validate::Severity::Error => "error",
            validate::Severity::Warning => "warning",
        };
        match &issue.path {
            Some(path) => println!(
                "{severity}: {}: {} ({})",
                issue.kind,
                issue.message,
                path.display()
            ),
            None => println!("{severity}: {}: {}", issue.kind, issue.message),
        }
    }

    println!(
        "status: {} ({} errors, {} warnings{})",
        if report.passes(strict) {
            "ok"
        } else {
            "failed"
        },
        report.error_count(),
        report.warning_count(),
        if strict { ", strict" } else { "" }
    );
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
