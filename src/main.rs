mod backend;
mod cli;
mod fingerprint;
mod model;
mod store;
mod tui;
mod validate;

use anyhow::{Result, bail};
use backend::detect_backend;
use chrono::Utc;
use clap::Parser;
use cli::{
    AddCommand, ApplyCommand, CaptureScope, Cli, Command, KindArg, RestoreTarget, StackCommand,
    StoreCommand,
};
use model::{CaptureSnapshot, ComponentRef, SnapshotScope, StackTemplate};
use serde::Serialize;
use std::collections::HashSet;
use std::io::{self, Write};
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
            let reuse = !args.no_reuse;
            match scope {
                CaptureScope::All => {
                    let mut captures = backend.capture_all_workspaces()?;
                    for capture in &mut captures {
                        fingerprint::annotate_workspace_capture(capture);
                    }
                    maybe_save_capture_snapshot(
                        &store,
                        args.append_snapshot,
                        args.plan,
                        backend.kind(),
                        SnapshotScope::All,
                        "all",
                        &captures,
                    )?;
                    let mut files_written = 0usize;
                    let mut total_plan = CapturePlan::default();
                    for capture in &mut captures {
                        let plan = plan_workspace_capture(&store, capture, reuse)?;
                        total_plan.extend(plan.clone());
                        if args.plan {
                            print_capture_plan(&plan);
                        } else {
                            files_written +=
                                save_planned_workspace_capture(&store, capture, &plan)?;
                            println!(
                                "captured workspace '{}' from {}",
                                capture.workspace.name,
                                backend.kind()
                            );
                        }
                    }
                    if args.plan {
                        println!(
                            "plan: {} workspaces, {} new files, {} reused components",
                            captures.len(),
                            total_plan.new_files,
                            total_plan.reused_components()
                        );
                    } else {
                        println!(
                            "captured {} workspaces from {} -> {} files",
                            captures.len(),
                            backend.kind(),
                            files_written
                        );
                    }
                }
                CaptureScope::Workspace => {
                    let mut workspace = backend.capture_current_workspace(name)?;
                    fingerprint::annotate_workspace_capture(&mut workspace);
                    maybe_save_capture_snapshot(
                        &store,
                        args.append_snapshot,
                        args.plan,
                        backend.kind(),
                        SnapshotScope::Workspace,
                        &workspace.workspace.name,
                        &workspace,
                    )?;
                    let plan = plan_workspace_capture(&store, &mut workspace, reuse)?;
                    if args.plan {
                        print_capture_plan(&plan);
                    } else {
                        let files = save_planned_workspace_capture(&store, &workspace, &plan)?;
                        println!(
                            "captured workspace '{}' from {} -> {} files",
                            workspace.workspace.name,
                            backend.kind(),
                            files
                        );
                    }
                }
                CaptureScope::Tab => {
                    let mut tab = backend.capture_current_tab(name)?;
                    fingerprint::annotate_tab_capture(&mut tab);
                    maybe_save_capture_snapshot(
                        &store,
                        args.append_snapshot,
                        args.plan,
                        backend.kind(),
                        SnapshotScope::Tab,
                        &tab.tab.name,
                        &tab,
                    )?;
                    let mut plan = plan_tab_capture(&store, &mut tab, reuse)?;
                    plan.new_files = files_for_tab_capture(&tab, &plan);
                    if args.plan {
                        print_capture_plan(&plan);
                    } else {
                        let files = save_planned_tab_capture(&store, &tab, &plan)?;
                        println!(
                            "captured tab '{}' from {} -> {} files",
                            tab.tab.name,
                            backend.kind(),
                            files
                        );
                    }
                }
                CaptureScope::Pane => {
                    let mut pane = backend.capture_current_pane(name)?;
                    fingerprint::annotate_pane(&mut pane);
                    maybe_save_capture_snapshot(
                        &store,
                        args.append_snapshot,
                        args.plan,
                        backend.kind(),
                        SnapshotScope::Pane,
                        &pane.name,
                        &pane,
                    )?;
                    let mut plan = plan_pane_capture(&store, &pane)?;
                    plan.new_files = 1;
                    if args.plan {
                        print_capture_plan(&plan);
                    } else {
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
        }
        Command::Restore(args) => {
            let (target, name) = args.resolve()?;
            match target {
                RestoreTarget::Workspace => {
                    confirm_live_change(
                        args.confirm,
                        args.dry_run,
                        &format!("restore workspace '{name}'"),
                    )?;
                    restore_workspace_by_name(
                        &store,
                        cli.backend.map(Into::into),
                        &name,
                        args.dry_run,
                        args.force,
                    )?;
                }
                RestoreTarget::Stack => {
                    confirm_live_change(
                        args.confirm,
                        args.dry_run,
                        &format!("restore stack '{name}'"),
                    )?;
                    let stack = store.load_stack(&name)?;
                    for workspace in &stack.workspaces {
                        restore_workspace_by_name(
                            &store,
                            cli.backend.map(Into::into),
                            &workspace.name,
                            args.dry_run,
                            args.force,
                        )?;
                    }
                }
            }
        }
        Command::Apply(args) => match args.command {
            ApplyCommand::Tab(args) => {
                confirm_live_change(
                    args.confirm,
                    args.dry_run,
                    &format!("apply tab '{}'", args.name),
                )?;
                apply_tab_by_name(
                    &store,
                    cli.backend.map(Into::into),
                    &args.name,
                    args.to.as_deref(),
                    args.dry_run,
                    args.force,
                )?;
            }
            ApplyCommand::Workspace(args) => {
                confirm_live_change(
                    args.confirm,
                    args.dry_run,
                    &format!("apply workspace '{}'", args.name),
                )?;
                restore_workspace_by_name(
                    &store,
                    cli.backend.map(Into::into),
                    &args.name,
                    args.dry_run,
                    args.force,
                )?;
            }
            ApplyCommand::Stack(args) => {
                confirm_live_change(
                    args.confirm,
                    args.dry_run,
                    &format!("apply stack '{}'", args.name),
                )?;
                apply_stack_by_name(
                    &store,
                    cli.backend.map(Into::into),
                    &args.name,
                    args.dry_run,
                    args.force,
                )?;
            }
        },
        Command::Add(args) => match args.command {
            AddCommand::Tab(args) => {
                let live_workspace_selector = args.to.clone();
                let workspace_name = match args.to {
                    Some(workspace) => workspace,
                    None => current_workspace_template_name(cli.backend.map(Into::into))?,
                };
                add_tab_to_workspace(&store, &args.name, &workspace_name)?;
                if args.apply {
                    confirm_live_change(
                        args.confirm,
                        args.dry_run,
                        &format!("apply tab '{}'", args.name),
                    )?;
                    apply_tab_by_name(
                        &store,
                        cli.backend.map(Into::into),
                        &args.name,
                        live_workspace_selector.as_deref(),
                        args.dry_run,
                        args.force,
                    )?;
                }
            }
        },
        Command::List(args) => list_items(&store, args.kind, args.json)?,
        Command::Show(args) => {
            let contents = store.show(args.kind.into(), &args.name)?;
            print!("{contents}");
        }
        Command::Tree(args) => print_tree(&store, args.kind.into(), &args.name)?,
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
        Command::Stack(args) => match args.command {
            StackCommand::Create(args) => {
                if args.workspaces.is_empty() {
                    bail!("stack create requires at least one workspace");
                }
                for workspace in &args.workspaces {
                    store.load_workspace(workspace)?;
                }
                let stack = StackTemplate {
                    schema: "kitsune.stack.v1".into(),
                    name: store::slug(&args.name),
                    workspaces: args
                        .workspaces
                        .iter()
                        .map(|name| ComponentRef {
                            name: name.clone(),
                            fingerprint: store
                                .load_workspace(name)
                                .ok()
                                .and_then(|workspace| workspace.identity.map(|i| i.fingerprint)),
                        })
                        .collect(),
                };
                let path = store.save_stack(&stack)?;
                println!("created stack '{}' -> {}", stack.name, path.display());
            }
        },
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
            tui::run(&store, cli.backend.map(Into::into))?;
        }
    }
    Ok(())
}

