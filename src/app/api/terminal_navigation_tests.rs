use crate::api::schema::{Method, Request, TerminalFocusParams, TerminalInspectParams};
use crate::app::{test_support::app_from_state, App, AppState, Mode};
use crate::terminal::{TerminalId, TerminalRuntime};
use serde_json::{json, Value};

fn navigation_state() -> AppState {
    let mut state = AppState::test_with_adversarial_identity_state();
    state
        .workspaces
        .push(crate::workspace::Workspace::test_new("other"));
    state.ensure_test_terminals();
    state
}

fn call(app: &mut App, method: Method) -> Value {
    serde_json::from_str(&app.handle_api_request(Request {
        id: "navigation".into(),
        method,
    }))
    .unwrap()
}

fn inspect(app: &mut App, terminal_id: &TerminalId) -> Value {
    call(
        app,
        Method::TerminalInspect(TerminalInspectParams {
            terminal_id: terminal_id.to_string(),
        }),
    )
}

fn focus(app: &mut App, terminal_id: &str, generation: &str) -> Value {
    call(
        app,
        Method::TerminalFocus(TerminalFocusParams {
            terminal_id: terminal_id.into(),
            expected_runtime_generation: generation.into(),
        }),
    )
}

#[tokio::test]
async fn terminal_navigation_exact_focus_is_generic_and_does_not_acknowledge_or_send_input() {
    for external in [false, true] {
        let mut state = navigation_state();
        state.external_agent_management = external;
        let pane = state.workspaces[1].tabs[0].root_pane;
        let terminal_id = state.terminal_id_for_pane(1, pane).unwrap();
        for ws in &mut state.workspaces {
            for tab in &mut ws.tabs {
                for pane in tab.panes.values_mut() {
                    pane.seen = false;
                }
            }
        }
        let mut app = app_from_state(state);
        let (runtime, mut input) = TerminalRuntime::test_with_channel(80, 24);
        let generation = runtime.runtime_generation().to_owned();
        app.terminal_runtimes.insert(terminal_id.clone(), runtime);
        let inspected = inspect(&mut app, &terminal_id);
        assert_eq!(inspected["id"], "navigation");
        assert_eq!(inspected["result"]["availability"], "live");
        assert_eq!(inspected["result"]["runtime_generation"], generation);
        let response = focus(&mut app, &terminal_id.to_string(), &generation);
        assert_eq!(response["result"]["type"], "terminal_focused");
        assert_eq!(response["result"]["runtime_generation"], generation);
        assert_eq!(app.state.active, Some(1));
        assert_eq!(app.state.workspaces[1].focused_pane_id(), Some(pane));
        assert_eq!(app.state.mode, Mode::Terminal);
        assert!(app
            .state
            .workspaces
            .iter()
            .flat_map(|ws| &ws.tabs)
            .flat_map(|tab| tab.panes.values())
            .all(|p| !p.seen));
        assert!(input.try_recv().is_err());
        assert!(app.event_hub.events_after(0).is_empty());
        assert!(app.event_rx.try_recv().is_err());
        app.state.assert_invariants_for_test();
    }
}

#[tokio::test]
async fn terminal_navigation_rejects_stale_missing_and_alias_targets_without_mutations() {
    for external in [false, true] {
        let mut state = navigation_state();
        state.external_agent_management = external;
        let pane = state.workspaces[1].tabs[0].root_pane;
        let terminal_id = state.terminal_id_for_pane(1, pane).unwrap();
        state.terminals.get_mut(&terminal_id).unwrap().agent_name = Some("named-target".into());
        let mut app = app_from_state(state);
        let (runtime, mut input) = TerminalRuntime::test_with_channel(80, 24);
        let generation = runtime.runtime_generation().to_owned();
        app.terminal_runtimes.insert(terminal_id.clone(), runtime);
        let before = (
            app.state.current_pane_focus_target(),
            app.state.mode,
            app.state.session_dirty,
        );
        let seen = app
            .state
            .workspaces
            .iter()
            .flat_map(|ws| &ws.tabs)
            .flat_map(|tab| tab.panes.values())
            .map(|p| p.seen)
            .collect::<Vec<_>>();
        for (target, token) in [
            (terminal_id.to_string(), "stale".to_owned()),
            (app.public_pane_id(1, pane).unwrap(), generation.clone()),
            (pane.raw().to_string(), generation.clone()),
            ("missing".into(), generation.clone()),
            ("named-target".into(), generation.clone()),
            (String::new(), generation.clone()),
        ] {
            assert!(focus(&mut app, &target, &token).get("error").is_some());
            assert_eq!(
                (
                    app.state.current_pane_focus_target(),
                    app.state.mode,
                    app.state.session_dirty
                ),
                before
            );
            assert_eq!(
                app.state
                    .workspaces
                    .iter()
                    .flat_map(|ws| &ws.tabs)
                    .flat_map(|tab| tab.panes.values())
                    .map(|p| p.seen)
                    .collect::<Vec<_>>(),
                seen
            );
            assert!(input.try_recv().is_err());
            assert!(app.event_hub.events_after(0).is_empty());
            assert!(app.event_rx.try_recv().is_err());
            assert!(app.session_save_deadline.is_none());
            app.state.assert_invariants_for_test();
        }
    }
}

