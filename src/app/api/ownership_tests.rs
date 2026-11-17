use crate::api::schema::*;
use crate::app::test_support::{legacy_control_fixture, legacy_control_snapshot};

#[tokio::test]
async fn legacy_agent_methods_are_globally_retired_before_validation_or_side_effects() {
    for external in [false, true] {
        let (mut app, _, terminal) = legacy_control_fixture();
        app.state.external_agent_management = external;
        let (runtime, mut input) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        app.terminal_runtimes.insert(terminal, runtime);
        let cases = [
            ("agent.list", serde_json::json!({})),
            ("agent.get", serde_json::json!({"target":"missing"})),
            ("agent.focus", serde_json::json!({"target":"ambiguous"})),
            (
                "agent.rename",
                serde_json::json!({"target":"missing","name":"Invalid name"}),
            ),
            (
                "agent.start",
                serde_json::json!({"pane_id":"missing","name":"Invalid name","kind":"unknown","args":["\n"],"timeout_ms":0}),
            ),
            (
                "agent.prompt",
                serde_json::json!({"target":"missing","text":"","wait":{"until":[],"timeout_ms":0}}),
            ),
            (
                "agent.wait",
                serde_json::json!({"target":"missing","until":[],"timeout_ms":0}),
            ),
            (
                "agent.read",
                serde_json::json!({"target":"missing","source":"detection"}),
            ),
            ("agent.explain", serde_json::json!({"target":"missing"})),
            (
                "agent.send_keys",
                serde_json::json!({"target":"missing","keys":["invalid key"]}),
            ),
            (
                "agent.view.set",
                serde_json::json!({"source":"invalid source","filter":{"op":"any","filters":[]}}),
            ),
            (
                "agent.view.clear",
                serde_json::json!({"source":"invalid source"}),
            ),
        ];
        for (method, params) in cases {
            let request: Request = serde_json::from_value(
                serde_json::json!({"id":"req","method":method,"params":params}),
            )
            .unwrap();
            let before = legacy_control_snapshot(&app);
            let response = app.handle_api_request(request.clone());
            let response: ErrorResponse = serde_json::from_str(&response).unwrap();
            let expected = legacy_agent_retirement_error(
                legacy_agent_retired_method(&request.method).unwrap(),
            );
            assert_eq!(response.error, expected, "{method}, external={external}");
            assert_eq!(legacy_control_snapshot(&app), before, "{method}");
            assert!(input.try_recv().is_err());
        }
        assert!(app.collect_agent_infos().is_empty());
        let response = app.handle_session_snapshot("snapshot".into());
        let value: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(value["result"]["snapshot"]["agents"], serde_json::json!([]));
        assert!(crate::ui::agent_panel_entries(&app.state).is_empty());
        app.state.assert_invariants_for_test();
    }
}

#[test]
fn retired_agent_helpers_do_not_depend_on_dispatch_guards() {
    for external in [false, true] {
        let (mut app, _, _) = legacy_control_fixture();
        app.state.external_agent_management = external;
        let before = legacy_control_snapshot(&app);
        let target = AgentTarget {
            target: "missing".into(),
        };
        for response in [
            app.handle_agent_get("get".into(), target.clone()),
            app.handle_agent_focus("focus".into(), target.clone()),
            app.handle_agent_explain("explain".into(), target),
            app.handle_agent_prompt(
                "prompt".into(),
                AgentPromptParams {
                    target: "missing".into(),
                    text: String::new(),
                    wait: None,
                },
            ),
        ] {
            let response: ErrorResponse = serde_json::from_str(&response).unwrap();
            assert_eq!(response.error.code, "feature_retired");
        }
        assert_eq!(legacy_control_snapshot(&app), before);
    }
}

#[test]
fn legacy_hook_ingress_preserves_external_error_precedence_without_mutation() {
    for external in [false, true] {
        let (mut app, _, _) = legacy_control_fixture();
        app.state.external_agent_management = external;
        for method in [
            "pane.report_agent",
            "pane.report_agent_session",
            "pane.clear_agent_authority",
            "pane.release_agent",
        ] {
            let request: Request = serde_json::from_value(serde_json::json!({
                "id":"hook", "method":method, "params":{
                    "pane_id":"missing", "source":"invalid source", "agent":"", "state":"working", "seq":0
                }
            })).unwrap();
            let before = legacy_control_snapshot(&app);
            let response = app.handle_api_request(request);
            let response: ErrorResponse = serde_json::from_str(&response).unwrap();
            if external {
                assert_eq!(response.error.code, "external_agent_management");
                assert_eq!(response.error.message, "legacy agent reports are disabled");
            } else {
                assert_eq!(response.error.code, "feature_retired");
                assert!(response.error.message.contains(method));
            }
            assert_eq!(legacy_control_snapshot(&app), before);
            assert!(app.event_rx.try_recv().is_err());
        }
    }
}