fn restore_workspace_by_name(
    store: &Store,
    requested_backend: Option<model::BackendKind>,
    name: &str,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let workspace = store.load_workspace_capture(name)?;
    let backend_kind = requested_backend.or(Some(workspace.workspace.backend));
    let backend = detect_backend(backend_kind)?;
    backend.restore_workspace(&workspace, dry_run, force)
}

fn print_tree(store: &Store, kind: ItemKind, name: &str) -> Result<()> {
    match kind {
        ItemKind::Workspace => print_workspace_tree(store, name),
        ItemKind::Tab => print_tab_tree(store, name, ""),
        ItemKind::Pane => print_pane_tree(store, name, ""),
        ItemKind::Stack => print_stack_tree(store, name),
        ItemKind::Snapshot => {
            let contents = store.show(ItemKind::Snapshot, name)?;
            print!("{contents}");
            Ok(())
        }
    }
}

fn print_stack_tree(store: &Store, name: &str) -> Result<()> {
    let stack = store.load_stack(name)?;
    println!(
        "stack {} [{} workspaces]",
        stack.name,
        stack.workspaces.len()
    );
    for (idx, workspace) in stack.workspaces.iter().enumerate() {
        let last = idx + 1 == stack.workspaces.len();
        let connector = if last { "└─ " } else { "├─ " };
        if store.path(ItemKind::Workspace, &workspace.name).exists() {
            print_workspace_tree_node(store, &workspace.name, "", connector)?;
        } else {
            println!("{connector}workspace {} (missing)", workspace.name);
        }
    }
    Ok(())
}

