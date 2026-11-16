use std::time::Instant;

use crate::agent_resume::{AgentSessionRef, PersistedAgentSession};
use crate::api::schema::{
    PaneAgentState, PaneReleaseAgentParams, PaneReportAgentParams, PaneReportAgentSessionParams,
    ResponseResult, SuccessResponse,
};
use crate::app::test_support::app_from_state;
use crate::app::{App, AppState};
use crate::detect::{Agent, AgentState};
use crate::terminal::evidence::{FallbackObservation, ForegroundObservation};
use crate::terminal::{TerminalId, TerminalRuntime};

fn external_legacy_controls_refuse(family: &str) {
    use crate::api::schema::*;
    use crate::app::test_support::{legacy_control_fixture, legacy_control_snapshot};
    for phase in ["named", "expired", "settled", "active"] {
        let (mut app, target, terminal_id) = legacy_control_fixture();
        if phase == "named" {
            app.state
                .terminals
                .get_mut(&terminal_id)
                .unwrap()
                .clear_agent_name();
            app.state
                .terminals
                .get_mut(&terminal_id)
                .unwrap()
                .set_agent_name("ready-marker".into());
        }
        if phase == "settled" {
            app.state
                .terminals
                .get_mut(&terminal_id)
                .unwrap()
                .begin_managed_agent(
                    "settled-marker".into(),
                    Agent::Pi,
                    Instant::now() - std::time::Duration::from_secs(5),
                    std::time::Duration::from_secs(3),
                    std::time::Duration::from_secs(30),
                );
        }
        if phase == "active" {
            app.state
                .terminals
                .get_mut(&terminal_id)
                .unwrap()
                .restore_managed_agent("active-marker".into(), Agent::Pi);
        }
        let (runtime, mut input) = TerminalRuntime::test_with_channel_capacity(80, 24, 4);
        runtime.test_process_pty_bytes(b"snapshot-marker\x1b[?2004h\x1b[?1004h");
        runtime
            .try_send_bytes(bytes::Bytes::from_static(b"SENTINEL"))
            .unwrap();
        app.terminal_runtimes.insert(terminal_id.clone(), runtime);
        app.state.agent_view_override = Some(AgentViewSetParams {
            source: "existing.view".into(),
            label: Some("held-view".into()),
            filter: None,
            sort: vec![],
        });
        app.state.agent_panel_scroll = 13;
        app.state.mobile_switcher_scroll = 17;
        for dirty in [false, true] {
            app.state.session_dirty = dirty;
            let before = legacy_control_snapshot(&app);
            let screen = app
                .terminal_runtimes
                .get(&terminal_id)
                .unwrap()
                .detection_text();
            for selector in [target.as_str(), "unknown-secret", "ambiguous"] {
                let mut methods = vec![
                    Method::AgentGet(AgentTarget {
                        target: selector.into(),
                    }),
                    Method::AgentFocus(AgentTarget {
                        target: selector.into(),
                    }),
                ];
                for text in ["", "prompt-secret"] {
                    methods.push(Method::AgentPrompt(AgentPromptParams {
                        target: selector.into(),
                        text: text.into(),
                        wait: None,
                    }));
                }
                for keys in [
                    vec!["enter".into()],
                    vec!["enter".into(), "bad-secret".into()],
                ] {
                    methods.push(Method::AgentSendKeys(AgentSendKeysParams {
                        target: selector.into(),
                        keys,
                    }));
                }
                for name in [
                    None,
                    Some("valid-name".into()),
                    Some("ambiguous".into()),
                    Some("Invalid Secret".into()),
                ] {
                    methods.push(Method::AgentRename(AgentRenameParams {
                        target: selector.into(),
                        name,
                    }));
                }
                for source in [
                    None,
                    Some("existing.view".into()),
                    Some("other.view".into()),
                    Some("invalid source".into()),
                ] {
                    methods.push(Method::AgentViewClear(AgentViewClearParams { source }));
                }
                for filter in [
                    None,
                    Some(AgentViewFilter::Any { filters: vec![] }),
                    Some(AgentViewFilter::Eq {
                        field: AgentViewField::Builtin(AgentViewBuiltinField::Status),
                        value: AgentViewValue::Bool(true),
                    }),
                ] {
                    methods.push(Method::AgentViewSet(AgentViewSetParams {
                        source: "replacement.view".into(),
                        label: None,
                        filter,
                        sort: vec![],
                    }));
                }
                for method in methods {
                    let selected = match &method {
                        Method::AgentGet(_) => "get",
                        Method::AgentFocus(_) => "focus",
                        Method::AgentPrompt(_) => "prompt",
                        Method::AgentSendKeys(_) => "keys",
                        Method::AgentRename(_) => "rename",
                        Method::AgentViewSet(_) => "view_set",
                        Method::AgentViewClear(_) => "view_clear",
                        _ => unreachable!(),
                    };
                    if selected != family {
                        continue;
                    }
                    let label = format!("{method:?}");
                    let response = app.handle_api_request(Request {
                        id: "control-id".into(),
                        method,
                    });
                    let response: ErrorResponse = serde_json::from_str(&response).expect(&label);
                    assert_eq!(response.id, "control-id");
                    assert_eq!(response.error.code, "external_agent_management", "{label}");
                    let guidance = match family {
                        "prompt" | "keys" => "use native provider commands or Coven-supported explicit transport for agent input",
                        "focus" => "use terminal.inspect and generation-checked terminal.focus for terminal navigation",
                        _ => "Coven owns agent identity, lifecycle reconciliation, status and views",
                    };
                    assert_eq!(response.error.message, format!("Legacy agent controls are disabled under external ownership; {guidance}"));
                    assert_eq!(legacy_control_snapshot(&app), before, "{label}");
                    assert_eq!(
                        app.terminal_runtimes
                            .get(&terminal_id)
                            .unwrap()
                            .detection_text(),
                        screen
                    );
                    assert_eq!(input.len(), 1);
                    assert!(app.session_save_thread.is_none());
                    assert!(app.event_hub.events_after(0).is_empty());
                    assert!(app.event_rx.try_recv().is_err());
                    app.state.assert_invariants_for_test();
                }
                if family == "get" {
                    app.reconcile_managed_agent_target(selector);
                    assert_eq!(legacy_control_snapshot(&app), before);
                }
                if family == "focus" {
                    let Err(error) = app.focus_agent_target(selector) else {
                        panic!("direct focus must refuse");
                    };
                    assert!(matches!(
                        error,
                        crate::app::agents::AgentFocusError::ExternalAgentManagement
                    ));
                    assert_eq!(
                        app.agent_focus_error_body(error).code,
                        "external_agent_management"
                    );
                    assert_eq!(legacy_control_snapshot(&app), before);
                }
                if family == "rename" {
                    for name in [None, Some("new-name".into())] {
                        let Err(error) = app.rename_agent_target(selector, name) else {
                            panic!("direct rename must refuse");
                        };
                        assert!(matches!(
                            error,
                            crate::app::agents::AgentRenameError::ExternalAgentManagement
                        ));
                        assert_eq!(
                            app.agent_rename_error_body(error).code,
                            "external_agent_management"
                        );
                        assert_eq!(legacy_control_snapshot(&app), before);
                    }
                }
            }
        }
        assert_eq!(input.try_recv().unwrap(), b"SENTINEL".as_slice());
        assert!(input.try_recv().is_err());
    }
}

