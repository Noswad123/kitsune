use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use ratatui::layout::Direction;
use tokio::sync::{mpsc, Notify};
use tracing::{error, warn};

use crate::events::AppEvent;
use crate::layout::{Node, PaneId, TileLayout};
use crate::pane::{PaneLaunchEnv, PaneState};
use crate::terminal::{TerminalId, TerminalRuntime, TerminalState};
use crate::workspace::Workspace;

use super::snapshot::{PaneAgentSessionSnapshot, TabHistorySnapshot, WorkspaceHistorySnapshot};
use super::{
    DirectionSnapshot, LayoutSnapshot, SessionHistorySnapshot, SessionSnapshot, TabSnapshot,
    WorkspaceSnapshot,
};

struct RestoreRuntimeContext<'a> {
    scrollback_limit_bytes: usize,
    shell_config: crate::pane::PaneShellConfig<'a>,
    events: mpsc::Sender<AppEvent>,
    render_notify: Arc<Notify>,
    render_dirty: Arc<AtomicBool>,
}

type RestoredSession = (
    Vec<Workspace>,
    HashMap<TerminalId, TerminalState>,
    HashMap<TerminalId, TerminalRuntime>,
);
type RestoredWorkspace = (
    Workspace,
    Vec<TerminalState>,
    HashMap<TerminalId, TerminalRuntime>,
);
type RestoredTab = (
    crate::workspace::Tab,
    Vec<TerminalState>,
    HashMap<TerminalId, TerminalRuntime>,
    HashMap<PaneId, u32>,
);
type RestoreFailures<T> = (T, usize);

/// Restore workspaces from a snapshot. Each pane gets a fresh shell in its saved cwd.
pub fn restore(
    snapshot: &SessionSnapshot,
    history: Option<&SessionHistorySnapshot>,
    rows: u16,
    cols: u16,
    scrollback_limit_bytes: usize,
    default_shell: &str,
    shell_mode: crate::config::ShellModeConfig,
    events: mpsc::Sender<AppEvent>,
    render_notify: Arc<Notify>,
    render_dirty: Arc<AtomicBool>,
) -> RestoredSession {
    let mut imported_panes = HashMap::new();
    restore_with_imports(
        snapshot,
        history,
        rows,
        cols,
        scrollback_limit_bytes,
        crate::pane::PaneShellConfig::new(default_shell, shell_mode),
        &mut imported_panes,
        events,
        render_notify,
        render_dirty,
    )
}

#[cfg(unix)]
pub fn restore_handoff(
    snapshot: &SessionSnapshot,
    scrollback_limit_bytes: usize,
    default_shell: &str,
    shell_mode: crate::config::ShellModeConfig,
    imports: &mut HashMap<u32, crate::handoff_runtime::ImportedHandoffRuntime>,
    events: mpsc::Sender<AppEvent>,
    render_notify: Arc<Notify>,
    render_dirty: Arc<AtomicBool>,
) -> std::io::Result<RestoredSession> {
    restore_with_imports_strict(
        snapshot,
        None,
        24,
        80,
        scrollback_limit_bytes,
        crate::pane::PaneShellConfig::new(default_shell, shell_mode),
        imports,
        events,
        render_notify,
        render_dirty,
    )
}

#[cfg(unix)]
pub fn handoff_pane_aliases(
    snapshot: &SessionSnapshot,
    workspaces: &[Workspace],
) -> HashMap<u32, PaneId> {
    let mut aliases = HashMap::new();
    for (ws_snap, workspace) in snapshot.workspaces.iter().zip(workspaces) {
        for (tab_snap, tab) in ws_snap.tabs.iter().zip(&workspace.tabs) {
            let old_ids = collect_snapshot_pane_ids(&tab_snap.layout);
            let new_ids = tab.layout.pane_ids();
            for (old_id, new_id) in old_ids.into_iter().zip(new_ids) {
                if old_id != new_id.raw() {
                    aliases.insert(old_id, new_id);
                }
            }
        }
    }
    aliases
}

#[cfg(unix)]
fn collect_snapshot_pane_ids(node: &LayoutSnapshot) -> Vec<u32> {
    let mut ids = Vec::new();
    collect_snapshot_ids_inner(node, &mut ids);
    ids
}

#[cfg(unix)]
fn collect_snapshot_ids_inner(node: &LayoutSnapshot, ids: &mut Vec<u32>) {
    match node {
        LayoutSnapshot::Pane(id) => ids.push(*id),
        LayoutSnapshot::Split { first, second, .. } => {
            collect_snapshot_ids_inner(first, ids);
            collect_snapshot_ids_inner(second, ids);
        }
    }
}

fn migrated_public_pane_numbers_by_old_raw(
    snap: &WorkspaceSnapshot,
    next_public_pane_number: &mut usize,
) -> HashMap<u32, usize> {
    let mut public_numbers = snap.public_pane_numbers.clone();
    for tab in &snap.tabs {
        let mut pane_ids = Vec::new();
        collect_layout_snapshot_pane_ids(&tab.layout, &mut pane_ids);
        for old_raw in pane_ids {
            public_numbers.entry(old_raw).or_insert_with(|| {
                let number = *next_public_pane_number;
                *next_public_pane_number += 1;
                number
            });
        }
    }
    public_numbers
}

fn collect_layout_snapshot_pane_ids(node: &LayoutSnapshot, ids: &mut Vec<u32>) {
    match node {
        LayoutSnapshot::Pane(id) => ids.push(*id),
        LayoutSnapshot::Split { first, second, .. } => {
            collect_layout_snapshot_pane_ids(first, ids);
            collect_layout_snapshot_pane_ids(second, ids);
        }
    }
}

#[cfg(unix)]
fn restore_with_imports_strict(
    snapshot: &SessionSnapshot,
    history: Option<&SessionHistorySnapshot>,
    rows: u16,
    cols: u16,
    scrollback_limit_bytes: usize,
    shell_config: crate::pane::PaneShellConfig<'_>,
    imported_panes: &mut HashMap<u32, crate::handoff_runtime::ImportedHandoffRuntime>,
    events: mpsc::Sender<AppEvent>,
    render_notify: Arc<Notify>,
    render_dirty: Arc<AtomicBool>,
) -> std::io::Result<RestoredSession> {
    let (restored, failed_imports) = restore_with_imports_and_failures(
        snapshot,
        history,
        rows,
        cols,
        scrollback_limit_bytes,
        shell_config,
        imported_panes,
        events,
        render_notify,
        render_dirty,
    );
    if failed_imports > 0 {
        return Err(std::io::Error::other(format!(
            "handoff failed to restore {failed_imports} imported pane runtime(s)"
        )));
    }
    if !imported_panes.is_empty() {
        return Err(std::io::Error::other(format!(
            "handoff import did not consume {} pane runtime(s)",
            imported_panes.len()
        )));
    }
    Ok(restored)
}

