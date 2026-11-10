use std::sync::{atomic::AtomicBool, Arc};
use std::time::Instant;

use crate::agent_resume::{AgentSessionRef, PersistedAgentSession};
use crate::api::schema::{
    PaneAgentState, PaneReleaseAgentParams, PaneReportAgentParams, PaneReportAgentSessionParams,
    ResponseResult, SuccessResponse,
};
use crate::app::{App, AppState};
use crate::detect::{Agent, AgentState};
use crate::terminal::TerminalId;

// Avoid App::new: these tests need no application config loading or runtimes.
fn fixture(agent: &str) -> (App, String, TerminalId, PersistedAgentSession) {
    let mut state = AppState::test_new();
    state.workspaces = vec![crate::workspace::Workspace::test_new("ownership")];
    state.ensure_test_terminals();
    state.active = Some(0);
    let pane_id = state.workspaces[0].tabs[0].root_pane;
    let terminal_id = state.workspaces[0].terminal_id(pane_id).unwrap().clone();
    let session = PersistedAgentSession {
        source: format!("kitsune:{agent}"),
        agent: agent.into(),
        session_ref: AgentSessionRef::id("restored-session").unwrap(),
    };
    let terminal = state.terminals.get_mut(&terminal_id).unwrap();
    terminal.set_persisted_agent_session(session.clone());
    terminal.pending_agent_resume_plan =
        crate::agent_resume::plan(&session.source, agent, &session.session_ref);
    assert!(terminal.pending_agent_resume_plan.is_some());
    state.session_dirty = false;
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
    let (_, api_rx) = tokio::sync::mpsc::unbounded_channel();
    let now = Instant::now();
    let app = App {
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
    };
    let public_pane_id = app.public_pane_id(0, pane_id).unwrap();
    (app, public_pane_id, terminal_id, session)
}

fn assert_ok(response: String) {
    let response: SuccessResponse = serde_json::from_str(&response).unwrap();
    assert_eq!(response.id, "ownership");
    assert_eq!(response.result, ResponseResult::Ok {});
}

#[test]
fn api_restored_session_release_checks_reference_before_consuming_sequence() {
    let (mut app, pane_id, terminal_id, session) = fixture("djinn");
    let pending = app.state.terminals[&terminal_id]
        .pending_agent_resume_plan
        .clone();
    assert!(pending.is_some());
    for (reference, cleared) in [("stale-session", false), ("restored-session", true)] {
        assert_ok(app.handle_pane_release_agent(
            "ownership".into(),
            PaneReleaseAgentParams {
                pane_id: pane_id.clone(),
                source: session.source.clone(),
                agent: session.agent.clone(),
                seq: Some(30),
                agent_session_id: Some(reference.into()),
                agent_session_path: None,
            },
        ));
        let terminal = &app.state.terminals[&terminal_id];
        assert_eq!(
            terminal.persisted_agent_session.as_ref(),
            (!cleared).then_some(&session)
        );
        assert!(terminal.hook_authority.is_none());
        assert_eq!(terminal.detected_agent, None);
        assert_eq!(terminal.state, AgentState::Unknown);
        // Releasing a restored reference does not cancel its queued resume plan.
        assert_eq!(terminal.pending_agent_resume_plan, pending);
        assert_eq!(app.state.session_dirty, cleared);
        assert_eq!(app.terminal_runtimes.len(), 0);
        app.state.assert_invariants_for_test();
    }
}

