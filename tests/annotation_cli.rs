#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn configure_peer(stream: &UnixStream) {
    // macOS can inherit the listener's nonblocking mode. Keep accept polling
    // nonblocking, but make each peer honor its bounded read/write timeouts.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
}

struct Fixture {
    root: PathBuf,
    listener: Option<UnixListener>,
}

struct RunningCli(Child);

impl Drop for RunningCli {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.listener.take());
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "tests/.annotation-cli-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let mut fixture = Self {
            root,
            listener: None,
        };
        for directory in ["config", "runtime", "state", "cache"] {
            fs::create_dir(fixture.root.join(directory)).unwrap();
        }
        let socket = fixture.root.join("s");
        assert!(
            socket.as_os_str().as_bytes().len() < 104,
            "Unix socket path too long"
        );
        let listener = UnixListener::bind(socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        fixture.listener = Some(listener);
        fixture
    }

    fn run(&self, args: &[&str], reject: bool) -> (Output, Vec<Value>) {
        self.run_with_error(args, reject.then_some("annotation_stale_revision"))
    }

    fn run_with_error(&self, args: &[&str], reject: Option<&str>) -> (Output, Vec<Value>) {
        let stdout = self.root.join("stdout");
        let stderr = self.root.join("stderr");
        let mut command = Command::new(env!("CARGO_BIN_EXE_kitsune"));
        for (name, _) in std::env::vars_os() {
            if name.as_bytes().starts_with(b"KITSUNE_") {
                command.env_remove(name);
            }
        }
        command
            .args(args)
            .current_dir(&self.root)
            .env("HOME", &self.root)
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_RUNTIME_DIR", self.root.join("runtime"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_CACHE_HOME", self.root.join("cache"))
            .env("KITSUNE_CONFIG_PATH", self.root.join("config.toml"))
            .env("KITSUNE_SOCKET_PATH", self.root.join("s"))
            .env_remove("KITSUNE_CLIENT_SOCKET_PATH")
            .env_remove("KITSUNE_ENV")
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap());
        let mut child = RunningCli(command.spawn().unwrap());
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut requests = Vec::new();
        let status = loop {
            assert!(Instant::now() < deadline, "CLI watchdog expired: {args:?}");
            let exited = child.0.try_wait().unwrap();
            match self.listener.as_ref().unwrap().accept() {
                Ok((mut stream, _)) => {
                    configure_peer(&stream);
                    let mut line = String::new();
                    BufReader::new((&stream).take(64 * 1024))
                        .read_line(&mut line)
                        .unwrap();
                    let request: Value = serde_json::from_str(&line).unwrap();
                    let response = if request["method"] == "ping" {
                        // Same pong shape as tests/cli/harness.rs::write_fake_pong.
                        json!({"id": request["id"], "result": {
                            "type": "pong", "version": "different-build-same-protocol", "protocol": 18,
                            "capabilities": {"live_handoff": true, "detached_server_daemon": true}
                        }})
                    } else if let Some(code) = reject {
                        json!({"id": request["id"], "error": {
                            "code": code, "message": "annotation revision rejected"
                        }})
                    } else {
                        json!({"id": request["id"], "result": {"type": "ok"}})
                    };
                    writeln!(stream, "{response}").unwrap();
                    stream.flush().unwrap();
                    requests.push(request);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if let Some(status) = exited {
                        break status;
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("fake socket accept failed: {error}"),
            }
        };
        (
            Output {
                status,
                stdout: fs::read(stdout).unwrap(),
                stderr: fs::read(stderr).unwrap(),
            },
            requests,
        )
    }
}

fn annotation_args() -> Vec<&'static str> {
    vec![
        "pane",
        "annotate",
        "w1:p1",
        "--expected-terminal",
        "term_1",
        "--annotation-epoch",
        "epoch_1",
        "--source",
        "build",
        "--revision",
        "1",
        "--ttl-ms",
        "5000",
        "--text",
        "Build: running",
    ]
}