#[tokio::test]
async fn external_legacy_controls_refuse_get() {
    external_legacy_controls_refuse("get");
}
#[tokio::test]
async fn external_legacy_controls_refuse_focus() {
    external_legacy_controls_refuse("focus");
}
#[tokio::test]
async fn external_legacy_controls_refuse_prompt() {
    external_legacy_controls_refuse("prompt");
}
#[tokio::test]
async fn external_legacy_controls_refuse_keys() {
    external_legacy_controls_refuse("keys");
}
#[tokio::test]
async fn external_legacy_controls_refuse_rename() {
    external_legacy_controls_refuse("rename");
}
#[tokio::test]
async fn external_legacy_controls_refuse_view_set() {
    external_legacy_controls_refuse("view_set");
}
#[tokio::test]
async fn external_legacy_controls_refuse_view_clear() {
    external_legacy_controls_refuse("view_clear");
}

#[test]
fn standalone_get_reconciles_the_expired_legacy_fixture() {
    let (mut app, target, terminal_id) = crate::app::test_support::legacy_control_fixture();
    assert!(app.state.terminals[&terminal_id].managed_agent_launch_pending());
    app.state.external_agent_management = false;
    app.reconcile_managed_agent_target(&target);
    assert!(!app.state.terminals[&terminal_id].managed_agent_launch_pending());
    assert!(app.state.session_dirty);
}