#[tokio::test]
async fn terminal_navigation_replacement_exit_detach_and_move() {
    let state = navigation_state();
    let pane = state.workspaces[0].tabs[0].root_pane;
    let other = state.workspaces[1].tabs[0].root_pane;
    let terminal_id = state.terminal_id_for_pane(0, pane).unwrap();
    let other_id = state.terminal_id_for_pane(1, other).unwrap();
    let mut app = app_from_state(state);
    let (mut runtime, _input) = TerminalRuntime::test_with_channel(80, 24);
    runtime.test_set_child_pid(12345);
    let old_pid = runtime.child_pid();
    let old = runtime.runtime_generation().to_owned();
    app.terminal_runtimes.insert(terminal_id.clone(), runtime);
    let (mut replacement, _input) = TerminalRuntime::test_with_channel(80, 24);
    replacement.test_set_child_pid(12345);
    assert_eq!(replacement.child_pid(), old_pid);
    let generation = replacement.runtime_generation().to_owned();
    assert_ne!(old, generation);
    assert_eq!(
        uuid::Uuid::parse_str(&generation)
            .unwrap()
            .get_version_num(),
        4
    );
    replacement.test_set_process_exited(true);
    app.terminal_runtimes
        .insert(terminal_id.clone(), replacement);
    assert_eq!(
        inspect(&mut app, &terminal_id)["result"]["availability"],
        "exited"
    );
    assert!(focus(&mut app, &terminal_id.to_string(), &generation)
        .get("error")
        .is_some());
    let mut replacement = app.terminal_runtimes.remove(&terminal_id).unwrap();
    replacement.test_set_process_exited(false);
    app.terminal_runtimes
        .insert(terminal_id.clone(), replacement);
    assert_eq!(
        focus(&mut app, &terminal_id.to_string(), &old)["error"]["code"],
        "runtime_generation_mismatch"
    );
    app.state.workspaces[0].tabs[0]
        .panes
        .get_mut(&pane)
        .unwrap()
        .attached_terminal_id = other_id.clone();
    assert_eq!(
        inspect(&mut app, &terminal_id)["result"]["availability"],
        "detached"
    );
    assert_eq!(
        inspect(&mut app, &terminal_id)["result"]["pane_id"],
        Value::Null
    );
    assert!(focus(&mut app, &terminal_id.to_string(), &generation)
        .get("error")
        .is_some());
    app.state.workspaces[1].tabs[0]
        .panes
        .get_mut(&other)
        .unwrap()
        .attached_terminal_id = terminal_id.clone();
    let moved = inspect(&mut app, &terminal_id);
    assert_eq!(moved["result"]["runtime_generation"], generation);
    assert_eq!(
        moved["result"]["pane_id"],
        app.public_pane_id(1, other).unwrap()
    );
    assert!(focus(&mut app, &terminal_id.to_string(), &generation)
        .get("result")
        .is_some());
    app.state.assert_invariants_for_test();
    app.terminal_runtimes.remove(&terminal_id);
    assert_eq!(
        inspect(&mut app, &terminal_id)["result"]["availability"],
        "unavailable"
    );
    assert_eq!(
        inspect(&mut app, &terminal_id)["result"]["runtime_generation"],
        Value::Null
    );
    assert!(focus(&mut app, &terminal_id.to_string(), &generation)
        .get("error")
        .is_some());
}