fn print_workspace_tree(store: &Store, name: &str) -> Result<()> {
    print_workspace_tree_node(store, name, "", "")
}

fn print_workspace_tree_node(
    store: &Store,
    name: &str,
    prefix: &str,
    connector: &str,
) -> Result<()> {
    let workspace = store.load_workspace(name)?;
    println!(
        "{prefix}{connector}workspace {}{} [{}]{}",
        workspace.name,
        label_suffix(workspace.label.as_deref()),
        count_label(workspace.tabs.len(), "tab"),
        cwd_suffix(workspace.cwd.as_ref())
    );
    let child_prefix = child_prefix(prefix, connector);
    for (idx, tab) in workspace.tabs.iter().enumerate() {
        let last = idx + 1 == workspace.tabs.len();
        let connector = if last { "└─ " } else { "├─ " };
        if store.path(ItemKind::Tab, &tab.name).exists() {
            print_tab_tree_node(store, &tab.name, &child_prefix, connector)?;
        } else {
            println!("{child_prefix}{connector}tab {} (missing)", tab.name);
        }
    }
    Ok(())
}

fn print_tab_tree(store: &Store, name: &str, prefix: &str) -> Result<()> {
    print_tab_tree_node(store, name, prefix, "")
}

fn print_tab_tree_node(store: &Store, name: &str, prefix: &str, connector: &str) -> Result<()> {
    let tab = store.load_tab(name)?;
    let split_suffix = if tab.layout.splits.is_empty() {
        String::new()
    } else {
        format!(", {}", count_label(tab.layout.splits.len(), "split"))
    };
    println!(
        "{prefix}{connector}tab {}{} [{}{}]{}",
        tab.name,
        label_suffix(tab.label.as_deref()),
        count_label(tab.panes.len(), "pane"),
        split_suffix,
        cwd_suffix(tab.cwd.as_ref())
    );
    let child_prefix = child_prefix(prefix, connector);
    for (idx, pane) in tab.panes.iter().enumerate() {
        let last = idx + 1 == tab.panes.len();
        let connector = if last { "└─ " } else { "├─ " };
        if store.path(ItemKind::Pane, &pane.name).exists() {
            print_pane_tree_node(store, &pane.name, &child_prefix, connector)?;
        } else {
            println!("{child_prefix}{connector}pane {} (missing)", pane.name);
        }
    }
    Ok(())
}

fn print_pane_tree(store: &Store, name: &str, prefix: &str) -> Result<()> {
    print_pane_tree_node(store, name, prefix, "")
}