fn restore_with_imports(
    snapshot: &SessionSnapshot,
    history: Option<&SessionHistorySnapshot>,
    rows: u16,
    cols: u16,
    scrollback_limit_bytes: usize,
    shell_config: crate::pane::PaneShellConfig<'_>,
    imported_panes: &mut HashMap<u32, crate::handoff_runtime::ImportedHandoffRuntime>,
    events: mpsc::Sender<AppEvent>,
    render_notify: Arc<Notify>,
    render_dirty: Arc<AtomicBool>,
) -> RestoredSession {
    restore_with_imports_and_failures(
        snapshot,
        history,
        rows,
        cols,
        scrollback_limit_bytes,
        shell_config,
        imported_panes,
        events,
        render_notify,
        render_dirty,
    )
    .0
}

fn restore_with_imports_and_failures(
    snapshot: &SessionSnapshot,
    history: Option<&SessionHistorySnapshot>,
    rows: u16,
    cols: u16,
    scrollback_limit_bytes: usize,
    shell_config: crate::pane::PaneShellConfig<'_>,
    imported_panes: &mut HashMap<u32, crate::handoff_runtime::ImportedHandoffRuntime>,
    events: mpsc::Sender<AppEvent>,
    render_notify: Arc<Notify>,
    render_dirty: Arc<AtomicBool>,
) -> RestoreFailures<RestoredSession> {
    let mut workspaces = Vec::new();
    let mut terminals = HashMap::new();
    let mut terminal_runtimes = HashMap::new();
    let mut failed_imports = 0;
    for (idx, ws_snap) in snapshot.workspaces.iter().enumerate() {
        let runtime_context = RestoreRuntimeContext {
            scrollback_limit_bytes,
            shell_config,
            events: events.clone(),
            render_notify: render_notify.clone(),
            render_dirty: render_dirty.clone(),
        };
        let (restored, workspace_failed_imports) = restore_workspace(
            ws_snap,
            history.and_then(|history| history.workspaces.get(idx)),
            rows,
            cols,
            &runtime_context,
            imported_panes,
        );
        failed_imports += workspace_failed_imports;
        if let Some((workspace, restored_terminals, restored_runtimes)) = restored {
            for terminal in restored_terminals {
                terminals.insert(terminal.id.clone(), terminal);
            }
            terminal_runtimes.extend(restored_runtimes);
            workspaces.push(workspace);
        }
    }
    crate::workspace::reserve_workspace_ids(&workspaces);
    ((workspaces, terminals, terminal_runtimes), failed_imports)
}

fn restore_workspace(
    snap: &WorkspaceSnapshot,
    history: Option<&WorkspaceHistorySnapshot>,
    rows: u16,
    cols: u16,
    runtime_context: &RestoreRuntimeContext<'_>,
    imported_panes: &mut HashMap<u32, crate::handoff_runtime::ImportedHandoffRuntime>,
) -> RestoreFailures<Option<RestoredWorkspace>> {
    let mut tabs = Vec::new();
    let mut terminals = Vec::new();
    let mut terminal_runtimes = HashMap::new();
    let workspace_id = snap
        .id
        .clone()
        .unwrap_or_else(crate::workspace::generate_workspace_id);
    let mut next_public_pane_number = snap
        .public_pane_numbers
        .values()
        .copied()
        .max()
        .and_then(|max| max.checked_add(1))
        .unwrap_or(1)
        .max(snap.next_public_pane_number);
    let public_pane_numbers_by_old_raw =
        migrated_public_pane_numbers_by_old_raw(snap, &mut next_public_pane_number);
    let public_pane_ids_by_old_raw: HashMap<u32, String> = public_pane_numbers_by_old_raw
        .iter()
        .map(|(old_raw, public_number)| {
            (
                *old_raw,
                format!(
                    "{}:p{}",
                    workspace_id,
                    crate::workspace::encode_public_number(*public_number)
                ),
            )
        })
        .collect();
    let mut public_pane_numbers = HashMap::new();
    let mut next_public_tab_number = snap
        .public_tab_numbers
        .iter()
        .copied()
        .max()
        .and_then(|max| max.checked_add(1))
        .unwrap_or(1)
        .max(snap.next_public_tab_number);
    let mut failed_imports = 0;

    for (idx, tab_snap) in snap.tabs.iter().enumerate() {
        let tab_number = snap.public_tab_numbers.get(idx).copied().unwrap_or(idx + 1);
        let (restored_tab, tab_failed_imports) = restore_tab(
            tab_snap,
            history.and_then(|history| history.tabs.get(idx)),
            tab_number,
            &workspace_id,
            rows,
            cols,
            runtime_context,
            imported_panes,
            &public_pane_ids_by_old_raw,
        );
        failed_imports += tab_failed_imports;
        let Some((mut tab, restored_terminals, restored_runtimes, reverse_id_map)) = restored_tab
        else {
            continue;
        };
        if let Some(public_tab_number) = snap.public_tab_numbers.get(idx).copied() {
            tab.number = public_tab_number;
        }
        next_public_tab_number = next_public_tab_number.max(tab.number + 1);
        for pane_id in tab.layout.pane_ids() {
            let public_number = public_pane_numbers_by_old_raw
                .get(
                    &reverse_id_map
                        .get(&pane_id)
                        .copied()
                        .unwrap_or(pane_id.raw()),
                )
                .copied()
                .unwrap_or_else(|| {
                    let number = next_public_pane_number;
                    next_public_pane_number += 1;
                    number
                });
            public_pane_numbers.insert(pane_id, public_number);
            next_public_pane_number = next_public_pane_number.max(public_number + 1);
        }
        terminals.extend(restored_terminals);
        terminal_runtimes.extend(restored_runtimes);
        tabs.push(tab);
    }

    if tabs.is_empty() {
        return (None, failed_imports);
    }

    let worktree_space = restored_worktree_space_membership(snap.worktree_space.clone());
    let (cached_git_space, cached_auto_label, cached_git_status_key) =
        crate::workspace::discover_workspace_git_identity(&snap.identity_cwd);

    (
        Some(Workspace {
            id: workspace_id,
            custom_name: snap.custom_name.clone(),
            identity_cwd: snap.identity_cwd.clone(),
            cached_identity_cwd: snap.identity_cwd.clone(),
            cached_auto_label,
            cached_git_status_key,
            cached_git_branch: crate::workspace::git_branch(&snap.identity_cwd),
            cached_git_branch_identity: Some((snap.identity_cwd.clone(), cached_git_space.clone())),
            cached_git_ahead_behind: None,
            cached_git_space,
            worktree_space,
            metadata_tokens: crate::metadata_tokens::MetadataTokens::default(),
            metadata_token_sequences: HashMap::new(),
            public_pane_numbers,
            next_public_pane_number,
            next_public_tab_number,
            active_tab: snap.active_tab.min(tabs.len().saturating_sub(1)),
            tabs,
            #[cfg(test)]
            test_runtimes: HashMap::new(),
        })
        .map(|workspace| (workspace, terminals, terminal_runtimes)),
        failed_imports,
    )
}