fn check_transport(clear: bool, reject: bool) {
    let fixture = Fixture::new();
    let mut args = annotation_args();
    if clear {
        args.truncate(13);
        args.push("--clear");
    }
    let (output, requests) = fixture.run(&args, reject);
    assert_eq!(
        output.status.code(),
        Some(if reject { 1 } else { 0 }),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    if reject {
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(
            error,
            json!({"id": "cli:request", "error": {
                "code": "annotation_stale_revision", "message": "annotation revision rejected"
            }})
        );
    } else {
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        requests.len(),
        2,
        "unexpected operation or retry: {requests:?}"
    );
    assert_eq!(requests[0]["method"], "ping");
    assert_eq!(
        requests[1],
        json!({"id": "cli:request", "method": "pane.annotate", "params": {
            "pane_id": "w1:p1", "expected_terminal_id": "term_1", "annotation_epoch": "epoch_1",
            "source": "build", "revision": 1,
            "text": if clear { None } else { Some("Build: running") }, "ttl_ms": 5000
        }})
    );
}

#[test]
fn annotation_fixture_configures_nonblocking_peers_for_bounded_reads() {
    let (peer, mut client) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    configure_peer(&peer);
    // Inspect only this owned test descriptor, so Linux also catches the macOS
    // inherited-mode regression deterministically rather than relying on a race.
    let flags = unsafe { libc::fcntl(peer.as_raw_fd(), libc::F_GETFL) };
    assert_ne!(flags, -1);
    assert_eq!(flags & libc::O_NONBLOCK, 0);
    assert_eq!(peer.read_timeout().unwrap(), Some(Duration::from_secs(2)));
    assert_eq!(peer.write_timeout().unwrap(), Some(Duration::from_secs(2)));
    let writer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        writeln!(client, "{{\"id\":\"delayed-peer\"}}").unwrap();
    });
    let mut line = String::new();
    BufReader::new(peer).read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap(),
        json!({"id": "delayed-peer"})
    );
    writer.join().unwrap();
}

#[test]
fn pane_annotation_cli_sends_only_ping_then_one_annotation() {
    check_transport(false, false);
}

#[test]
fn pane_annotation_cli_clear_sends_explicit_null() {
    check_transport(true, false);
}

#[test]
fn pane_annotation_cli_server_error_exits_one_without_retry() {
    check_transport(false, true);
}

#[test]
fn pane_annotation_cli_deadline_is_exact_on_text_clear_and_expired_error_without_retry() {
    for clear in [false, true] {
        for error in [None, Some("annotation_expired")] {
            let fixture = Fixture::new();
            let mut args = annotation_args();
            if clear {
                args.truncate(13);
                args.push("--clear");
            }
            args.extend(["--valid-until-unix-ms", "9007199254740991"]);
            let (output, requests) = fixture.run_with_error(&args, error);
            assert_eq!(
                output.status.code(),
                Some(if error.is_some() { 1 } else { 0 })
            );
            assert!(output.stdout.is_empty());
            assert_eq!(
                requests.len(),
                2,
                "unexpected discovery or retry: {requests:?}"
            );
            assert_eq!(requests[0]["method"], "ping");
            assert_eq!(
                requests[1],
                json!({"id": "cli:request", "method": "pane.annotate", "params": {
                    "pane_id": "w1:p1", "expected_terminal_id": "term_1", "annotation_epoch": "epoch_1",
                    "source": "build", "revision": 1, "text": if clear { None } else { Some("Build: running") },
                    "ttl_ms": 5000, "valid_until_unix_ms": 9_007_199_254_740_991u64
                }})
            );
            if error.is_some() {
                let response: Value = serde_json::from_slice(&output.stderr).unwrap();
                assert_eq!(response["error"]["code"], "annotation_expired");
            } else {
                assert!(output.stderr.is_empty());
            }
        }
    }
}

