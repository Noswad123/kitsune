use std::sync::{atomic::AtomicBool, Arc};
use std::time::Instant;

use super::{App, AppState};

pub(crate) fn legacy_control_snapshot(app: &App) -> String {
    let mut terminals: Vec<_> = app
        .state
        .terminals
        .iter()
        .map(|(id, t)| (id.to_string(), t.test_legacy_control_snapshot()))
        .collect();
    terminals.sort();
    let workspaces: Vec<_> = app
        .state
        .workspaces
        .iter()
        .map(|ws| {
            let tabs: Vec<_> = ws
                .tabs
                .iter()
                .map(|tab| {
                    let mut seen: Vec<_> = tab.panes.iter().map(|(id, p)| (*id, p.seen)).collect();
                    seen.sort_by_key(|(id, _)| id.raw());
                    format!(
                        "{:?} {:?} {seen:?}",
                        tab.layout.pane_ids(),
                        tab.layout.focused()
                    )
                })
                .collect();
            (ws.active_tab, tabs)
        })
        .collect();
    format!(
        "{terminals:?} {workspaces:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {} {}",
        app.state.active,
        app.state.selected,
        app.state.mode,
        app.state.previous_pane_focus,
        app.state.agent_view_override,
        app.session_save_deadline,
        app.state.agent_panel_scroll,
        app.state.mobile_switcher_scroll,
        app.state.session_dirty
    )
}

pub(crate) fn legacy_control_fixture() -> (App, String, crate::terminal::TerminalId) {
    use crate::{
        agent_resume::{AgentSessionRef, PersistedAgentSession},
        detect::{Agent, AgentState},
    };
    let mut state = AppState::test_with_adversarial_identity_state();
    state
        .workspaces
        .push(crate::workspace::Workspace::test_adversarial_identity_state());
    state.ensure_test_terminals();
    state.active = Some(1);
    state.selected = 1;
    state.external_agent_management = true;
    let now = Instant::now() - std::time::Duration::from_secs(60);
    for terminal in state.terminals.values_mut() {
        let session = PersistedAgentSession {
            source: "kitsune:djinn".into(),
            agent: "djinn".into(),
            session_ref: AgentSessionRef::id("held-history").unwrap(),
        };
        terminal.set_persisted_agent_session(session.clone());
        terminal.pending_agent_resume_plan =
            crate::agent_resume::plan(&session.source, &session.agent, &session.session_ref);
        terminal.set_manual_label("manual-marker".into());
        terminal.launch_argv = Some(vec!["launch-marker".into()]);
        terminal.respawn_shell_on_exit = true;
        terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
        terminal.hook_authority = Some(crate::terminal::state::HookAuthority {
            source: "custom:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Idle,
            message: Some("held-authority".into()),
            reported_at: Instant::now(),
            session_ref: Some(session.session_ref.clone()),
        });
        terminal.set_agent_name("ambiguous".into());
        assert!(terminal.persisted_agent_session.is_some());
        assert!(terminal.pending_agent_resume_plan.is_some());
    }
    for ws in &mut state.workspaces {
        for tab in &mut ws.tabs {
            for pane in tab.panes.values_mut() {
                pane.seen = false;
            }
        }
    }
    let tab = state.workspaces[0].tabs.len() - 1;
    assert!(state.workspaces[0].tabs[tab].panes.len() > 1);
    let pane = state.workspaces[0].tabs[tab].root_pane;
    state.previous_pane_focus = Some(super::state::PaneFocusTarget {
        workspace_id: state.workspaces[0].id.clone(),
        pane_id: state.workspaces[0].tabs[0].root_pane,
    });
    let terminal_id = state.terminal_id_for_pane(0, pane).unwrap();
    state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .begin_managed_agent(
            "pending-marker".into(),
            Agent::Pi,
            now,
            std::time::Duration::from_secs(3),
            std::time::Duration::from_secs(10),
        );
    let mut app = app_from_state(state);
    app.no_session = false;
    let target = app.public_pane_id(0, pane).unwrap();
    (app, target, terminal_id)
}

// Avoid App::new: these tests need no application config loading or runtimes.
pub(crate) fn app_from_state(state: AppState) -> App {
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
    let (_, api_rx) = tokio::sync::mpsc::unbounded_channel();
    let now = Instant::now();
    App {
        state,
        terminal_runtimes: crate::terminal::TerminalRuntimeRegistry::new(),
        event_tx,
        event_rx,
        api_rx,
        event_hub: crate::api::EventHub::default(),
        last_focus: None,
        no_session: true,
        input_rx: None,
        last_terminal_size: None,
        config_diagnostic_deadline: None,
        toast_deadline: None,
        copy_feedback_deadline: None,
        last_api_notification_at: None,
        last_git_remote_status_refresh: now,
        last_git_repo_discovery_refresh: now,
        git_refresh_in_flight: false,
        git_refresh_due_after_in_flight: false,
        git_identity_refresh_requested: false,
        git_status_cache: Default::default(),
        pending_api_worktree_creates: Default::default(),
        pending_api_worktree_removes: Default::default(),
        pending_api_worktree_remove_paths: Default::default(),
        next_api_worktree_operation_id: 1,
        last_sidebar_divider_click: None,
        last_pane_click: None,
        pending_url_click_sources: Default::default(),
        next_resize_poll: now,
        next_animation_tick: None,
        next_agent_manifest_update_check: None,
        update_manifest_check_enabled: false,
        loaded_host_cursor: Default::default(),
        agent_metadata_deadline: None,
        pending_agent_resume_deadline: None,
        selection_autoscroll_deadline: None,
        selection_highlight_clear_deadline: None,
        session_save_deadline: None,
        session_save_thread: None,
        detached_custom_command_children: Vec::new(),
        persist_pane_history: false,
        last_render_at: None,
        pressed_terminal_keys: Default::default(),
        suppressed_repeat_keys: Default::default(),
        render_notify: Arc::new(tokio::sync::Notify::new()),
        render_dirty: Arc::new(AtomicBool::new(false)),
        full_redraw_pending: false,
        overlay_panes: Default::default(),
        local_terminal_notifications: false,
        local_input_source_switch: false,
        config_reloaded_from_disk: false,
        prefix_input_source: Box::new(crate::platform::RealPrefixInputSource::default()),
    }
}