fn restored_worktree_space_membership(
    space: Option<crate::workspace::WorktreeSpaceMembership>,
) -> Option<crate::workspace::WorktreeSpaceMembership> {
    space.filter(|space| {
        space.checkout_path.exists()
            && crate::workspace::git_space_metadata(&space.checkout_path)
                .is_some_and(|current| current.key == space.key)
    })
}

fn restore_tab(
    snap: &TabSnapshot,
    history: Option<&TabHistorySnapshot>,
    number: usize,
    workspace_id: &str,
    rows: u16,
    cols: u16,
    runtime_context: &RestoreRuntimeContext<'_>,
    imported_panes: &mut HashMap<u32, crate::handoff_runtime::ImportedHandoffRuntime>,
    public_pane_ids_by_old_raw: &HashMap<u32, String>,
) -> RestoreFailures<Option<RestoredTab>> {
    let (node, id_map) = restore_node_remapped(&snap.layout);
    let reverse_id_map: HashMap<PaneId, u32> = id_map
        .iter()
        .map(|(&old_id, &new_id)| (new_id, old_id))
        .collect();
    let pane_ids = collect_pane_ids(&node);

    let mut panes = HashMap::new();
    let mut terminals = Vec::new();
    let mut terminal_runtimes = HashMap::new();
    let mut failed_imports = 0;
    for id in &pane_ids {
        let old_id = reverse_id_map.get(id);
        let saved_pane = old_id.and_then(|old_id| snap.panes.get(old_id));
        let saved_cwd = saved_pane
            .map(|p| p.cwd.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| "/".into()));

        let cwd = if saved_cwd.exists() {
            saved_cwd
        } else {
            warn!(
                cwd = %saved_cwd.display(),
                "saved pane cwd does not exist, falling back to HOME"
            );
            let home = std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/"));
            if home.exists() {
                home
            } else {
                PathBuf::from("/")
            }
        };

        let saved_label = saved_pane.and_then(|p| p.label.clone());
        let saved_agent_name = saved_pane.and_then(|p| p.agent_name.clone());
        let saved_managed_agent = saved_pane
            .and_then(|pane| pane.managed_agent_kind.as_deref())
            .and_then(crate::detect::parse_canonical_agent_label);
        let saved_launch_argv = saved_pane.and_then(|p| p.launch_argv.clone());
        let saved_agent_session = saved_pane.and_then(|p| p.agent_session.as_ref());
        let saved_history =
            old_id.and_then(|old_id| history.and_then(|history| history.panes.get(old_id)));
        let restored_agent_session =
            saved_agent_session.and_then(persisted_agent_session_from_snapshot);
        let old_pane_id = reverse_id_map.get(id).copied();
        let public_pane_id = old_pane_id
            .and_then(|old_id| public_pane_ids_by_old_raw.get(&old_id))
            .map(String::as_str);
        let launch_env = public_pane_id
            .map(|pane_id| {
                PaneLaunchEnv::from_extra(Vec::new()).with_identity(
                    workspace_id.to_string(),
                    crate::workspace::public_tab_id_for_number(workspace_id, number),
                    pane_id.to_string(),
                )
            })
            .unwrap_or_default();
        let imported_runtime = old_pane_id.and_then(|old_id| imported_panes.remove(&old_id));
        let was_imported = imported_runtime.is_some();
        #[cfg(not(unix))]
        if imported_runtime.is_some() {
            failed_imports += 1;
            continue;
        }

        let runtime_result = {
            #[cfg(unix)]
            if let Some(imported) = imported_runtime {
                TerminalRuntime::from_handoff_fd(
                    crate::handoff_runtime::ImportedHandoffRuntime {
                        master_fd: imported.master_fd,
                        state: imported.state.with_pane_id(*id),
                    },
                    runtime_context.scrollback_limit_bytes,
                    crate::terminal_theme::TerminalTheme::default(),
                    runtime_context.events.clone(),
                    runtime_context.render_notify.clone(),
                    runtime_context.render_dirty.clone(),
                )
            } else {
                TerminalRuntime::spawn_with_initial_history(
                    *id,
                    rows,
                    cols,
                    cwd.clone(),
                    runtime_context.scrollback_limit_bytes,
                    crate::terminal_theme::TerminalTheme::default(),
                    runtime_context.shell_config,
                    &launch_env,
                    saved_history.map(|history| history.ansi.as_str()),
                    runtime_context.events.clone(),
                    runtime_context.render_notify.clone(),
                    runtime_context.render_dirty.clone(),
                )
            }

            #[cfg(not(unix))]
            {
                TerminalRuntime::spawn_with_initial_history(
                    *id,
                    rows,
                    cols,
                    cwd.clone(),
                    runtime_context.scrollback_limit_bytes,
                    crate::terminal_theme::TerminalTheme::default(),
                    runtime_context.shell_config,
                    &launch_env,
                    saved_history.map(|history| history.ansi.as_str()),
                    runtime_context.events.clone(),
                    runtime_context.render_notify.clone(),
                    runtime_context.render_dirty.clone(),
                )
            }
        };

        match runtime_result {
            Ok(runtime) => {
                let terminal_id = TerminalId::alloc();
                let mut terminal = TerminalState::new(terminal_id.clone(), cwd.clone());
                // Saved argv is launch history, not a cold-restore command.
                terminal.launch_argv = saved_launch_argv;
                terminal.respawn_shell_on_exit = was_imported
                    && saved_pane.is_some_and(|pane| pane.respawn_shell_on_exit == Some(true));
                if let Some(label) = saved_label {
                    terminal.set_manual_label(label);
                }
                if let Some(session) = restored_agent_session {
                    terminal.set_persisted_agent_session(session);
                }
                terminal.agent_name = saved_agent_name;
                terminal.historical_managed_agent = saved_managed_agent;
                panes.insert(*id, PaneState::new(terminal_id.clone()));
                terminal_runtimes.insert(terminal_id, runtime);
                terminals.push(terminal);
            }
            Err(e) => {
                if was_imported {
                    failed_imports += 1;
                    error!(
                        tab = ?snap.custom_name,
                        pane_id = id.raw(),
                        err = %e,
                        "failed to restore imported pane"
                    );
                }
                error!(
                    tab = ?snap.custom_name,
                    pane_id = id.raw(),
                    err = %e,
                    "failed to restore pane, skipping"
                );
            }
        }
    }

    if panes.is_empty() {
        warn!(
            tab = ?snap.custom_name,
            "no panes could be restored for tab, dropping it"
        );
        return (None, failed_imports);
    }

    let surviving: HashSet<PaneId> = panes.keys().copied().collect();
    let Some(node) = prune_restored_node(node, &surviving) else {
        warn!(
            tab = ?snap.custom_name,
            "restored tab lost all panes after pruning missing layout nodes"
        );
        return (None, failed_imports);
    };
    let pane_ids = collect_pane_ids(&node);
    let Some(focus) = resolve_restored_pane(snap.focused, &id_map, &surviving, &pane_ids) else {
        return (None, failed_imports);
    };
    let Some(root_pane) = resolve_restored_pane(snap.root_pane, &id_map, &surviving, &pane_ids)
    else {
        return (None, failed_imports);
    };
    let layout = TileLayout::from_saved(node, focus);

    (
        Some((
            crate::workspace::Tab {
                custom_name: snap.custom_name.clone(),
                number,
                root_pane,
                layout,
                panes,
                #[cfg(test)]
                runtimes: HashMap::new(),
                zoomed: snap.zoomed,
                events: runtime_context.events.clone(),
                render_notify: runtime_context.render_notify.clone(),
                render_dirty: runtime_context.render_dirty.clone(),
            },
            terminals,
            terminal_runtimes,
            reverse_id_map,
        )),
        failed_imports,
    )
}

