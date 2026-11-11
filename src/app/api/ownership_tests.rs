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
