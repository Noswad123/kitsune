use super::harness::*;

pub(super) struct RetirementTestDirectory(PathBuf);

impl RetirementTestDirectory {
    pub(super) fn new() -> Self {
        let directory = Self(std::env::temp_dir().join(format!(
                "ar-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        fs::create_dir(&directory.0).unwrap();
        directory
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for RetirementTestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove owned retirement test directory");
    }
}

#[test]
fn retired_agent_commands_never_contact_api_or_client_canaries() {
    let base = RetirementTestDirectory::new();
    let socket = base.path().join("api.sock");
    let client_socket = base.path().join("client.sock");
    let api = UnixListener::bind(&socket).unwrap();
    let client = UnixListener::bind(&client_socket).unwrap();
    api.set_nonblocking(true).unwrap();
    client.set_nonblocking(true).unwrap();
    let cases: &[(&[&str], &str)] = &[
        (&["list"], "agent.list"),
        (&["get", "private-target"], "agent.get"),
        (&["read", "private-target"], "agent.read"),
        (
            &["read", "private-target", "--format", "text"],
            "agent.read",
        ),
        (&["read", "private-target", "--ansi"], "agent.read"),
        (
            &["read", "private-target", "--source", "recent_unwrapped"],
            "agent.read",
        ),
        (
            &[
                "read",
                "private-target",
                "--source",
                "detection",
                "--format",
                "ansi",
                "--lines",
                "2",
            ],
            "agent.read",
        ),
        (&["send-keys", "private-target", "enter"], "agent.send_keys"),
        (&["prompt", "private-target", "--wait"], "agent.prompt"),
        (
            &["prompt", "private-target", "private-instructions"],
            "agent.prompt",
        ),
        (
            &[
                "prompt",
                "private-target",
                "private-instructions",
                "--wait",
                "--until",
                "idle",
            ],
            "agent.prompt",
        ),
        (
            &["rename", "private-target", "private-name"],
            "agent.rename",
        ),
        (&["rename", "private-target", "--clear"], "agent.rename"),
        (&["focus", "private-target"], "agent.focus"),
        (&["wait", "private-target"], "agent.wait"),
        (
            &[
                "wait",
                "private-target",
                "--until",
                "idle",
                "--until",
                "done",
                "--timeout",
                "100",
            ],
            "agent.wait",
        ),
        (
            &[
                "start",
                "private-name",
                "--kind",
                "pi",
                "--pane",
                "private-target",
                "--",
                "--private-arg",
            ],
            "agent.start",
        ),
        (&["attach", "private-target"], "agent.get"),
        (&["attach", "private-target", "--takeover"], "agent.get"),
        (&["explain", "private-target"], "agent.explain"),
        (&["explain", "private-target", "--json"], "agent.explain"),
        (
            &["explain", "private-target", "--format", "json"],
            "agent.explain",
        ),
        (
            &["explain", "private-target", "--format", "text", "--verbose"],
            "agent.explain",
        ),
    ];
    for (args, method) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_kitsune"))
            .arg("agent")
            .args(*args)
            .env("KITSUNE_SOCKET_PATH", &socket)
            .env("KITSUNE_CLIENT_SOCKET_PATH", &client_socket)
            .env("XDG_CONFIG_HOME", base.path().join("absent-config"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "feature_retired");
        assert_eq!(error["error"]["message"], format!("Legacy agent operation {method} is retired; Kitsune no longer provides built-in agent lifecycle, readiness, or status. Use generic terminal/pane operations, or an external agent manager."));
        assert_eq!(error["error"].as_object().unwrap().len(), 2);
        assert_eq!(
            api.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "{args:?}: no protocol ping or operation"
        );
        assert_eq!(
            client.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "{args:?}: no attach fallback"
        );
        assert!(!base.path().join("absent-config").exists());
    }
}

#[test]
fn offline_agent_explain_remains_local() {
    let base = RetirementTestDirectory::new();
    let file = base.path().join("screen.txt");
    fs::write(&file, "offline screen\n").unwrap();
    let socket = base.path().join("api.sock");
    let api = UnixListener::bind(&socket).unwrap();
    api.set_nonblocking(true).unwrap();
    for format in ["text", "json"] {
        let output = run_cli(
            &socket,
            &[
                "agent",
                "explain",
                "--file",
                file.to_str().unwrap(),
                "--agent",
                "pi",
                "--format",
                format,
            ],
        );
        assert!(output.status.success(), "{output:?}");
        if format == "json" {
            let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(value["agent"], "pi");
        } else {
            assert!(String::from_utf8_lossy(&output.stdout).contains("agent: pi"));
        }
        assert_eq!(
            api.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
