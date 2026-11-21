use super::*;
use crate::api::schema::{EmptyParams, Method, Request};

#[test]
fn retired_manifest_success_dtos_still_decode() {
    for result in [
        serde_json::json!({"type": "agent_manifest_status", "last_check_unix": 123, "last_result": "checked", "manifests": []}),
        serde_json::json!({"type": "agent_manifest_reload", "manifests": []}),
    ] {
        let decoded: crate::api::schema::SuccessResponse =
            serde_json::from_value(serde_json::json!({"id": "historical", "result": result}))
                .unwrap();
        assert_eq!(decoded.id, "historical");
    }
}

#[tokio::test]
async fn manifest_management_is_globally_retired_without_side_effects() {
    for external in [false, true] {
        let mut state = crate::app::AppState::test_with_adversarial_identity_state();
        state.external_agent_management = external;
        for terminal in state.terminals.values_mut() {
            terminal.seed_restored_agent_identity(crate::detect::Agent::Codex);
            terminal.set_manual_label("held-label".into());
            let session = crate::agent_resume::PersistedAgentSession {
                source: "kitsune:codex".into(),
                agent: "codex".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id("held-history").unwrap(),
            };
            terminal.set_persisted_agent_session(session.clone());
            terminal.pending_agent_resume_plan = Some(crate::agent_resume::AgentResumePlan {
                agent: session.agent.clone(),
                argv: vec!["historical-command-must-not-run".into()],
                dedupe_key: "held-history".into(),
            });
            terminal.hook_authority = Some(crate::terminal::state::HookAuthority {
                source: "custom:codex".into(),
                agent_label: "codex".into(),
                state: crate::detect::AgentState::Idle,
                message: Some("held-native-status".into()),
                reported_at: Instant::now(),
                session_ref: Some(session.session_ref),
            });
        }
        let mut app = crate::app::test_support::app_from_state(state);
        for id in app.state.terminals.keys() {
            let (runtime, _) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
            app.terminal_runtimes.insert(id.clone(), runtime);
        }
        app.render_dirty.store(false, Ordering::Release);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        app.event_tx
            .try_send(AppEvent::PaneDied { pane_id })
            .unwrap();
        let before = snapshot(&app);
        for method in [
            Method::ServerAgentManifests(EmptyParams::default()),
            Method::ServerReloadAgentManifests(EmptyParams::default()),
        ] {
            let response: serde_json::Value =
                serde_json::from_str(&app.handle_api_request(Request {
                    id: "manifest-canary".into(),
                    method: method.clone(),
                }))
                .unwrap();
            assert_eq!(response["id"], "manifest-canary");
            assert_eq!(response["error"]["code"], "feature_retired");
            assert_eq!(
                response["error"]["message"],
                crate::api::schema::DETECTOR_MANIFEST_RETIREMENT_MESSAGE
            );
            assert!(response.get("result").is_none());
            let request = Request {
                id: "queued-canary".into(),
                method,
            };
            assert!(!crate::api::request_changes_ui(&request));
            assert!(crate::api::request_skips_default_workspace(&request));
            let (respond_to, response_rx) = std::sync::mpsc::channel();
            assert!(
                !app.handle_api_request_message(crate::api::ApiRequestMessage {
                    request,
                    respond_to,
                    response_write_complete: None,
                })
            );
            let queued: serde_json::Value =
                serde_json::from_str(&response_rx.recv().unwrap()).unwrap();
            assert_eq!(queued["id"], "queued-canary");
            assert_eq!(queued["error"]["code"], "feature_retired");
            assert_eq!(snapshot(&app), before);
            app.state.assert_invariants_for_test();
        }
        assert!(matches!(
            app.event_rx.try_recv().unwrap(),
            AppEvent::PaneDied { .. }
        ));
    }
}

fn snapshot(app: &App) -> String {
    let mut terminals: Vec<_> = app
        .state
        .terminals
        .iter()
        .map(|(id, terminal)| (id.to_string(), terminal.test_legacy_control_snapshot()))
        .collect();
    terminals.sort_by(|left, right| left.0.cmp(&right.0));
    format!(
        "{terminals:?} {:?} {:?} {:?} {} {} {}",
        app.state.toast,
        app.toast_deadline,
        app.session_save_deadline,
        app.state.session_dirty,
        app.render_dirty.load(Ordering::Acquire),
        app.event_hub.events_after(0).len()
    )
}