#[test]
fn pane_annotation_cli_invalid_args_exit_two_without_peer_contact() {
    let fixture = Fixture::new();
    let mut cases = vec![vec!["pane", "annotate"]];
    for index in (3..15).step_by(2) {
        let mut missing = annotation_args();
        missing.drain(index..index + 2);
        cases.push(missing);
        let mut duplicate = annotation_args();
        duplicate.extend_from_within(index..index + 2);
        cases.push(duplicate);
    }
    for (index, invalid) in [
        (2, "1-1"),
        (2, "agent:build"),
        (4, ""),
        (6, ""),
        (8, "bad:source"),
        (10, "-1"),
        (10, "18446744073709551616"),
        (12, "0"),
        (12, "300001"),
        (14, ""),
    ] {
        let mut args = annotation_args();
        args[index] = invalid;
        cases.push(args);
    }
    for extra in ["--clear", "--unknown", "--current"] {
        let mut args = annotation_args();
        args.push(extra);
        cases.push(args);
    }
    for invalid in [
        "",
        "-1",
        "+1",
        "1.0",
        "1.5",
        "null",
        "true",
        "9007199254740992",
        "18446744073709551616",
    ] {
        let mut args = annotation_args();
        args.extend(["--valid-until-unix-ms", invalid]);
        cases.push(args);
    }
    let mut missing = annotation_args();
    missing.push("--valid-until-unix-ms");
    cases.push(missing);
    let mut duplicate = annotation_args();
    duplicate.extend(["--valid-until-unix-ms", "1", "--valid-until-unix-ms", "2"]);
    cases.push(duplicate);
    for args in cases {
        let (output, requests) = fixture.run(&args, false);
        assert!(
            requests.is_empty(),
            "invalid arguments contacted peer: {args:?}: {requests:?}"
        );
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn pane_annotation_cli_embedded_schema_has_required_nullable_text_and_optional_descriptor() {
    let fixture = Fixture::new();
    let (output, requests) = fixture.run(&["api", "schema", "--json"], false);
    assert!(requests.is_empty());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let schema: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(schema["protocol"], 18);
    let request = &schema["schemas"]["request"];
    assert!(request["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .any(|variant| variant["properties"]["method"]["const"] == "pane.annotate"));
    let params = &request["$defs"]["PaneAnnotateParams"];
    assert_eq!(params["additionalProperties"], false);
    for field in [
        "pane_id",
        "expected_terminal_id",
        "annotation_epoch",
        "source",
        "revision",
        "text",
        "ttl_ms",
    ] {
        assert!(
            params["required"]
                .as_array()
                .unwrap()
                .contains(&json!(field)),
            "missing {field}"
        );
    }
    assert_eq!(
        params["properties"]["text"]["type"],
        json!(["string", "null"])
    );
    assert!(!params["required"]
        .as_array()
        .unwrap()
        .contains(&json!("valid_until_unix_ms")));
    let deadline = &params["properties"]["valid_until_unix_ms"];
    assert_eq!(deadline["type"], "integer");
    assert_eq!(deadline["minimum"], 0);
    assert_eq!(deadline["maximum"], 9_007_199_254_740_991u64);
    let definitions = &schema["schemas"]["success_response"]["$defs"];
    let snapshot = &definitions["SessionSnapshot"];
    assert!(snapshot["properties"].get("annotations").is_some());
    assert!(!snapshot["required"]
        .as_array()
        .unwrap()
        .contains(&json!("annotations")));
    let descriptor = &definitions["AnnotationInfo"];
    assert_eq!(descriptor["properties"]["epoch"]["type"], "string");
    assert_eq!(descriptor["properties"]["sources"]["type"], "array");
    assert_eq!(
        descriptor["properties"]["sources"]["items"]["type"],
        "string"
    );
    assert_eq!(descriptor["required"], json!(["epoch", "sources"]));
    assert_eq!(
        descriptor["properties"]["valid_until_unix_ms"]["type"],
        "boolean"
    );
}

#[test]
fn pane_annotation_cli_help_and_completion_include_explicit_fences() {
    let fixture = Fixture::new();
    for args in [
        vec!["pane", "annotate", "--help"],
        vec!["completion", "bash"],
        vec!["completion", "elvish"],
        vec!["completion", "fish"],
        vec!["completion", "powershell"],
        vec!["completion", "zsh"],
    ] {
        let (output, requests) = fixture.run(&args, false);
        assert!(requests.is_empty());
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        for expected in [
            "annotate",
            "expected-terminal",
            "annotation-epoch",
            "source",
            "revision",
            "text",
            "clear",
            "ttl-ms",
            "valid-until-unix-ms",
        ] {
            assert!(text.contains(expected), "{args:?}: missing {expected}");
        }
    }
}