#[test]
fn terminal_navigation_strict_requests_and_old_snapshot() {
    for (method, good) in [
        ("terminal.inspect", json!({"terminal_id":"exact"})),
        (
            "terminal.focus",
            json!({"terminal_id":"exact", "expected_runtime_generation":"opaque"}),
        ),
    ] {
        assert!(serde_json::from_value::<Request>(
            json!({"id":"r", "method":method, "params":good})
        )
        .is_ok());
        for key in good.as_object().unwrap().keys() {
            let mut missing = good.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(serde_json::from_value::<Request>(
                json!({"id":"r", "method":method, "params":missing})
            )
            .is_err());
            let mut invalid = good.clone();
            invalid[key] = json!(17);
            assert!(serde_json::from_value::<Request>(
                json!({"id":"r", "method":method, "params":invalid})
            )
            .is_err());
        }
        let mut unknown = good.clone();
        unknown["pane_id"] = json!("alias");
        assert!(serde_json::from_value::<Request>(
            json!({"id":"r", "method":method, "params":unknown})
        )
        .is_err());
    }
    let mut app = app_from_state(AppState::test_new());
    let snapshot = call(&mut app, Method::SessionSnapshot(Default::default()));
    assert_eq!(
        snapshot["result"]["snapshot"]["terminal_navigation"],
        json!({"version":1})
    );
    let mut old = snapshot["result"]["snapshot"].clone();
    old.as_object_mut().unwrap().remove("terminal_navigation");
    let old: crate::api::schema::SessionSnapshot = serde_json::from_value(old).unwrap();
    assert_eq!(old.terminal_navigation, None);
}

#[tokio::test]
async fn terminal_navigation_unavailable_exit_detach_and_ambiguity_are_side_effect_free() {
    for external in [false, true] {
        for failure in ["unavailable", "exited", "detached", "ambiguous"] {
            let mut state = navigation_state();
            state.external_agent_management = external;
            let pane = state.workspaces[1].tabs[0].root_pane;
            let terminal_id = state.terminal_id_for_pane(1, pane).unwrap();
            let other = state.workspaces[0].tabs[0].root_pane;
            if failure == "detached" {
                state.workspaces[1].tabs[0]
                    .panes
                    .get_mut(&pane)
                    .unwrap()
                    .attached_terminal_id = TerminalId::alloc();
            }
            if failure == "ambiguous" {
                state.workspaces[0].tabs[0]
                    .panes
                    .get_mut(&other)
                    .unwrap()
                    .attached_terminal_id = terminal_id.clone();
            }
            let mut app = app_from_state(state);
            let (mut runtime, mut input) = TerminalRuntime::test_with_channel(80, 24);
            runtime.test_set_process_exited(failure == "exited");
            let generation = runtime.runtime_generation().to_owned();
            runtime
                .try_send_bytes(bytes::Bytes::from_static(b"existing"))
                .unwrap();
            if failure != "unavailable" {
                app.terminal_runtimes.insert(terminal_id.clone(), runtime);
            }
            let before = (
                app.state.current_pane_focus_target(),
                app.state.mode,
                app.state.session_dirty,
            );
            let terminal_before = (
                app.state.terminals[&terminal_id].revision,
                app.state.terminals[&terminal_id].state,
            );
            let seen = app
                .state
                .workspaces
                .iter()
                .flat_map(|ws| &ws.tabs)
                .flat_map(|tab| tab.panes.values())
                .map(|p| p.seen)
                .collect::<Vec<_>>();
            let response = focus(&mut app, &terminal_id.to_string(), &generation);
            assert_eq!(response["error"]["code"], format!("terminal_{failure}"));
            assert_eq!(
                (
                    app.state.current_pane_focus_target(),
                    app.state.mode,
                    app.state.session_dirty
                ),
                before
            );
            assert_eq!(
                (
                    app.state.terminals[&terminal_id].revision,
                    app.state.terminals[&terminal_id].state
                ),
                terminal_before
            );
            assert_eq!(
                app.state
                    .workspaces
                    .iter()
                    .flat_map(|ws| &ws.tabs)
                    .flat_map(|tab| tab.panes.values())
                    .map(|p| p.seen)
                    .collect::<Vec<_>>(),
                seen
            );
            assert_eq!(input.try_recv().unwrap(), b"existing".as_slice());
            assert!(input.try_recv().is_err());
            assert!(app.event_rx.try_recv().is_err());
            assert!(app.event_hub.events_after(0).is_empty());
            assert!(app.session_save_deadline.is_none());
        }
    }
}

#[test]
fn terminal_navigation_request_classification_skips_bootstrap() {
    for method in [
        Method::TerminalInspect(TerminalInspectParams {
            terminal_id: "missing".into(),
        }),
        Method::TerminalFocus(TerminalFocusParams {
            terminal_id: "missing".into(),
            expected_runtime_generation: "missing".into(),
        }),
    ] {
        let changes_ui = matches!(method, Method::TerminalFocus(_));
        let request = Request {
            id: "r".into(),
            method,
        };
        assert!(crate::api::request_skips_default_workspace(&request));
        assert_eq!(crate::api::request_changes_ui(&request), changes_ui);
    }
}