fn print_pane_tree_node(store: &Store, name: &str, prefix: &str, connector: &str) -> Result<()> {
    let pane = store.load_pane(name)?;
    println!(
        "{prefix}{connector}pane {}{}{}{}",
        pane.name,
        label_suffix(pane.label.as_deref()),
        cwd_suffix(pane.cwd.as_ref()),
        pane.agent
            .as_ref()
            .map(|agent| format!(" — agent: {agent}"))
            .unwrap_or_default()
    );
    Ok(())
}

fn child_prefix(prefix: &str, connector: &str) -> String {
    format!(
        "{}{}",
        prefix,
        if connector.is_empty() {
            ""
        } else if connector.starts_with('└') {
            "   "
        } else {
            "│  "
        }
    )
}

fn label_suffix(label: Option<&str>) -> String {
    label.map(|label| format!(" ({label})")).unwrap_or_default()
}

fn cwd_suffix(cwd: Option<&std::path::PathBuf>) -> String {
    cwd.map(|cwd| format!(" — {}", compact_path(cwd)))
        .unwrap_or_default()
}

fn compact_path(path: &std::path::Path) -> String {
    let display = path.display().to_string();
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::PathBuf::from(home).display().to_string();
        if let Some(rest) = display.strip_prefix(&home) {
            if rest.is_empty() {
                return "~".into();
            }
            if rest.starts_with('/') {
                return format!("~{rest}");
            }
        }
    }
    display
}

fn count_label(count: usize, singular: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {singular}s")
    }
}

fn confirm_live_change(confirm: bool, dry_run: bool, summary: &str) -> Result<()> {
    if !confirm || dry_run {
        return Ok(());
    }

    print!("Proceed with {summary}? [y/N] ");
    io::stdout().flush()?;
    let mut response = String::new();
    io::stdin().read_line(&mut response)?;
    match response.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => Ok(()),
        _ => bail!("aborted"),
    }
}

fn apply_stack_by_name(
    store: &Store,
    requested_backend: Option<model::BackendKind>,
    name: &str,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let stack = store.load_stack(name)?;
    for workspace in &stack.workspaces {
        restore_workspace_by_name(store, requested_backend, &workspace.name, dry_run, force)?;
    }
    Ok(())
}

fn apply_tab_by_name(
    store: &Store,
    requested_backend: Option<model::BackendKind>,
    name: &str,
    workspace: Option<&str>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let tab = store.load_tab_capture(name)?;
    let backend = detect_backend(requested_backend)?;
    backend.apply_tab(&tab, workspace, dry_run, force)
}

fn current_workspace_template_name(
    requested_backend: Option<model::BackendKind>,
) -> Result<String> {
    let backend = detect_backend(requested_backend)?;
    let workspace = backend.capture_current_workspace(None)?;
    Ok(workspace.workspace.name)
}

fn add_tab_to_workspace(store: &Store, tab_name: &str, workspace_name: &str) -> Result<()> {
    let mut workspace = store.load_workspace(workspace_name)?;
    if workspace
        .tabs
        .iter()
        .any(|reference| reference.name == tab_name)
    {
        println!(
            "workspace '{}' already references tab '{}'",
            workspace.name, tab_name
        );
        return Ok(());
    }

    let mut tab = store.load_tab_capture(tab_name)?;
    if tab.tab.identity.is_none() {
        fingerprint::annotate_tab_capture(&mut tab);
    }
    let fingerprint = tab
        .tab
        .identity
        .as_ref()
        .map(|identity| identity.fingerprint.clone());

    workspace.tabs.push(ComponentRef {
        name: tab_name.to_string(),
        fingerprint,
    });

    let tab_fingerprints = workspace_tab_fingerprints(store, &workspace)?;
    fingerprint::annotate_workspace_from_fingerprints(&mut workspace, tab_fingerprints);
    let path = store.save_workspace(&workspace)?;
    println!(
        "added tab '{}' to workspace '{}' -> {}",
        tab_name,
        workspace.name,
        path.display()
    );
    Ok(())
}

