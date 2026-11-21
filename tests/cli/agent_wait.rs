use super::harness::*;

#[test]
fn retired_agent_commands_preserve_syntax_errors_and_help() {
    let base = super::agent_transport::RetirementTestDirectory::new();
    let socket = base.path().join("api.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let invalid: &[&[&str]] = &[
        &["list", "--json"],
        &["get"],
        &["get", "target", "--json"],
        &["read", "target", "--format", "json"],
        &["read", "target", "--source", "invalid"],
        &["read", "target", "--lines", "invalid"],
        &["prompt", "target", "text", "--until", "idle"],
        &["prompt", "target", "text", "--timeout", "1"],
        &["prompt", "target", "text", "--wait", "--until", "invalid"],
        &["wait", "target", "--timeout", "invalid"],
        &["wait", "target", "--until", "invalid"],
        &["start", "name", "--kind", "pi"],
        &[
            "start",
            "name",
            "--kind",
            "pi",
            "--pane",
            "target",
            "--timeout",
            "invalid",
        ],
        &["attach", "target", "--unknown"],
        &["rename", "target"],
        &["explain", "target", "--format", "ansi"],
        &["explain", "--file", "absent"],
    ];
    for args in invalid {
        let mut command = vec!["agent"];
        command.extend_from_slice(args);
        let output = run_cli(&socket, &command);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("feature_retired"));
    }
    for subcommand in [
        "list",
        "get",
        "read",
        "send-keys",
        "prompt",
        "rename",
        "focus",
        "wait",
        "start",
        "attach",
        "explain",
    ] {
        let output = run_cli(&socket, &["agent", subcommand, "--help"]);
        assert!(output.status.success(), "{subcommand}: {output:?}");
    }
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