#[tokio::test]
async fn retired_dispatch_does_not_drain_a_queued_generic_exit() {
    for external in [false, true] {
        let (mut app, _, _) = legacy_control_fixture();
        app.state.external_agent_management = external;
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        app.event_tx
            .try_send(crate::events::AppEvent::PaneDied { pane_id })
            .unwrap();
        let before = legacy_control_snapshot(&app);
        let response = app.dispatch_api_request(
            "queued",
            Method::AgentWait(AgentWaitParams {
                target: "missing".into(),
                until: vec![],
                timeout_ms: Some(0),
            }),
        );
        let error: ErrorResponse = serde_json::from_str(&response).unwrap();
        assert_eq!(error.error, legacy_agent_retirement_error("agent.wait"));
        assert_eq!(legacy_control_snapshot(&app), before);
        assert!(matches!(
            app.event_rx.try_recv(),
            Ok(crate::events::AppEvent::PaneDied { .. })
        ));
    }
}

#[tokio::test]
async fn generic_metadata_sequence_ttl_and_tokens_are_not_agent_authority() {
    for external in [false, true] {
        let (mut app, pane, terminal) = legacy_control_fixture();
        app.state.external_agent_management = external;
        let request = |params| {
            serde_json::from_value::<Request>(serde_json::json!({
                "id":"tokens", "method":"pane.report_metadata", "params":params,
            }))
            .unwrap()
        };
        let params = serde_json::json!({"pane_id":pane, "source":"kitsune:codex", "seq":1,
            "ttl_ms":100, "tokens":{"agent":"opaque generic token", "summary":"first"}});
        let initial_revision = app.state.terminals[&terminal].revision;
        let response = app.handle_api_request(request(params.clone()));
        assert!(
            serde_json::from_str::<SuccessResponse>(&response).is_ok(),
            "{response}"
        );
        let tokens = app.state.terminals[&terminal].metadata_tokens.values();
        assert_eq!(tokens["agent"], "opaque generic token");
        assert!(app.collect_agent_infos().is_empty());
        assert!(app.state.terminals[&terminal]
            .effective_agent_label()
            .is_none());
        assert_eq!(
            app.state.terminals[&terminal].revision,
            initial_revision + 1
        );
        for extra in [
            serde_json::json!({"agent":"codex"}),
            serde_json::json!({"applies_to_source":"kitsune:codex"}),
            serde_json::json!({"title":"old title"}),
            serde_json::json!({"display_agent":"old agent"}),
            serde_json::json!({"state_labels":{"idle":"ready"}}),
            serde_json::json!({"clear_title":true}),
            serde_json::json!({"clear_display_agent":true}),
            serde_json::json!({"clear_state_labels":true}),
        ] {
            let mut mixed = params.clone();
            mixed["seq"] = serde_json::json!(100);
            mixed["tokens"] = serde_json::json!({"summary":"must not change"});
            mixed
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            let response = app.handle_api_request(request(mixed));
            let error: ErrorResponse = serde_json::from_str(&response).unwrap();
            assert_eq!(
                error.error,
                legacy_agent_retirement_error("pane.report_metadata")
            );
            assert_eq!(
                app.state.terminals[&terminal].metadata_tokens.values(),
                tokens
            );
            assert_eq!(
                app.state.terminals[&terminal].revision,
                initial_revision + 1
            );
        }
        let mut fresh = params.clone();
        fresh["seq"] = serde_json::json!(2);
        fresh["tokens"] = serde_json::json!({"summary":"fresh"});
        let response = app.handle_api_request(request(fresh));
        assert!(serde_json::from_str::<SuccessResponse>(&response).is_ok());
        app.handle_api_request(request(params));
        assert_eq!(
            app.state.terminals[&terminal].metadata_tokens.values()["summary"],
            "fresh"
        );
        let deadline = app.agent_metadata_deadline.unwrap();
        app.expire_metadata_at(deadline, deadline + std::time::Duration::from_secs(1));
        assert!(app.state.terminals[&terminal]
            .metadata_tokens
            .values()
            .is_empty());
        assert!(app
            .event_hub
            .events_after(0)
            .iter()
            .all(|(_, event)| !matches!(
                event.event,
                EventKind::PaneAgentDetected | EventKind::PaneAgentStatusChanged
            )));
    }
}

#[test]
fn historical_agent_aliases_and_view_overrides_cannot_drive_navigation() {
    for external in [false, true] {
        let (mut app, _, _) = legacy_control_fixture();
        app.state.external_agent_management = external;
        app.state.agent_view_override = Some(AgentViewSetParams {
            source: "old.view".into(),
            label: Some("old".into()),
            filter: None,
            sort: vec![],
        });
        let before = legacy_control_snapshot(&app);
        assert!(app.resolve_terminal_target("ambiguous").is_err());
        assert!(crate::ui::agent_panel_entries(&app.state).is_empty());
        app.state.navigator.scope = crate::app::state::NavigatorScope::Agents;
        assert!(app.state.navigator_rows().is_empty());
        app.state.navigator.scope = crate::app::state::NavigatorScope::Tree;
        assert!(!app.state.navigator_rows().is_empty());
        assert_eq!(legacy_control_snapshot(&app), before);
    }
}
