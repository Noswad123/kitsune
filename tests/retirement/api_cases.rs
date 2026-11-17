use super::*;

#[test]
fn typed_legacy_methods_and_hook_ingress_are_globally_retired_over_a_real_socket() {
    let _lock = test_lock();
    for external in [false, true] {
        let base = unique_test_dir();
        let config = base.join("config");
        let runtime = base.join("runtime");
        let socket = runtime.join("kitsune.sock");
        let ownership = if external {
            "[integrations]\nsources=['custom']\n[integrations.runtime]\nsource='custom'\ncommand=['/not-installed']\nreport_command=['/not-installed-report']\nexternal_agent_management=true\n"
        } else {
            "[integrations]\nsources=[]\n"
        };
        let child = spawn_kitsune_with_config(
            &config,
            &runtime,
            &socket,
            None,
            "/bin/sh",
            &format!("onboarding=false\n{ownership}"),
        );
        wait_for_socket(&socket, Duration::from_secs(5));
        for (method, params) in [
            ("agent.list", serde_json::json!({})),
            ("agent.get", serde_json::json!({"target":"missing"})),
            ("agent.focus", serde_json::json!({"target":"missing"})),
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
                serde_json::json!({"target":"missing","text":"","wait":{"until":[],"timeout_ms":60000}}),
            ),
            (
                "agent.wait",
                serde_json::json!({"target":"missing","until":[],"timeout_ms":60000}),
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
                serde_json::json!({"source":"invalid source"}),
            ),
            (
                "agent.view.clear",
                serde_json::json!({"source":"invalid source"}),
            ),
        ] {
            let start = Instant::now();
            let response = send_request(
                &socket,
                &serde_json::json!({"id":"retired", "method":method, "params":params}).to_string(),
            );
            assert_eq!(response["id"], "retired");
            assert_eq!(response["error"]["code"], "feature_retired");
            assert_eq!(response["error"]["message"], format!("Legacy agent operation {method} is retired; Kitsune no longer provides built-in agent lifecycle, readiness, or status. Use generic terminal/pane operations, or an external agent manager."));
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "{method} polled instead of retiring"
            );
        }
        for method in [
            "pane.report_agent",
            "pane.report_agent_session",
            "pane.clear_agent_authority",
            "pane.release_agent",
        ] {
            let response = send_request(
                &socket,
                &serde_json::json!({
                    "id":"hook", "method":method,
                    "params":{"pane_id":"missing","source":"","agent":"","state":"working"}
                })
                .to_string(),
            );
            assert_eq!(
                response["error"]["code"],
                if external {
                    "external_agent_management"
                } else {
                    "feature_retired"
                }
            );
        }
        let snapshot = send_request(
            &socket,
            r#"{"id":"snapshot","method":"session.snapshot","params":{}}"#,
        );
        assert_eq!(
            snapshot["result"]["snapshot"]["agents"],
            serde_json::json!([])
        );
        assert!(
            snapshot["result"]["snapshot"]["workspaces"]
                .as_array()
                .unwrap()
                .is_empty(),
            "retired requests bootstrapped a workspace"
        );
        cleanup_spawned_kitsune(child, base);
    }
}

#[test]
fn generic_metadata_patch_sequence_ttl_and_subscription_survive_retirement() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config = base.join("config");
    let runtime = base.join("runtime");
    let socket = runtime.join("kitsune.sock");
    let child = spawn_kitsune(&config, &runtime, &socket);
    wait_for_socket(&socket, Duration::from_secs(5));
    let created = send_request(&socket, &serde_json::json!({"id":"create", "method":"workspace.create", "params":{"cwd":base,"focus":false}}).to_string());
    let pane = created["result"]["root_pane"]["pane_id"].as_str().unwrap();
    let mut events = open_subscription(&socket, &serde_json::json!({"id":"subscribe", "method":"events.subscribe", "params":{"subscriptions":[{"type":"pane.updated","pane_id":pane}]}}).to_string());
    assert_eq!(
        events.read_json_line(Duration::from_secs(2))["result"]["type"],
        "subscription_started"
    );
    let report = |seq, tokens, extra: serde_json::Value| {
        let mut params = serde_json::json!({"pane_id":pane, "source":"kitsune:codex", "seq":seq, "ttl_ms":500, "tokens":tokens});
        params
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        send_request(
            &socket,
            &serde_json::json!({"id":"metadata", "method":"pane.report_metadata", "params":params})
                .to_string(),
        )
    };
    assert_eq!(
        report(
            1,
            serde_json::json!({"summary":"first","agent":"opaque token"}),
            serde_json::json!({})
        )["result"]["type"],
        "ok"
    );
    let event = events.read_json_line(Duration::from_secs(2));
    assert_eq!(event["data"]["pane"]["tokens"]["summary"], "first");
    let rejected = report(
        100,
        serde_json::json!({"summary":"must not replace"}),
        serde_json::json!({"agent":"codex","title":"legacy"}),
    );
    assert_eq!(rejected["error"]["code"], "feature_retired");
    assert_eq!(
        report(
            2,
            serde_json::json!({"summary":"fresh","agent":"opaque token"}),
            serde_json::json!({})
        )["result"]["type"],
        "ok"
    );
    report(
        1,
        serde_json::json!({"summary":"stale"}),
        serde_json::json!({}),
    );
    let event = events.read_json_line(Duration::from_secs(2));
    assert_eq!(event["data"]["pane"]["tokens"]["summary"], "fresh");
    let expired = events.read_json_line(Duration::from_secs(3));
    assert!(
        expired["data"]["pane"]["tokens"].is_null()
            || expired["data"]["pane"]["tokens"]
                .as_object()
                .is_some_and(|tokens| tokens.is_empty())
    );
    assert_eq!(expired["data"]["pane"]["agent_status"], "unknown");
    assert!(expired["data"]["pane"]["agent"].is_null());
    cleanup_spawned_kitsune(child, base);
}