#[tokio::test]
async fn external_legacy_read_facades_remain_available_without_reconciliation() {
    use crate::api::schema::*;
    use crate::app::test_support::{legacy_control_fixture, legacy_control_snapshot};
    let (mut app, target, terminal_id) = legacy_control_fixture();
    app.terminal_runtimes.insert(
        terminal_id,
        TerminalRuntime::test_with_screen_bytes(80, 24, b"read-marker"),
    );
    let before = legacy_control_snapshot(&app);
    for method in [
        Method::AgentList(EmptyParams::default()),
        Method::AgentRead(AgentReadParams {
            target: target.clone(),
            source: ReadSource::Detection,
            lines: None,
            format: ReadFormat::Text,
            strip_ansi: true,
        }),
        Method::AgentExplain(AgentTarget { target }),
    ] {
        let response = app.handle_api_request(Request {
            id: "read-facade".into(),
            method,
        });
        let response: SuccessResponse = serde_json::from_str(&response).unwrap();
        assert_eq!(response.id, "read-facade");
        assert_eq!(legacy_control_snapshot(&app), before);
    }
}

// Avoid App::new: these tests need no application config loading or runtimes.
#[tokio::test]
async fn external_agent_management_rejects_agent_start_before_validation_without_side_effects() {
    use crate::api::schema::{AgentStartParams, ErrorResponse, Method, Request};
    use crate::app::agents::AgentStartError;
    use std::time::Duration;

    for with_runtime in [false, true] {
        let mut state = AppState::test_with_adversarial_identity_state();
        state.external_agent_management = true;
        let pane = state.workspaces[0].tabs[0].root_pane;
        let terminal_id = state.terminal_id_for_pane(0, pane).unwrap();
        let session = PersistedAgentSession {
            source: "kitsune:djinn".into(),
            agent: "djinn".into(),
            session_ref: AgentSessionRef::id("historical-only").unwrap(),
        };
        let terminal = state.terminals.get_mut(&terminal_id).unwrap();
        terminal.set_persisted_agent_session(session.clone());
        terminal.pending_agent_resume_plan =
            crate::agent_resume::plan(&session.source, &session.agent, &session.session_ref);
        assert!(terminal.pending_agent_resume_plan.is_some());
        terminal.set_manual_label("shell-history".into());
        terminal.launch_argv = Some(vec!["saved-launch-marker".into()]);
        terminal.respawn_shell_on_exit = true;
        let other = state
            .terminals
            .values_mut()
            .find(|terminal| terminal.id != terminal_id)
            .unwrap();
        other.begin_managed_agent(
            "taken".into(),
            Agent::Pi,
            Instant::now(),
            Duration::from_secs(3),
            Duration::from_secs(30),
        );
        let mut app = app_from_state(state);
        app.no_session = false;
        let pane_id = app.public_pane_id(0, pane).unwrap();
        let (runtime, mut input) = TerminalRuntime::test_with_channel_capacity(80, 24, 2);
        runtime
            .try_send_bytes(bytes::Bytes::from_static(b"existing-input"))
            .unwrap();
        if with_runtime {
            app.terminal_runtimes.insert(terminal_id.clone(), runtime);
        }
        let snapshot = |app: &App| {
            app.state
                .terminals
                .iter()
                .map(|(id, terminal)| {
                    (
                        id.clone(),
                        (
                            terminal.revision,
                            terminal.agent_name.clone(),
                            terminal.manual_label.clone(),
                            terminal.persisted_agent_session.clone(),
                            terminal.pending_agent_resume_plan.clone(),
                            terminal.managed_agent_kind(),
                            terminal.managed_agent_launch_pending(),
                            terminal.managed_agent_interactive_ready(),
                            terminal.next_managed_agent_deadline(),
                            terminal.state,
                            terminal.hook_authority.clone(),
                            terminal.launch_argv.clone(),
                        ),
                    )
                })
                .collect::<std::collections::HashMap<_, _>>()
        };
        let before = snapshot(&app);
        for dirty in [false, true] {
            app.state.session_dirty = dirty;
            for (name, kind, target, args, timeout_ms) in [
                ("worker", "pi", pane_id.as_str(), vec![], None),
                ("Invalid Name", "pi", pane_id.as_str(), vec![], None),
                ("worker", "unknown", pane_id.as_str(), vec![], None),
                ("worker", "pi", "w999:p999", vec![], None),
                ("worker", "pi", "malformed", vec![], None),
                ("taken", "pi", pane_id.as_str(), vec![], None),
                ("worker", "pi", pane_id.as_str(), vec!["bad\n".into()], None),
                ("worker", "pi", pane_id.as_str(), vec![], Some(1)),
            ] {
                for via_api in [false, true] {
                    let params = AgentStartParams {
                        name: name.into(),
                        kind: kind.into(),
                        pane_id: target.into(),
                        args: args.clone(),
                        timeout_ms,
                    };
                    let error = if via_api {
                        let response = app.handle_api_request(Request {
                            id: "external-start".into(),
                            method: Method::AgentStart(params),
                        });
                        let response: ErrorResponse = serde_json::from_str(&response).unwrap();
                        assert_eq!(response.id, "external-start");
                        response.error
                    } else {
                        let Err(error) = app.start_agent(params) else {
                            panic!("external ownership must reject direct start");
                        };
                        assert!(matches!(error, AgentStartError::ExternalAgentManagement));
                        app.agent_start_error_body(error)
                    };
                    assert_eq!(error.code, "external_agent_management");
                    assert!(error.message.contains("disabled under external ownership"));
                    assert!(error.message.contains("native provider"));
                    assert!(error.message.contains("Coven-supported explicit commands"));
                    assert_eq!(snapshot(&app), before);
                    assert!(app.state.terminals[&terminal_id].respawn_shell_on_exit);
                    assert_eq!(app.state.session_dirty, dirty);
                    assert!(app.session_save_deadline.is_none());
                    assert!(app.session_save_thread.is_none());
                    assert!(app.event_hub.events_after(0).is_empty());
                    assert!(app.event_rx.try_recv().is_err());
                    assert_eq!(input.len(), 1);
                    assert_eq!(app.terminal_runtimes.len(), usize::from(with_runtime));
                    assert!(app.state.external_agent_management);
                    app.state.assert_invariants_for_test();
                }
            }
        }
        assert_eq!(input.try_recv().unwrap(), b"existing-input".as_slice());
        assert!(input.try_recv().is_err());
    }
}

