use super::harness::*;

#[test]
fn legacy_agent_commands_retire_locally_without_a_server() {
    let socket = std::env::temp_dir().join("must-not-connect.sock");
    for (method, args) in [
        ("agent.list", vec!["agent", "list"]),
        ("agent.get", vec!["agent", "get", "missing"]),
        ("agent.focus", vec!["agent", "focus", "missing"]),
        (
            "agent.rename",
            vec!["agent", "rename", "missing", "Invalid name"],
        ),
        (
            "agent.start",
            vec![
                "agent",
                "start",
                "Invalid name",
                "--kind",
                "pi",
                "--pane",
                "missing",
            ],
        ),
        (
            "agent.prompt",
            vec![
                "agent",
                "prompt",
                "missing",
                "private prompt",
                "--wait",
                "--timeout",
                "0",
            ],
        ),
        (
            "agent.wait",
            vec!["agent", "wait", "missing", "--timeout", "0"],
        ),
        ("agent.read", vec!["agent", "read", "missing"]),
        ("agent.explain", vec!["agent", "explain", "missing"]),
        (
            "agent.send_keys",
            vec!["agent", "send-keys", "missing", "invalid key"],
        ),
    ] {
        let output = run_cli(&socket, &args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "feature_retired", "{args:?}");
        assert_eq!(error["error"]["message"], format!("Legacy agent operation {method} is retired; Kitsune no longer provides built-in agent lifecycle, readiness, or status. Use generic terminal/pane operations, or an external agent manager."));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private prompt"));
        assert!(output.stdout.is_empty());
    }
}