fn persisted_agent_session_from_snapshot(
    session: &PaneAgentSessionSnapshot,
) -> Option<crate::agent_resume::PersistedAgentSession> {
    crate::agent_resume::session_ref_from_snapshot(
        &session.source,
        &session.agent,
        session.kind,
        &session.value,
    )
}

pub(super) fn prune_restored_node(node: Node, surviving: &HashSet<PaneId>) -> Option<Node> {
    match node {
        Node::Pane(id) => surviving.contains(&id).then_some(Node::Pane(id)),
        Node::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let first = prune_restored_node(*first, surviving);
            let second = prune_restored_node(*second, surviving);
            match (first, second) {
                (Some(first), Some(second)) => Some(Node::Split {
                    direction,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                (None, None) => None,
            }
        }
    }
}

pub(super) fn resolve_restored_pane(
    saved_old_id: Option<u32>,
    id_map: &HashMap<u32, PaneId>,
    surviving: &HashSet<PaneId>,
    pane_ids: &[PaneId],
) -> Option<PaneId> {
    saved_old_id
        .and_then(|old_id| id_map.get(&old_id).copied())
        .filter(|pane_id| surviving.contains(pane_id))
        .or_else(|| pane_ids.first().copied())
}

/// Restore a layout tree, remapping every pane ID to a fresh globally unique one.
/// Returns the new tree and a map of old_raw_id → new PaneId.
pub(super) fn restore_node_remapped(snap: &LayoutSnapshot) -> (Node, HashMap<u32, PaneId>) {
    let mut id_map = HashMap::new();
    let node = remap_inner(snap, &mut id_map);
    (node, id_map)
}

fn remap_inner(snap: &LayoutSnapshot, id_map: &mut HashMap<u32, PaneId>) -> Node {
    match snap {
        LayoutSnapshot::Pane(old_id) => {
            let new_id = PaneId::alloc();
            id_map.insert(*old_id, new_id);
            Node::Pane(new_id)
        }
        LayoutSnapshot::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let first_node = remap_inner(first, id_map);
            let second_node = remap_inner(second, id_map);
            let dir = match direction {
                DirectionSnapshot::Horizontal => Direction::Horizontal,
                DirectionSnapshot::Vertical => Direction::Vertical,
            };
            Node::Split {
                direction: dir,
                ratio: *ratio,
                first: Box::new(first_node),
                second: Box::new(second_node),
            }
        }
    }
}

pub(super) fn collect_pane_ids(node: &Node) -> Vec<PaneId> {
    let mut ids = Vec::new();
    collect_ids_inner(node, &mut ids);
    ids
}