fn maybe_save_capture_snapshot<T: Serialize>(
    store: &Store,
    append_snapshot: bool,
    plan_only: bool,
    backend: model::BackendKind,
    scope: SnapshotScope,
    base_name: &str,
    payload: &T,
) -> Result<()> {
    if !append_snapshot {
        return Ok(());
    }

    let snapshot = capture_snapshot(backend, scope, base_name, payload)?;
    if plan_only {
        println!(
            "snapshot: would append {} -> snapshots/{}.yaml",
            scope.as_str(),
            snapshot.name
        );
        return Ok(());
    }

    let path = store.save_snapshot(&snapshot)?;
    println!(
        "snapshot: appended {} -> {}",
        scope.as_str(),
        path.display()
    );
    Ok(())
}

fn capture_snapshot<T: Serialize>(
    backend: model::BackendKind,
    scope: SnapshotScope,
    base_name: &str,
    payload: &T,
) -> Result<CaptureSnapshot> {
    let captured_at = Utc::now();
    let timestamp = format!(
        "{}{:09}Z",
        captured_at.format("%Y%m%dT%H%M%S"),
        captured_at.timestamp_subsec_nanos()
    );
    let name = format!(
        "{}-{}-{}",
        scope.as_str(),
        store::slug(base_name),
        timestamp
    );
    Ok(CaptureSnapshot {
        schema: "kitsune.snapshot.v1".into(),
        name,
        captured_at,
        backend,
        scope,
        payload: serde_yaml::to_value(payload)?,
    })
}

fn workspace_tab_fingerprints(
    store: &Store,
    workspace: &model::WorkspaceTemplate,
) -> Result<Vec<String>> {
    let mut fingerprints = vec![];
    for reference in &workspace.tabs {
        if let Some(fingerprint) = &reference.fingerprint {
            fingerprints.push(fingerprint.clone());
            continue;
        }
        let mut tab = store.load_tab_capture(&reference.name)?;
        if tab.tab.identity.is_none() {
            fingerprint::annotate_tab_capture(&mut tab);
        }
        if let Some(identity) = tab.tab.identity {
            fingerprints.push(identity.fingerprint);
        }
    }
    Ok(fingerprints)
}

#[derive(Debug, Clone, Default)]
struct CapturePlan {
    actions: Vec<String>,
    reused_tabs: HashSet<String>,
    reused_panes: HashSet<String>,
    new_files: usize,
}

impl CapturePlan {
    fn extend(&mut self, other: CapturePlan) {
        self.actions.extend(other.actions);
        self.reused_tabs.extend(other.reused_tabs);
        self.reused_panes.extend(other.reused_panes);
        self.new_files += other.new_files;
    }

    fn reused_components(&self) -> usize {
        self.reused_tabs.len() + self.reused_panes.len()
    }
}

fn plan_workspace_capture(
    store: &Store,
    workspace: &mut model::WorkspaceCapture,
    reuse: bool,
) -> Result<CapturePlan> {
    let mut plan = CapturePlan::default();
    plan.actions
        .push(format!("workspace {}", workspace.workspace.name));

    for tab in &mut workspace.tabs {
        let before_tab_name = tab.tab.name.clone();
        let tab_plan = plan_tab_capture(store, tab, reuse)?;
        plan.extend(tab_plan);

        if reuse {
            let Some(fingerprint) = tab
                .tab
                .identity
                .as_ref()
                .map(|identity| &identity.fingerprint)
            else {
                continue;
            };
            let matches = matching_tabs(store, fingerprint)?;
            if let Some(existing) = matches.first() {
                plan.actions.push(format!(
                    "  reuse tab {} -> {}",
                    tab.tab.label_or_name(),
                    existing
                ));
                tab.tab.name = existing.clone();
                plan.reused_tabs.insert(existing.clone());
            } else {
                plan.actions.push(format!("  save tab {}", before_tab_name));
            }
        } else {
            plan.actions.push(format!("  save tab {}", before_tab_name));
        }
    }

    fingerprint::annotate_workspace_capture(workspace);
    plan.actions
        .push(format!("  save workspace {}", workspace.workspace.name));
    plan.new_files += files_for_workspace_capture(workspace, &plan);
    Ok(plan)
}