#[test]
fn external_agent_management_rejects_legacy_api_even_with_coven_source() {
    let mut state = AppState::test_with_adversarial_identity_state();
    state.external_agent_management = true;
    let mut app = app_from_state(state);
    let raw_pane = app.state.workspaces[0].tabs[0].root_pane;
    let pane_id = app.public_pane_id(0, raw_pane).unwrap();
    let terminal_id = app.state.workspaces[0]
        .terminal_id(raw_pane)
        .unwrap()
        .clone();
    let revision = app.state.terminals[&terminal_id].revision;
    for source in ["kitsune:djinn", "coven", "coven.agent-management.v1"] {
        let params = serde_json::json!({
            "pane_id": pane_id, "source": source, "agent": "djinn",
            "state": "working", "agent_session_id": "must-not-become-authority", "seq": 999
        });
        let responses = [
            app.handle_pane_report_agent(
                "external".into(),
                serde_json::from_value(params.clone()).unwrap(),
            ),
            app.handle_pane_report_agent_session(
                "external".into(),
                serde_json::from_value(params.clone()).unwrap(),
            ),
            app.handle_pane_release_agent(
                "external".into(),
                serde_json::from_value(params).unwrap(),
            ),
        ];
        for response in responses {
            let response: serde_json::Value = serde_json::from_str(&response).unwrap();
            assert_eq!(response["error"]["code"], "external_agent_management");
        }
        let terminal = &app.state.terminals[&terminal_id];
        assert_eq!(terminal.revision, revision);
        assert!(terminal.hook_authority.is_none());
        assert!(terminal.persisted_agent_session.is_none());
        assert!(terminal.detected_agent().is_none());
        app.state.assert_invariants_for_test();
    }
}

