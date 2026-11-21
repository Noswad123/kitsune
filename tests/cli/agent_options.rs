use super::harness::*;

#[test]
fn agent_wait_repeated_until_options_cannot_reenable_polling() {
    let socket = std::env::temp_dir().join("must-not-connect.sock");
    let output = run_cli(
        &socket,
        &[
            "agent",
            "wait",
            "missing",
            "--until",
            "working",
            "--until",
            "idle",
            "--timeout",
            "0",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "feature_retired");
    assert!(error["error"]["message"]
        .as_str()
        .unwrap()
        .contains("agent.wait"));
}