#[test]
fn api_invalid_report_reference_is_ignored_but_invalid_release_is_unscoped() {
    // A control character invalidates an ID; codex ignores path-only selectors.
    for (id, path) in [
        (Some("invalid\n"), None),
        (None, Some("/unused/session.jsonl")),
        (None, None),
    ] {
        let (mut app, pane_id, terminal_id, session) = fixture("codex");
        let pending = app.state.terminals[&terminal_id]
            .pending_agent_resume_plan
            .clone();
        let revision = app.state.terminals[&terminal_id].revision;
        assert_ok(app.handle_pane_report_agent(
            "ownership".into(),
            PaneReportAgentParams {
                pane_id: pane_id.clone(),
                source: session.source.clone(),
                agent: session.agent.clone(),
                state: PaneAgentState::Working,
                message: Some("must not become authority".into()),
                seq: Some(20),
                agent_session_id: id.map(str::to_string),
                agent_session_path: path.map(str::to_string),
            },
        ));
        assert_ok(app.handle_pane_report_agent_session(
            "ownership".into(),
            PaneReportAgentSessionParams {
                pane_id: pane_id.clone(),
                source: session.source.clone(),
                agent: session.agent.clone(),
                seq: Some(20),
                agent_session_id: id.map(str::to_string),
                agent_session_path: path.map(str::to_string),
                session_start_source: Some("startup".into()),
            },
        ));
        let terminal = &app.state.terminals[&terminal_id];
        assert_eq!(terminal.persisted_agent_session.as_ref(), Some(&session));
        assert!(terminal.hook_authority.is_none());
        assert_eq!(terminal.state, AgentState::Unknown);
        assert_eq!(terminal.revision, revision);
        assert!(!app.state.session_dirty);

        // Same seq proves neither ignored report consumed the source sequence.
        // Invalid selectors and intentionally absent selectors both release here.
        assert_ok(app.handle_pane_release_agent(
            "ownership".into(),
            PaneReleaseAgentParams {
                pane_id,
                source: session.source,
                agent: session.agent,
                seq: Some(20),
                agent_session_id: id.map(str::to_string),
                agent_session_path: path.map(str::to_string),
            },
        ));
        let terminal = &app.state.terminals[&terminal_id];
        assert!(terminal.persisted_agent_session.is_none());
        assert!(terminal.hook_authority.is_none());
        assert_eq!(terminal.detected_agent, None);
        assert_eq!(terminal.state, AgentState::Unknown);
        assert_eq!(terminal.pending_agent_resume_plan, pending);
        assert!(app.state.session_dirty);
        assert_eq!(app.terminal_runtimes.len(), 0);
        app.state.assert_invariants_for_test();
    }
}

#[test]
fn api_rejected_identity_update_consumes_sequence_before_foreground_confirmation() {
    let (mut app, pane_id, terminal_id, session) = fixture("droid");
    let pending = app.state.terminals[&terminal_id]
        .pending_agent_resume_plan
        .clone();
    let params = PaneReportAgentSessionParams {
        pane_id,
        source: "kitsune:claude".into(),
        agent: "claude".into(),
        seq: Some(20),
        agent_session_id: Some("claude-session".into()),
        agent_session_path: None,
        session_start_source: Some(" resume ".into()),
    };
    assert_ok(app.handle_pane_report_agent_session("ownership".into(), params.clone()));
    assert_eq!(
        app.state.terminals[&terminal_id]
            .persisted_agent_session
            .as_ref(),
        Some(&session)
    );
    assert!(!app.state.session_dirty);

    let pane_id = app.state.workspaces[0].tabs[0].root_pane;
    app.handle_internal_event(crate::events::AppEvent::StateChanged {
        pane_id,
        agent: Some(Agent::Claude),
        state: AgentState::Idle,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: Instant::now(),
    });
    app.state.session_dirty = false;
    for seq in [Some(19), Some(20), None, Some(21)] {
        let accepted = seq == Some(21);
        assert_ok(app.handle_pane_report_agent_session(
            "ownership".into(),
            PaneReportAgentSessionParams {
                seq,
                ..params.clone()
            },
        ));
        let terminal = &app.state.terminals[&terminal_id];
        let expected = PersistedAgentSession {
            source: params.source.clone(),
            agent: params.agent.clone(),
            session_ref: AgentSessionRef::id("claude-session").unwrap(),
        };
        assert_eq!(
            terminal.persisted_agent_session.as_ref(),
            Some(if accepted { &expected } else { &session })
        );
        assert!(terminal.hook_authority.is_none());
        assert_eq!(terminal.detected_agent, Some(Agent::Claude));
        assert_eq!(terminal.state, AgentState::Idle);
        assert_eq!(terminal.pending_agent_resume_plan, pending);
        assert_eq!(app.state.session_dirty, accepted);
        assert_eq!(app.terminal_runtimes.len(), 0);
        app.state.assert_invariants_for_test();
    }
}