#[test]
fn external_agent_management_ignores_queued_evidence_without_runtime_or_fallback() {
    use crate::events::AppEvent;
    let mut state = AppState::test_with_adversarial_identity_state();
    state.external_agent_management = true;
    let pane_id = state.workspaces[0].tabs[0].root_pane;
    let terminal_id = state.workspaces[0].terminal_id(pane_id).unwrap().clone();
    let session = PersistedAgentSession {
        source: "kitsune:djinn".into(),
        agent: "djinn".into(),
        session_ref: AgentSessionRef::id("historical-only").unwrap(),
    };
    state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .set_persisted_agent_session(session.clone());
    state.session_dirty = false;
    let revision = state.terminals[&terminal_id].revision;
    let now = Instant::now();
    let mut app = app_from_state(state);
    for via_app in [false, true] {
        let events = [
            AppEvent::HookStateReported {
                pane_id,
                source: "coven".into(),
                agent_label: "djinn".into(),
                state: AgentState::Working,
                message: None,
                seq: Some(999),
                session_ref: Some(AgentSessionRef::id("new-session").unwrap()),
            },
            AppEvent::AgentSessionReported {
                pane_id,
                source: "coven".into(),
                agent_label: "djinn".into(),
                seq: Some(999),
                session_ref: Some(AgentSessionRef::id("new-session").unwrap()),
                session_start_source: None,
            },
            AppEvent::DetectorUpdate {
                pane_id,
                epoch: crate::terminal::evidence::DetectorEpoch::new(),
                foreground: Some(ForegroundObservation::Confirmed {
                    agent: Agent::Djinn,
                    process_group_id: None,
                }),
                fallback: Some(FallbackObservation {
                    agent: Some(Agent::Djinn),
                    state: AgentState::Working,
                    visible_blocker: false,
                    visible_working: true,
                }),
                observed_at: now,
            },
            AppEvent::StateChanged {
                pane_id,
                agent: Some(Agent::Djinn),
                state: AgentState::Blocked,
                visible_blocker: true,
                visible_working: false,
                process_exited: false,
                observed_at: now,
            },
            AppEvent::HookAuthorityCleared {
                pane_id,
                source: None,
                seq: Some(1000),
            },
            AppEvent::HookAgentReleased {
                pane_id,
                source: session.source.clone(),
                agent_label: session.agent.clone(),
                known_agent: Some(Agent::Djinn),
                seq: Some(1001),
                session_ref: Some(session.session_ref.clone()),
            },
        ];
        for event in events {
            if via_app {
                app.handle_internal_event(event);
            } else {
                assert!(app
                    .state
                    .handle_app_event_with_release_outcome(event)
                    .0
                    .is_empty());
            }
            let terminal = &app.state.terminals[&terminal_id];
            assert_eq!(terminal.revision, revision);
            assert_eq!(terminal.persisted_agent_session.as_ref(), Some(&session));
            assert!(terminal.hook_authority.is_none());
            assert!(terminal.detected_agent().is_none());
            assert!(terminal.foreground_agent_evidence().is_none());
            assert!(!app.state.session_dirty);
            app.state.assert_invariants_for_test();
        }
    }
    assert!(app
        .state
        .publish_pane_process_exit_if_agent(pane_id)
        .is_none());
    assert!(app.state.reconcile_managed_agents_at(now).is_empty());
    assert!(app.state.next_managed_agent_deadline().is_none());
    assert_eq!(app.terminal_runtimes.len(), 0);
    assert!(app.state.external_agent_management);
}

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
    let app = app_from_state(state);
    let public_pane_id = app.public_pane_id(0, pane_id).unwrap();
    (app, public_pane_id, terminal_id, session)
}