fn collect_ids_inner(node: &Node, ids: &mut Vec<PaneId>) {
    match node {
        Node::Pane(id) => ids.push(*id),
        Node::Split { first, second, .. } => {
            collect_ids_inner(first, ids);
            collect_ids_inner(second, ids);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn handoff_preserves_only_explicit_live_respawn_policy_not_historical_argv() {
        use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
        for (saved_policy, live_policy, has_argv) in [
            (None, false, true),
            (Some(true), false, true),
            (None, true, true),
            (None, true, false),
        ] {
            let argv = has_argv.then(|| {
                vec![
                    "/never-execute-historical-provider".to_string(),
                    "--resume".into(),
                    "old-ref".into(),
                ]
            });
            let snapshot: SessionSnapshot = serde_json::from_value(serde_json::json!({
                "version":3,"active":0,"selected":0,"workspaces":[{
                    "id":"w1","identity_cwd":std::env::temp_dir(),"active_tab":0,
                    "tabs":[{"layout":{"Pane":7},"zoomed":false,"panes":{"7":{
                        "cwd":std::env::temp_dir(),"launch_argv":argv,"respawn_shell_on_exit":saved_policy,
                        "agent_session":{"source":"kitsune:codex","agent":"codex","kind":"id","value":"old-ref"}
                    }}}]
                }]
            })).unwrap();
            let (events, _event_rx) = mpsc::channel(64);
            let notify = Arc::new(Notify::new());
            let dirty = Arc::new(AtomicBool::new(false));
            let (workspaces, mut terminals, runtimes) = restore(
                &snapshot,
                None,
                24,
                80,
                4096,
                "/bin/sh",
                crate::config::ShellModeConfig::NonLogin,
                events.clone(),
                notify.clone(),
                dirty.clone(),
            );
            let mut runtimes: crate::terminal::TerminalRuntimeRegistry = runtimes.into();
            let pane = workspaces[0].tabs[0].root_pane;
            let id = workspaces[0].terminal_id(pane).unwrap().clone();
            assert!(
                !terminals[&id].respawn_shell_on_exit,
                "cold restore must ignore saved live policy"
            );
            terminals.get_mut(&id).unwrap().respawn_shell_on_exit = live_policy;
            let captured = crate::persist::capture(
                &workspaces,
                &terminals,
                &runtimes,
                Some(0),
                0,
                26,
                Default::default(),
                Default::default(),
            );
            let encoded = serde_json::to_vec(&captured).unwrap();
            let transferred: SessionSnapshot = serde_json::from_slice(&encoded).unwrap();
            let saved = &transferred.workspaces[0].tabs[0].panes[&pane.raw()];
            assert_eq!(saved.respawn_shell_on_exit, live_policy.then_some(true));
            let runtime = runtimes.get(&id).unwrap();
            runtime
                .pause_handoff_reader(std::time::Duration::from_secs(2))
                .unwrap();
            let fd = unsafe { OwnedFd::from_raw_fd(runtime.duplicate_handoff_fd().unwrap()) };
            let state = runtime.handoff_runtime_state(pane.raw());
            let mut imports = HashMap::from([(
                pane.raw(),
                crate::handoff_runtime::ImportedHandoffRuntime {
                    master_fd: fd.into_raw_fd(),
                    state,
                },
            )]);
            let (_, imported_terminals, mut imported_runtimes) = restore_handoff(
                &transferred,
                4096,
                "/bin/sh",
                crate::config::ShellModeConfig::NonLogin,
                &mut imports,
                events,
                notify,
                dirty,
            )
            .unwrap();
            assert!(imports.is_empty());
            for runtime in imported_runtimes.values_mut() {
                runtime.assume_handoff_ownership();
            }
            runtimes.remove(&id).unwrap().preserve_for_handoff();
            let terminal = imported_terminals.values().next().unwrap();
            assert_eq!(terminal.respawn_shell_on_exit, live_policy);
            assert_eq!(terminal.launch_argv, argv);
            assert_eq!(
                terminal
                    .persisted_agent_session
                    .as_ref()
                    .unwrap()
                    .session_ref
                    .value,
                "old-ref"
            );
            for runtime in imported_runtimes.into_values() {
                runtime.shutdown();
            }
        }
    }

    #[cfg(windows)]
    fn test_restore_shell() -> &'static str {
        "C:\\Windows\\System32\\whoami.exe"
    }

    #[cfg(not(windows))]
    fn test_restore_shell() -> &'static str {
        "/bin/sh"
    }

    #[test]
    fn capture_and_restore_node_round_trip() {
        let node = Node::Split {
            direction: Direction::Horizontal,
            ratio: 0.5,
            first: Box::new(Node::Pane(PaneId::from_raw(0))),
            second: Box::new(Node::Split {
                direction: Direction::Vertical,
                ratio: 0.3,
                first: Box::new(Node::Pane(PaneId::from_raw(1))),
                second: Box::new(Node::Pane(PaneId::from_raw(2))),
            }),
        };

        let snap = super::super::snapshot::capture_node(&node);
        let (restored, id_map) = restore_node_remapped(&snap);

        assert_eq!(id_map.len(), 3);
        let ids = collect_pane_ids(&restored);
        assert_eq!(ids.len(), 3);
        let unique: std::collections::HashSet<u32> = ids.iter().map(|id| id.raw()).collect();
        assert_eq!(unique.len(), 3);
    }

    #[test]
    fn prune_restored_node_collapses_missing_branch() {
        let keep = PaneId::from_raw(11);
        let missing = PaneId::from_raw(12);
        let node = Node::Split {
            direction: Direction::Horizontal,
            ratio: 0.5,
            first: Box::new(Node::Pane(keep)),
            second: Box::new(Node::Pane(missing)),
        };
        let surviving = std::collections::HashSet::from([keep]);

        let pruned = prune_restored_node(node, &surviving).expect("remaining pane should survive");

        assert!(matches!(pruned, Node::Pane(id) if id == keep));
    }

    #[test]
    fn resolve_restored_pane_prefers_surviving_saved_id_and_falls_back_to_first_remaining() {
        let first = PaneId::from_raw(21);
        let second = PaneId::from_raw(22);
        let id_map = HashMap::from([(0_u32, first), (1_u32, second)]);
        let surviving = std::collections::HashSet::from([first]);
        let pane_ids = vec![first];

        assert_eq!(
            resolve_restored_pane(Some(0), &id_map, &surviving, &pane_ids),
            Some(first)
        );
        assert_eq!(
            resolve_restored_pane(Some(1), &id_map, &surviving, &pane_ids),
            Some(first)
        );
    }

    #[test]
    fn restored_worktree_space_membership_drops_missing_checkout() {
        let missing =
            std::env::temp_dir().join(format!("kitsune-missing-worktree-{}", std::process::id()));
        let membership = crate::workspace::WorktreeSpaceMembership {
            key: "repo-key".into(),
            label: "kitsune".into(),
            repo_root: missing.join("repo"),
            checkout_path: missing.join("checkout"),
            is_linked_worktree: true,
        };

        assert_eq!(restored_worktree_space_membership(Some(membership)), None);
    }

    #[test]
    fn restore_rehydrates_agent_session_metadata() {
        let session = super::super::snapshot::PaneAgentSessionSnapshot {
            source: "kitsune:hermes".into(),
            agent: "hermes".into(),
            kind: crate::agent_resume::AgentSessionRefKind::Id,
            value: "hermes-session".into(),
        };

        let preserved = persisted_agent_session_from_snapshot(&session)
            .expect("restore should preserve metadata");
        assert_eq!(preserved.source, "kitsune:hermes");
        assert_eq!(preserved.agent, "hermes");
        assert_eq!(preserved.session_ref.value, "hermes-session");
    }

    #[tokio::test]
    async fn restore_carries_persisted_agent_session_metadata() {
        let cwd = std::env::current_dir().unwrap();
        let snapshot = SessionSnapshot {
            version: super::super::snapshot::SNAPSHOT_VERSION,
            workspaces: vec![WorkspaceSnapshot {
                id: Some("workspace".into()),
                custom_name: None,
                identity_cwd: cwd.clone(),
                worktree_space: None,
                public_pane_numbers: HashMap::new(),
                next_public_pane_number: 0,
                public_tab_numbers: Vec::new(),
                next_public_tab_number: 0,
                tabs: vec![TabSnapshot {
                    custom_name: None,
                    layout: LayoutSnapshot::Pane(0),
                    panes: HashMap::from([(
                        0,
                        super::super::snapshot::PaneSnapshot {
                            cwd,
                            label: Some("reviewer".into()),
                            agent_name: Some("reviewer".into()),
                            managed_agent_kind: Some("opencode".into()),
                            agent_session: Some(super::super::snapshot::PaneAgentSessionSnapshot {
                                source: "kitsune:opencode".into(),
                                agent: "opencode".into(),
                                kind: crate::agent_resume::AgentSessionRefKind::Id,
                                value: "opencode-session".into(),
                            }),
                            launch_argv: None,
                            respawn_shell_on_exit: None,
                        },
                    )]),
                    zoomed: false,
                    focused: Some(0),
                    root_pane: Some(0),
                }],
                active_tab: 0,
            }],
            active: Some(0),
            selected: 0,
            sidebar_width: None,
            collapsed_sidebar_sections: Default::default(),
            collapsed_space_keys: Default::default(),
        };
        let (events, _event_rx) = mpsc::channel(4);

        let (_workspaces, terminals, _runtimes) = restore(
            &snapshot,
            None,
            24,
            80,
            0,
            test_restore_shell(),
            crate::config::ShellModeConfig::NonLogin,
            events,
            Arc::new(Notify::new()),
            Arc::new(AtomicBool::new(false)),
        );

        let terminal = terminals
            .values()
            .next()
            .expect("restored terminal should exist");
        assert!(
            !terminal.respawn_shell_on_exit,
            "agent sessions should not use native restore lifecycle when resume_agents_on_restore is disabled"
        );
        assert_eq!(terminal.agent_name.as_deref(), Some("reviewer"));
        assert!(!terminal.is_agent_terminal());
        assert_eq!(terminal.manual_label.as_deref(), Some("reviewer"));
        let session = terminal
            .persisted_agent_session
            .as_ref()
            .expect("persisted agent session should survive restore");
        assert_eq!(session.source, "kitsune:opencode");
        assert_eq!(session.agent, "opencode");
        assert_eq!(session.session_ref.value, "opencode-session");
    }

    #[tokio::test]
    async fn restore_preserves_public_id_mapping_after_pane_id_remap() {
        let cwd = std::env::current_dir().unwrap();
        let snapshot = SessionSnapshot {
            version: super::super::snapshot::SNAPSHOT_VERSION,
            workspaces: vec![WorkspaceSnapshot {
                id: Some("w1".into()),
                custom_name: None,
                identity_cwd: cwd.clone(),
                worktree_space: None,
                public_pane_numbers: HashMap::from([(10, 1), (20, 3)]),
                next_public_pane_number: 4,
                public_tab_numbers: vec![5],
                next_public_tab_number: 6,
                tabs: vec![TabSnapshot {
                    custom_name: None,
                    layout: LayoutSnapshot::Split {
                        direction: super::super::snapshot::DirectionSnapshot::Horizontal,
                        ratio: 0.5,
                        first: Box::new(LayoutSnapshot::Pane(10)),
                        second: Box::new(LayoutSnapshot::Pane(20)),
                    },
                    panes: HashMap::from([
                        (
                            10,
                            super::super::snapshot::PaneSnapshot {
                                cwd: cwd.clone(),
                                label: None,
                                agent_name: None,
                                managed_agent_kind: None,
                                agent_session: None,
                                launch_argv: None,
                                respawn_shell_on_exit: None,
                            },
                        ),
                        (
                            20,
                            super::super::snapshot::PaneSnapshot {
                                cwd: cwd.clone(),
                                label: None,
                                agent_name: None,
                                managed_agent_kind: None,
                                agent_session: None,
                                launch_argv: None,
                                respawn_shell_on_exit: None,
                            },
                        ),
                    ]),
                    zoomed: false,
                    focused: Some(10),
                    root_pane: Some(10),
                }],
                active_tab: 0,
            }],
            active: Some(0),
            selected: 0,
            sidebar_width: None,
            collapsed_sidebar_sections: Default::default(),
            collapsed_space_keys: Default::default(),
        };
        let (events, _event_rx) = mpsc::channel(4);

        let (workspaces, _terminals, _runtimes) = restore(
            &snapshot,
            None,
            24,
            80,
            0,
            test_restore_shell(),
            crate::config::ShellModeConfig::NonLogin,
            events,
            Arc::new(Notify::new()),
            Arc::new(AtomicBool::new(false)),
        );

        let workspace = workspaces.first().expect("workspace should restore");
        let mut public_numbers: Vec<_> = workspace.public_pane_numbers.values().copied().collect();
        public_numbers.sort_unstable();
        assert_eq!(public_numbers, vec![1, 3]);
        assert_eq!(workspace.next_public_pane_number, 4);
        assert_eq!(workspace.tabs[0].number, 5);
        assert_eq!(workspace.next_public_tab_number, 6);
    }

    #[tokio::test]
    async fn cold_restore_keeps_gapped_tab_numbers_and_agent_name_as_history_only() {
        let cwd = std::env::current_dir().unwrap();
        let pane_snap = |id: &str| {
            (
                id.parse::<u32>().unwrap(),
                super::super::snapshot::PaneSnapshot {
                    cwd: cwd.clone(),
                    label: None,
                    agent_name: None,
                    managed_agent_kind: None,
                    agent_session: None,
                    launch_argv: None,
                    respawn_shell_on_exit: None,
                },
            )
        };
        let final_pane = super::super::snapshot::PaneSnapshot {
            cwd: cwd.clone(),
            label: Some("planner".into()),
            agent_name: Some("planner".into()),
            managed_agent_kind: None,
            agent_session: Some(super::super::snapshot::PaneAgentSessionSnapshot {
                source: "kitsune:codex".into(),
                agent: "codex".into(),
                kind: crate::agent_resume::AgentSessionRefKind::Id,
                value: "codex-session".into(),
            }),
            launch_argv: None,
            respawn_shell_on_exit: None,
        };
        let snapshot = SessionSnapshot {
            version: super::super::snapshot::SNAPSHOT_VERSION,
            workspaces: vec![WorkspaceSnapshot {
                id: Some("w1".into()),
                custom_name: None,
                identity_cwd: cwd.clone(),
                worktree_space: None,
                public_pane_numbers: HashMap::from([(10, 1), (11, 2), (12, 3), (13, 4)]),
                next_public_pane_number: 5,
                public_tab_numbers: vec![1, 3, 4, 5],
                next_public_tab_number: 6,
                tabs: vec![
                    TabSnapshot {
                        custom_name: None,
                        layout: LayoutSnapshot::Pane(10),
                        panes: HashMap::from([pane_snap("10")]),
                        zoomed: false,
                        focused: Some(10),
                        root_pane: Some(10),
                    },
                    TabSnapshot {
                        custom_name: None,
                        layout: LayoutSnapshot::Pane(11),
                        panes: HashMap::from([pane_snap("11")]),
                        zoomed: false,
                        focused: Some(11),
                        root_pane: Some(11),
                    },
                    TabSnapshot {
                        custom_name: None,
                        layout: LayoutSnapshot::Pane(12),
                        panes: HashMap::from([pane_snap("12")]),
                        zoomed: false,
                        focused: Some(12),
                        root_pane: Some(12),
                    },
                    TabSnapshot {
                        custom_name: None,
                        layout: LayoutSnapshot::Pane(13),
                        panes: HashMap::from([(13, final_pane)]),
                        zoomed: false,
                        focused: Some(13),
                        root_pane: Some(13),
                    },
                ],
                active_tab: 3,
            }],
            active: Some(0),
            selected: 0,
            sidebar_width: None,
            collapsed_sidebar_sections: Default::default(),
            collapsed_space_keys: Default::default(),
        };
        let (events, _event_rx) = mpsc::channel(4);

        let (workspaces, terminals, _runtimes) = restore(
            &snapshot,
            None,
            24,
            80,
            0,
            test_restore_shell(),
            crate::config::ShellModeConfig::NonLogin,
            events,
            Arc::new(Notify::new()),
            Arc::new(AtomicBool::new(false)),
        );

        let workspace = workspaces.first().expect("workspace should restore");
        assert_eq!(workspace.active_tab, 3);
        assert_eq!(workspace.tabs[3].number, 5);
        let agent_pane = workspace.tabs[3].root_pane;
        let terminal_id = &workspace.tabs[3].panes[&agent_pane].attached_terminal_id;
        assert_eq!(
            terminals[terminal_id].agent_name.as_deref(),
            Some("planner")
        );
        assert!(!terminals[terminal_id].is_agent_terminal());
        assert_eq!(terminals[terminal_id].managed_agent_kind(), None);
        assert!(terminals[terminal_id].effective_agent_label().is_none());
    }

    #[test]
    fn legacy_restore_precomputes_missing_public_pane_numbers() {
        let cwd = std::env::current_dir().unwrap();
        let snapshot = WorkspaceSnapshot {
            id: Some("w1".into()),
            custom_name: None,
            identity_cwd: cwd,
            worktree_space: None,
            public_pane_numbers: HashMap::new(),
            next_public_pane_number: 0,
            public_tab_numbers: Vec::new(),
            next_public_tab_number: 0,
            tabs: vec![TabSnapshot {
                custom_name: None,
                layout: LayoutSnapshot::Split {
                    direction: super::super::snapshot::DirectionSnapshot::Horizontal,
                    ratio: 0.5,
                    first: Box::new(LayoutSnapshot::Pane(10)),
                    second: Box::new(LayoutSnapshot::Pane(20)),
                },
                panes: HashMap::new(),
                zoomed: false,
                focused: Some(10),
                root_pane: Some(10),
            }],
            active_tab: 0,
        };
        let mut next_public_pane_number = 1;

        let public_numbers =
            migrated_public_pane_numbers_by_old_raw(&snapshot, &mut next_public_pane_number);

        assert_eq!(public_numbers, HashMap::from([(10, 1), (20, 2)]));
        assert_eq!(next_public_pane_number, 3);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn historical_native_agent_restore_spawns_only_a_generic_shell() {
        let cwd = std::env::current_dir().unwrap();
        let saved_argv = vec![
            "historical-command-must-not-execute".to_string(),
            "--resume".into(),
            "codex-session".into(),
        ];
        let snapshot = SessionSnapshot {
            version: super::super::snapshot::SNAPSHOT_VERSION,
            workspaces: vec![WorkspaceSnapshot {
                id: Some("workspace".into()),
                custom_name: None,
                identity_cwd: cwd.clone(),
                worktree_space: None,
                public_pane_numbers: HashMap::new(),
                next_public_pane_number: 0,
                public_tab_numbers: Vec::new(),
                next_public_tab_number: 0,
                tabs: vec![TabSnapshot {
                    custom_name: None,
                    layout: LayoutSnapshot::Pane(0),
                    panes: HashMap::from([(
                        0,
                        super::super::snapshot::PaneSnapshot {
                            cwd,
                            label: None,
                            agent_name: None,
                            managed_agent_kind: None,
                            agent_session: Some(super::super::snapshot::PaneAgentSessionSnapshot {
                                source: "kitsune:codex".into(),
                                agent: "codex".into(),
                                kind: crate::agent_resume::AgentSessionRefKind::Id,
                                value: "codex-session".into(),
                            }),
                            launch_argv: Some(saved_argv.clone()),
                            respawn_shell_on_exit: None,
                        },
                    )]),
                    zoomed: false,
                    focused: Some(0),
                    root_pane: Some(0),
                }],
                active_tab: 0,
            }],
            active: Some(0),
            selected: 0,
            sidebar_width: None,
            collapsed_sidebar_sections: Default::default(),
            collapsed_space_keys: Default::default(),
        };
        let (events, _event_rx) = mpsc::channel(4);

        let (_workspaces, terminals, runtimes) = restore(
            &snapshot,
            None,
            24,
            80,
            0,
            test_restore_shell(),
            crate::config::ShellModeConfig::NonLogin,
            events,
            Arc::new(Notify::new()),
            Arc::new(AtomicBool::new(false)),
        );

        let terminal = terminals
            .values()
            .next()
            .expect("native agent restore should create terminal state");
        assert_eq!(terminal.launch_argv.as_ref(), Some(&saved_argv));
        assert!(
            terminal.pending_agent_resume_plan.is_none(),
            "historical native agent references must not create executable work"
        );
        assert!(
            !terminal.respawn_shell_on_exit,
            "deferred agent resume should not use native restore lifecycle before launch"
        );
        assert!(
            runtimes.len() == 1,
            "restore should create a generic shell runtime"
        );
        let mut imports = HashMap::new();
        let (_handoff_workspaces, handoff_terminals, handoff_runtimes) = restore_handoff(
            &snapshot,
            0,
            test_restore_shell(),
            crate::config::ShellModeConfig::NonLogin,
            &mut imports,
            mpsc::channel(4).0,
            Arc::new(Notify::new()),
            Arc::new(AtomicBool::new(false)),
        )
        .expect("handoff restore should preserve pending native agent resume");
        let handoff_terminal = handoff_terminals
            .values()
            .next()
            .expect("handoff restore should create terminal state");
        assert!(
            handoff_terminal.pending_agent_resume_plan.is_none(),
            "handoff without an imported PTY must not construct a native resume"
        );
        assert!(
            handoff_runtimes.len() == 1,
            "handoff without an imported PTY should restore a generic shell"
        );
    }

    #[tokio::test]
    async fn restore_seeds_saved_pane_history_into_runtime() {
        let (snapshot, history) = snapshot_with_saved_pane_history();
        let (events, _events_rx) = mpsc::channel(8);
        let render_notify = Arc::new(Notify::new());
        let render_dirty = Arc::new(AtomicBool::new(false));

        let (_workspaces, _terminals, runtimes) = restore(
            &snapshot,
            Some(&history),
            5,
            40,
            4096,
            test_restore_shell(),
            crate::config::ShellModeConfig::NonLogin,
            events,
            render_notify,
            render_dirty,
        );
        let runtime = runtimes
            .values()
            .next()
            .expect("restored runtime should exist");

        let restored_text = runtime.recent_unwrapped_text(10);
        assert!(
            restored_text.contains("RESTORED_HISTORY 👨‍👩‍👧 LINK"),
            "styled Unicode and hyperlink text should survive history replay"
        );

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while runtime.cwd().is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = runtime.try_send_bytes(bytes::Bytes::from_static(b"exit\n"));
    }

    #[tokio::test]
    async fn restore_without_history_snapshot_keeps_pane_contents_empty() {
        let (snapshot, _history) = snapshot_with_saved_pane_history();
        let (events, _events_rx) = mpsc::channel(8);
        let render_notify = Arc::new(Notify::new());
        let render_dirty = Arc::new(AtomicBool::new(false));

        let (_workspaces, _terminals, runtimes) = restore(
            &snapshot,
            None,
            5,
            40,
            4096,
            test_restore_shell(),
            crate::config::ShellModeConfig::NonLogin,
            events,
            render_notify,
            render_dirty,
        );
        let runtime = runtimes
            .values()
            .next()
            .expect("restored runtime should exist");

        assert!(
            !runtime
                .recent_unwrapped_text(10)
                .contains("RESTORED_HISTORY"),
            "pane history should not restore unless a history snapshot is supplied"
        );

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while runtime.cwd().is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = runtime.try_send_bytes(bytes::Bytes::from_static(b"exit\n"));
    }

    fn snapshot_with_saved_pane_history() -> (SessionSnapshot, SessionHistorySnapshot) {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut panes = HashMap::new();
        panes.insert(
            0,
            super::super::snapshot::PaneSnapshot {
                cwd: cwd.clone(),
                label: None,
                agent_name: None,
                managed_agent_kind: None,
                agent_session: None,
                launch_argv: None,
                respawn_shell_on_exit: None,
            },
        );
        let history = SessionHistorySnapshot {
            version: super::super::snapshot::SNAPSHOT_VERSION,
            workspaces: vec![WorkspaceHistorySnapshot {
                tabs: vec![super::super::snapshot::TabHistorySnapshot {
                    panes: HashMap::from([(
                        0,
                        super::super::snapshot::PaneHistorySnapshot {
                            ansi: concat!(
                                "\x1b[31mRESTORED_HISTORY 👨‍👩‍👧\x1b[0m ",
                                "\x1b]8;;https://example.com\x1b\\LINK\x1b]8;;\x1b\\\r\n"
                            )
                            .to_string(),
                            lines: 1,
                        },
                    )]),
                }],
            }],
        };
        let snapshot = SessionSnapshot {
            version: super::super::snapshot::SNAPSHOT_VERSION,
            workspaces: vec![WorkspaceSnapshot {
                id: Some("workspace".into()),
                custom_name: None,
                identity_cwd: cwd,
                worktree_space: None,
                public_pane_numbers: HashMap::new(),
                next_public_pane_number: 0,
                public_tab_numbers: Vec::new(),
                next_public_tab_number: 0,
                tabs: vec![TabSnapshot {
                    custom_name: None,
                    layout: LayoutSnapshot::Pane(0),
                    panes,
                    zoomed: false,
                    focused: Some(0),
                    root_pane: Some(0),
                }],
                active_tab: 0,
            }],
            active: Some(0),
            selected: 0,
            sidebar_width: Some(26),
            collapsed_sidebar_sections: Default::default(),
            collapsed_space_keys: Default::default(),
        };
        (snapshot, history)
    }
}