fn plan_tab_capture(
    store: &Store,
    tab: &mut model::TabCapture,
    reuse: bool,
) -> Result<CapturePlan> {
    let mut plan = CapturePlan::default();
    plan.actions.push(format!("tab {}", tab.tab.name));

    for pane in &mut tab.panes {
        let pane_plan = plan_pane_capture(store, pane)?;
        plan.extend(pane_plan);

        if reuse {
            let Some(fingerprint) = pane.identity.as_ref().map(|identity| &identity.fingerprint)
            else {
                continue;
            };
            let matches = matching_panes(store, fingerprint)?;
            if let Some(existing) = matches.first() {
                plan.actions.push(format!(
                    "  reuse pane {} -> {}",
                    pane.label_or_name(),
                    existing
                ));
                pane.name = existing.clone();
                plan.reused_panes.insert(existing.clone());
            } else {
                plan.actions.push(format!("  save pane {}", pane.name));
            }
        } else {
            plan.actions.push(format!("  save pane {}", pane.name));
        }
    }

    fingerprint::annotate_tab_capture(tab);
    Ok(plan)
}

fn plan_pane_capture(store: &Store, pane: &model::PaneTemplate) -> Result<CapturePlan> {
    let mut plan = CapturePlan::default();
    let Some(fingerprint) = pane.identity.as_ref().map(|identity| &identity.fingerprint) else {
        plan.actions.push(format!("pane {}", pane.name));
        return Ok(plan);
    };
    let matches = matching_panes(store, fingerprint)?;
    if !matches.is_empty() {
        plan.actions.push(format!(
            "pane {} matches existing {}",
            pane.label_or_name(),
            matches.join(", ")
        ));
    }
    Ok(plan)
}

fn save_planned_workspace_capture(
    store: &Store,
    capture: &model::WorkspaceCapture,
    plan: &CapturePlan,
) -> Result<usize> {
    let mut count = 0usize;
    for tab in &capture.tabs {
        if plan.reused_tabs.contains(&tab.tab.name) {
            continue;
        }
        count += save_planned_tab_capture(store, tab, plan)?;
    }
    store.save_workspace(&capture.workspace)?;
    count += 1;
    Ok(count)
}

fn save_planned_tab_capture(
    store: &Store,
    capture: &model::TabCapture,
    plan: &CapturePlan,
) -> Result<usize> {
    let mut count = 0usize;
    for pane in &capture.panes {
        if plan.reused_panes.contains(&pane.name) {
            continue;
        }
        store.save_pane(pane)?;
        count += 1;
    }
    store.save_tab(&capture.tab)?;
    count += 1;
    Ok(count)
}

fn files_for_workspace_capture(capture: &model::WorkspaceCapture, plan: &CapturePlan) -> usize {
    let mut count = 1;
    for tab in &capture.tabs {
        if plan.reused_tabs.contains(&tab.tab.name) {
            continue;
        }
        count += 1;
        for pane in &tab.panes {
            if !plan.reused_panes.contains(&pane.name) {
                count += 1;
            }
        }
    }
    count
}

fn files_for_tab_capture(capture: &model::TabCapture, plan: &CapturePlan) -> usize {
    let mut count = 1;
    for pane in &capture.panes {
        if !plan.reused_panes.contains(&pane.name) {
            count += 1;
        }
    }
    count
}

fn print_capture_plan(plan: &CapturePlan) {
    for action in &plan.actions {
        println!("{action}");
    }
    println!(
        "summary: {} new files, {} reused tabs, {} reused panes",
        plan.new_files,
        plan.reused_tabs.len(),
        plan.reused_panes.len()
    );
}

fn matching_tabs(store: &Store, fingerprint: &str) -> Result<Vec<String>> {
    let mut matches = vec![];
    for name in store.list(ItemKind::Tab)? {
        let mut tab = store.load_tab_capture(&name)?;
        if tab.tab.identity.is_none() {
            fingerprint::annotate_tab_capture(&mut tab);
        }
        if tab
            .tab
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