fn assert_ok(response: String) {
    let response: SuccessResponse = serde_json::from_str(&response).unwrap();
    assert_eq!(response.id, "ownership");
    assert_eq!(response.result, ResponseResult::Ok {});
}

fn observe_live_fixture(app: &mut App, agent: Agent) -> &TerminalRuntime {
    let pane_id = app.state.workspaces[0].tabs[0].root_pane;
    let terminal_id = app.state.workspaces[0]
        .terminal_id(pane_id)
        .unwrap()
        .clone();
    let runtime = TerminalRuntime::test_with_screen_bytes(80, 24, b"");
    assert_eq!(runtime.child_pid(), None);
    let epoch = runtime.detector_epoch();
    app.terminal_runtimes.insert(terminal_id.clone(), runtime);
    let observed_at = Instant::now();
    app.handle_internal_event(crate::events::AppEvent::DetectorUpdate {
        pane_id,
        epoch,
        foreground: Some(ForegroundObservation::Confirmed {
            agent,
            process_group_id: None,
        }),
        fallback: Some(FallbackObservation {
            agent: Some(agent),
            state: AgentState::Idle,
            visible_blocker: false,
            visible_working: false,
        }),
        observed_at,
    });
    assert_eq!(
        app.state.terminals[&terminal_id].foreground_agent_evidence(),
        Some((agent, observed_at))
    );
    let runtime = app.terminal_runtimes.get(&terminal_id).unwrap();
    assert_eq!(runtime.child_pid(), None);
    runtime
}

fn live_djinn_fixture() -> (App, String, TerminalId, PersistedAgentSession) {
    let (mut app, pane_id, terminal_id, session) = fixture("djinn");
    let pending = app.state.terminals[&terminal_id]
        .pending_agent_resume_plan
        .clone();
    observe_live_fixture(&mut app, Agent::Djinn);
    assert_ok(app.handle_pane_report_agent(
        "ownership".into(),
        PaneReportAgentParams {
            pane_id: pane_id.clone(),
            source: session.source.clone(),
            agent: session.agent.clone(),
            state: PaneAgentState::Working,
            message: Some("live authority".into()),
            seq: Some(20),
            agent_session_id: Some(session.session_ref.value.clone()),
            agent_session_path: None,
        },
    ));
    let terminal = &app.state.terminals[&terminal_id];
    let authority = terminal.hook_authority.as_ref().unwrap();
    assert_eq!(authority.source, session.source);
    assert_eq!(authority.agent_label, session.agent);
    assert_eq!(authority.session_ref.as_ref(), Some(&session.session_ref));
    assert_eq!(authority.state, AgentState::Working);
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(terminal.detected_agent(), Some(Agent::Djinn));
    assert_eq!(terminal.effective_agent_label(), Some("djinn"));
    assert!(terminal.persisted_agent_session.is_none());
    assert_eq!(terminal.pending_agent_resume_plan, pending);
    assert_eq!(app.terminal_runtimes.len(), 1);
    assert_eq!(
        app.terminal_runtimes.get(&terminal_id).unwrap().child_pid(),
        None
    );
    app.state.assert_invariants_for_test();
    app.state.session_dirty = false;
    (app, pane_id, terminal_id, session)
}

#[tokio::test]
async fn api_live_hook_release_checks_reference_and_sequence_before_clearing_authority() {
    let (mut app, pane_id, terminal_id, session) = live_djinn_fixture();
    let pending = app.state.terminals[&terminal_id]
        .pending_agent_resume_plan
        .clone();
    let authority = app.state.terminals[&terminal_id].hook_authority.clone();
    let revision = app.state.terminals[&terminal_id].revision;
    for (reference, seq, accepted) in [
        ("stale-session", Some(30), false),
        ("restored-session", Some(19), false),
        ("restored-session", Some(20), false),
        ("restored-session", None, false),
        ("restored-session", Some(30), true),
    ] {
        assert_ok(app.handle_pane_release_agent(
            "ownership".into(),
            PaneReleaseAgentParams {
                pane_id: pane_id.clone(),
                source: session.source.clone(),
                agent: session.agent.clone(),
                seq,
                agent_session_id: Some(reference.into()),
                agent_session_path: None,
            },
        ));
        let terminal = &app.state.terminals[&terminal_id];
        if accepted {
            assert!(terminal.hook_authority.is_none());
            assert_eq!(terminal.state, AgentState::Idle);
        } else {
            assert_eq!(terminal.hook_authority, authority);
            assert_eq!(terminal.state, AgentState::Working);
            assert_eq!(terminal.revision, revision);
        }
        assert!(terminal.persisted_agent_session.is_none());
        assert_eq!(terminal.detected_agent(), Some(Agent::Djinn));
        assert_eq!(terminal.effective_agent_label(), Some("djinn"));
        assert_eq!(terminal.pending_agent_resume_plan, pending);
        assert_eq!(app.state.session_dirty, accepted);
        assert_eq!(app.terminal_runtimes.len(), 1);
        assert_eq!(
            app.terminal_runtimes.get(&terminal_id).unwrap().child_pid(),
            None
        );
        app.state.assert_invariants_for_test();
    }
}

#[tokio::test]
async fn api_live_hook_release_rejects_foreign_source_or_agent_before_consuming_sequence() {
    for (source, agent) in [("custom:djinn", "djinn"), ("kitsune:djinn", "claude")] {
        let (mut app, pane_id, terminal_id, session) = live_djinn_fixture();
        let pending = app.state.terminals[&terminal_id]
            .pending_agent_resume_plan
            .clone();
        let authority = app.state.terminals[&terminal_id].hook_authority.clone();
        let revision = app.state.terminals[&terminal_id].revision;
        // Valid selector text becomes None for these mismatched source/agent pairs.
        assert!(AgentSessionRef::id("restored-session").is_some());
        assert!(crate::agent_resume::session_ref_from_report(
            source,
            agent,
            Some("restored-session".into()),
            None,
        )
        .is_none());
        for accepted in [false, true] {
            assert_ok(app.handle_pane_release_agent(
                "ownership".into(),
                PaneReleaseAgentParams {
                    pane_id: pane_id.clone(),
                    source: if accepted { &session.source } else { source }.into(),
                    agent: if accepted { &session.agent } else { agent }.into(),
                    seq: Some(30),
                    agent_session_id: Some("restored-session".into()),
                    agent_session_path: None,
                },
            ));
            let terminal = &app.state.terminals[&terminal_id];
            if accepted {
                assert!(terminal.hook_authority.is_none());
                assert_eq!(terminal.state, AgentState::Idle);
            } else {
                assert_eq!(terminal.hook_authority, authority);
                assert_eq!(terminal.state, AgentState::Working);
                assert_eq!(terminal.revision, revision);
            }
            assert!(terminal.persisted_agent_session.is_none());
            assert_eq!(terminal.detected_agent(), Some(Agent::Djinn));
            assert_eq!(terminal.effective_agent_label(), Some("djinn"));
            assert_eq!(terminal.pending_agent_resume_plan, pending);
            assert_eq!(app.state.session_dirty, accepted);
            assert_eq!(app.terminal_runtimes.len(), 1);
            assert_eq!(
                app.terminal_runtimes.get(&terminal_id).unwrap().child_pid(),
                None
            );
            app.state.assert_invariants_for_test();
        }
        // Sources have separate sequences; only the wrong-agent case proves
        // rejection preserved the legitimate owner's sequence at this value.
    }
}

#[tokio::test]
async fn api_unsupported_owner_path_is_discarded_before_takeover_sequence() {
    let (mut app, pane_id, terminal_id, session) = fixture("droid");
    let pending = app.state.terminals[&terminal_id]
        .pending_agent_resume_plan
        .clone();
    observe_live_fixture(&mut app, Agent::Claude);
    app.state.session_dirty = false;
    let revision = app.state.terminals[&terminal_id].revision;
    let path = std::env::current_dir()
        .unwrap()
        .join("unused-claude-session.jsonl")
        .display()
        .to_string();
    assert!(AgentSessionRef::path(path.clone()).is_some());
    let expected = PersistedAgentSession {
        source: "kitsune:claude".into(),
        agent: "claude".into(),
        session_ref: AgentSessionRef::id("claude-session").unwrap(),
    };
    for accepted in [false, true] {
        assert_ok(app.handle_pane_report_agent_session(
            "ownership".into(),
            PaneReportAgentSessionParams {
                pane_id: pane_id.clone(),
                source: expected.source.clone(),
                agent: expected.agent.clone(),
                seq: Some(20),
                agent_session_id: accepted.then(|| "claude-session".into()),
                agent_session_path: (!accepted).then(|| path.clone()),
                session_start_source: Some("startup".into()),
            },
        ));
        let terminal = &app.state.terminals[&terminal_id];
        assert_eq!(
            terminal.persisted_agent_session.as_ref(),
            Some(if accepted { &expected } else { &session })
        );
        if !accepted {
            assert_eq!(terminal.revision, revision);
        }
        assert!(terminal.hook_authority.is_none());
        assert_eq!(terminal.detected_agent(), Some(Agent::Claude));
        assert_eq!(terminal.state, AgentState::Idle);
        assert_eq!(terminal.pending_agent_resume_plan, pending);
        assert_eq!(app.state.session_dirty, accepted);
        assert_eq!(app.terminal_runtimes.len(), 1);
        assert_eq!(
            app.terminal_runtimes.get(&terminal_id).unwrap().child_pid(),
            None
        );
        app.state.assert_invariants_for_test();
    }
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
        assert_eq!(terminal.detected_agent(), None);
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
        assert_eq!(terminal.detected_agent(), None);
        assert_eq!(terminal.state, AgentState::Unknown);
        assert_eq!(terminal.pending_agent_resume_plan, pending);
        assert!(app.state.session_dirty);
        assert_eq!(app.terminal_runtimes.len(), 0);
        app.state.assert_invariants_for_test();
    }
}

#[tokio::test]
async fn api_rejected_identity_update_consumes_sequence_before_foreground_confirmation() {
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

    observe_live_fixture(&mut app, Agent::Claude);
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
        assert_eq!(terminal.detected_agent(), Some(Agent::Claude));
        assert_eq!(terminal.state, AgentState::Idle);
        assert_eq!(terminal.pending_agent_resume_plan, pending);
        assert_eq!(app.state.session_dirty, accepted);
        assert_eq!(app.terminal_runtimes.len(), 1);
        assert_eq!(
            app.terminal_runtimes.get(&terminal_id).unwrap().child_pid(),
            None
        );
        app.state.assert_invariants_for_test();
    }
}
