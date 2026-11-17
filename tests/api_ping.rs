#[path = "retirement/api_cases.rs"]
mod retirement_cases;
mod support;

use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use support::{
    cleanup_test_base, register_runtime_dir, register_spawned_kitsune_pid,
    unregister_spawned_kitsune_pid,
};

fn unique_test_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("ha{nanos:x}"))
}

struct SpawnedKitsune {
    _master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Drop for SpawnedKitsune {
    fn drop(&mut self) {
        let pid = self.child.process_id();
        let _ = self.child.kill();

        if let Some(pid) = pid {
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                let mut status = 0;
                let result =
                    unsafe { libc::waitpid(pid as libc::pid_t, &mut status, libc::WNOHANG) };
                if result == pid as libc::pid_t || result == -1 {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }

            unregister_spawned_kitsune_pid(Some(pid));
        }
    }
}

fn cleanup_spawned_kitsune(spawned: SpawnedKitsune, base: PathBuf) {
    drop(spawned);
    cleanup_test_base(&base);
}

fn test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn wait_for_socket(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() && UnixStream::connect(path).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("socket did not appear at {}", path.display());
}

#[cfg(target_os = "linux")]
fn wait_for_path(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("path did not appear at {}", path.display());
}

fn spawn_kitsune(config_home: &Path, runtime_dir: &Path, socket_path: &Path) -> SpawnedKitsune {
    spawn_kitsune_with_options(config_home, runtime_dir, socket_path, None, "/bin/sh")
}

fn spawn_kitsune_with_path(
    config_home: &Path,
    runtime_dir: &Path,
    socket_path: &Path,
    path_override: Option<&Path>,
) -> SpawnedKitsune {
    spawn_kitsune_with_options(
        config_home,
        runtime_dir,
        socket_path,
        path_override,
        "/bin/sh",
    )
}

#[cfg(target_os = "linux")]
fn spawn_kitsune_with_shell(
    config_home: &Path,
    runtime_dir: &Path,
    socket_path: &Path,
    shell: &str,
) -> SpawnedKitsune {
    spawn_kitsune_with_options(config_home, runtime_dir, socket_path, None, shell)
}

fn spawn_kitsune_with_options(
    config_home: &Path,
    runtime_dir: &Path,
    socket_path: &Path,
    path_override: Option<&Path>,
    shell: &str,
) -> SpawnedKitsune {
    spawn_kitsune_with_config(
        config_home,
        runtime_dir,
        socket_path,
        path_override,
        shell,
        "onboarding = false\n",
    )
}

fn spawn_kitsune_with_config(
    config_home: &Path,
    runtime_dir: &Path,
    socket_path: &Path,
    path_override: Option<&Path>,
    shell: &str,
    config: &str,
) -> SpawnedKitsune {
    fs::create_dir_all(config_home.join("kitsune")).unwrap();
    fs::create_dir_all(runtime_dir).unwrap();
    register_runtime_dir(runtime_dir);
    fs::write(config_home.join("kitsune/config.toml"), config).unwrap();

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();

    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_kitsune"));
    cmd.arg("server");
    cmd.env("XDG_CONFIG_HOME", config_home);
    cmd.env("XDG_RUNTIME_DIR", runtime_dir);
    cmd.env("XDG_STATE_HOME", runtime_dir.join("state"));
    cmd.env_remove("KITSUNE_CONFIG_PATH");
    cmd.env("KITSUNE_SOCKET_PATH", socket_path);
    cmd.env_remove("KITSUNE_CLIENT_SOCKET_PATH");
    cmd.env("SHELL", shell);
    cmd.env_remove("KITSUNE_ENV");
    if let Some(path) = path_override {
        cmd.env("PATH", path);
    }

    let child = pair.slave.spawn_command(cmd).unwrap();
    register_spawned_kitsune_pid(child.process_id());

    SpawnedKitsune {
        _master: pair.master,
        child,
    }
}

struct JsonLineReader {
    stream: UnixStream,
    buf: Vec<u8>,
}

impl JsonLineReader {
    fn connect(socket_path: &Path) -> Self {
        Self {
            stream: UnixStream::connect(socket_path).unwrap(),
            buf: Vec::new(),
        }
    }

    fn send_line(&mut self, json: &str) {
        self.stream.write_all(json.as_bytes()).unwrap();
        self.stream.write_all(b"\n").unwrap();
        self.stream.flush().unwrap();
    }

    fn read_json_line(&mut self, timeout: Duration) -> serde_json::Value {
        self.try_read_json_line(timeout)
            .unwrap_or_else(|| panic!("timed out waiting for json line"))
    }

    fn try_read_json_line(&mut self, timeout: Duration) -> Option<serde_json::Value> {
        let deadline = Instant::now() + timeout;
        self.stream.set_nonblocking(true).unwrap();

        loop {
            if Instant::now() >= deadline {
                self.stream.set_nonblocking(false).unwrap();
                return None;
            }

            if let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                let line = String::from_utf8(self.buf.drain(..=pos).collect()).unwrap();
                self.stream.set_nonblocking(false).unwrap();
                return Some(serde_json::from_str(&line).unwrap());
            }

            let mut bytes = [0u8; 256];
            match self.stream.read(&mut bytes) {
                Ok(0) => panic!("stream closed while waiting for json line"),
                Ok(n) => self.buf.extend_from_slice(&bytes[..n]),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("failed to read json line: {err}"),
            }
        }
    }
}

fn send_request(socket_path: &Path, json: &str) -> serde_json::Value {
    let mut reader = JsonLineReader::connect(socket_path);
    reader.send_line(json);
    reader.read_json_line(Duration::from_secs(5))
}

fn open_subscription(socket_path: &Path, json: &str) -> JsonLineReader {
    let mut reader = JsonLineReader::connect(socket_path);
    reader.send_line(json);
    reader
}

#[cfg(not(target_os = "macos"))]
fn wait_for_event(
    reader: &mut JsonLineReader,
    expected: &str,
    timeout: Duration,
) -> serde_json::Value {
    wait_for_event_matching(reader, expected, timeout, |_| true)
}

#[cfg(not(target_os = "macos"))]
fn wait_for_event_matching<F>(
    reader: &mut JsonLineReader,
    expected: &str,
    timeout: Duration,
    mut matches: F,
) -> serde_json::Value
where
    F: FnMut(&serde_json::Value) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let value = reader.read_json_line(remaining.max(Duration::from_millis(1)));
        if value["event"] == expected && matches(&value) {
            return value;
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn wait_for_events(
    reader: &mut JsonLineReader,
    expected: &[&str],
    timeout: Duration,
) -> Vec<serde_json::Value> {
    let deadline = Instant::now() + timeout;
    let mut remaining = expected.to_vec();
    let mut events = Vec::new();
    while !remaining.is_empty() {
        let remaining_timeout = deadline.saturating_duration_since(Instant::now());
        let value = reader.read_json_line(remaining_timeout.max(Duration::from_millis(1)));
        let Some(event) = value["event"].as_str() else {
            continue;
        };
        if let Some(index) = remaining.iter().position(|expected| *expected == event) {
            remaining.remove(index);
            events.push(value);
        }
    }
    events
}

#[cfg(not(target_os = "macos"))]
fn event_by_kind<'a>(events: &'a [serde_json::Value], kind: &str) -> &'a serde_json::Value {
    events
        .iter()
        .find(|event| event["event"] == kind)
        .unwrap_or_else(|| panic!("missing event {kind}"))
}

#[test]
fn ping_over_socket_returns_version() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");

    let child = spawn_kitsune(&config_home, &runtime_dir, &socket_path);
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let value = send_request(
        &socket_path,
        r#"{"id":"req_1","method":"ping","params":{}}"#,
    );
    assert_eq!(value["id"], "req_1");
    assert_eq!(value["result"]["type"], "pong");
    assert_eq!(value["result"]["version"], env!("CARGO_PKG_VERSION"));
    // Intentionally hardcoded so wire protocol bumps require updating this test.
    // Changing this value means old clients/servers are no longer compatible.
    assert_eq!(value["result"]["protocol"], 18);

    cleanup_spawned_kitsune(child, base);
}

#[test]
fn server_manifest_management_is_retired_and_ignores_existing_files() {
    let _lock = test_lock();
    for external in [false, true] {
        let base = unique_test_dir();
        let config_home = base.join("config");
        let runtime_dir = base.join("runtime");
        let socket_path = runtime_dir.join("kitsune.sock");

        let ownership = if external {
            "[integrations]\nsources = ['custom']\n[integrations.runtime]\nsource = 'custom'\ncommand = ['/not-installed']\nreport_command = ['/not-installed-report']\nexternal_agent_management = true\n"
        } else {
            "[integrations]\nsources = []\n"
        };
        let child = spawn_kitsune_with_config(
            &config_home,
            &runtime_dir,
            &socket_path,
            None,
            "/bin/sh",
            &format!("onboarding = false\n{ownership}"),
        );
        wait_for_socket(&socket_path, Duration::from_secs(5));

        let override_dir = config_home.join("kitsune-dev").join("agent-detection");
        fs::create_dir_all(&override_dir).unwrap();
        let override_path = override_dir.join("codex.toml");
        fs::write(
            &override_path,
            r#"
id = "codex"

[[rules]]
id = "reload_marker"
state = "blocked"
contains = ["server-reload-marker"]
"#,
        )
        .unwrap();

        let root = runtime_dir.join("state/kitsune-dev/agent-detection");
        fs::create_dir_all(root.join("remote")).unwrap();
        let remote = root.join("remote/codex.toml");
        let status = root.join("status.toml");
        fs::write(&remote, "id = 'codex'\nversion = '9999.1'\nmin_engine_version = 1\n[[rules]]\nid = 'malicious'\nstate = 'blocked'\ncontains = ['malicious']\n").unwrap();
        fs::write(
            &status,
            "last_check_unix = 123\nlast_result = 'malicious'\n",
        )
        .unwrap();
        let before: Vec<_> = [&override_path, &remote, &status]
            .map(|path| fs::read(path).unwrap())
            .into();
        for value in ["true", "false", "'wrong type'"] {
            let config = format!("onboarding = false\n{ownership}[agent_detection]\nmanifest_check = {value}\n[update]\nmanifest_check = true\n[terminal]\ndefault_shell = '/bin/sh'\n");
            fs::write(config_home.join("kitsune/config.toml"), &config).unwrap();
            let reload = send_request(
                &socket_path,
                r#"{"id":"config","method":"server.reload_config","params":{}}"#,
            );
            assert_eq!(reload["id"], "config");
            assert!(reload["result"]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d.as_str().is_some_and(|s| s.contains("retired"))));
            for method in ["server.agent_manifests", "server.reload_agent_manifests"] {
                let response = send_request(
                    &socket_path,
                    &format!(r#"{{"id":"retired","method":"{method}","params":{{}}}}"#),
                );
                assert_eq!(response["id"], "retired");
                assert_eq!(response["error"]["code"], "feature_retired");
                assert_eq!(response["error"]["message"], "Built-in detector-manifest management has been retired; Kitsune no longer downloads or reloads agent-detection manifests. Generic pane/process operations remain available.");
                assert!(response.get("result").is_none());
            }
            assert_eq!(
                fs::read_to_string(config_home.join("kitsune/config.toml")).unwrap(),
                config
            );
            for (path, content) in [&override_path, &remote, &status].into_iter().zip(&before) {
                assert_eq!(fs::read(path).unwrap(), *content);
            }
        }

        cleanup_spawned_kitsune(child, base);
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn workspace_list_and_create_round_trip() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");

    let child = spawn_kitsune(&config_home, &runtime_dir, &socket_path);
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let empty = send_request(
        &socket_path,
        r#"{"id":"req_2","method":"workspace.list","params":{}}"#,
    );
    assert_eq!(empty["id"], "req_2");
    assert_eq!(empty["result"]["type"], "workspace_list");
    assert_eq!(empty["result"]["workspaces"].as_array().unwrap().len(), 0);

    let created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_3","method":"workspace.create","params":{{"cwd":"{}","focus":true}}}}"#,
            base.display()
        ),
    );
    assert_eq!(created["id"], "req_3");
    assert_eq!(created["result"]["type"], "workspace_created");
    let workspace_id = created["result"]["workspace"]["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();
    let active_tab_id = created["result"]["workspace"]["active_tab_id"]
        .as_str()
        .unwrap()
        .to_string();
    let root_pane_id = created["result"]["root_pane"]["pane_id"]
        .as_str()
        .unwrap()
        .to_string();
    let root_terminal_id = created["result"]["root_pane"]["terminal_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(root_terminal_id.starts_with("term_"));
    assert_ne!(root_terminal_id, root_pane_id);
    assert_eq!(created["result"]["workspace"]["number"], 1);
    assert_eq!(created["result"]["workspace"]["focused"], true);
    assert_eq!(created["result"]["workspace"]["tab_count"], 1);
    assert_eq!(created["result"]["tab"]["tab_id"], active_tab_id);
    assert_eq!(created["result"]["root_pane"]["tab_id"], active_tab_id);
    assert_eq!(active_tab_id, format!("{workspace_id}:t1"));

    let listed = send_request(
        &socket_path,
        r#"{"id":"req_4","method":"workspace.list","params":{}}"#,
    );
    let workspaces = listed["result"]["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0]["workspace_id"], workspace_id);

    let fetched = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_5","method":"workspace.get","params":{{"workspace_id":"{}"}}}}"#,
            workspace_id
        ),
    );
    assert_eq!(fetched["result"]["workspace"]["workspace_id"], workspace_id);

    let metadata = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_workspace_metadata","method":"workspace.report_metadata","params":{{"workspace_id":"{}","source":"user:test","tokens":{{"jj_status":"2 changes"}}}}}}"#,
            workspace_id
        ),
    );
    assert_eq!(metadata["result"]["type"], "ok");
    let fetched = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_workspace_metadata_get","method":"workspace.get","params":{{"workspace_id":"{}"}}}}"#,
            workspace_id
        ),
    );
    assert_eq!(
        fetched["result"]["workspace"]["tokens"]["jj_status"],
        "2 changes"
    );

    let panes = send_request(
        &socket_path,
        r#"{"id":"req_6","method":"pane.list","params":{}}"#,
    );
    let panes = panes["result"]["panes"].as_array().unwrap();
    assert_eq!(panes.len(), 1);
    assert_eq!(panes[0]["workspace_id"], workspace_id);
    assert_eq!(panes[0]["tab_id"], active_tab_id);
    let pane_id = panes[0]["pane_id"].as_str().unwrap().to_string();
    assert_eq!(pane_id, root_pane_id);
    assert_eq!(panes[0]["terminal_id"], root_terminal_id);
    let legacy_pane_id = format!("{workspace_id}-1");

    let pane = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_7","method":"pane.get","params":{{"pane_id":"{}"}}}}"#,
            legacy_pane_id
        ),
    );
    assert_eq!(pane["result"]["pane"]["pane_id"], pane_id);
    assert_eq!(pane["result"]["pane"]["terminal_id"], root_terminal_id);

    let read = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_8","method":"pane.read","params":{{"pane_id":"{}","source":"visible"}}}}"#,
            legacy_pane_id
        ),
    );
    assert_eq!(read["result"]["read"]["pane_id"], pane_id);
    assert_eq!(read["result"]["read"]["tab_id"], active_tab_id);
    assert!(read["result"]["read"]["text"].is_string());

    let send_text = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_9","method":"pane.send_text","params":{{"pane_id":"{}","text":"echo alpha; echo beta; echo gamma"}}}}"#,
            pane_id
        ),
    );
    assert_eq!(send_text["result"]["type"], "ok");

    let send_enter = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_10","method":"pane.send_keys","params":{{"pane_id":"{}","keys":["Enter"]}}}}"#,
            pane_id
        ),
    );
    assert_eq!(send_enter["result"]["type"], "ok");

    std::thread::sleep(Duration::from_millis(300));

    let recent = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_11","method":"pane.read","params":{{"pane_id":"{}","source":"recent","lines":20}}}}"#,
            pane_id
        ),
    );
    let recent_text = recent["result"]["read"]["text"].as_str().unwrap();
    assert!(recent_text.contains("beta") || recent_text.contains("gamma"));

    for source in ["visible", "detection"] {
        let limited = send_request(
            &socket_path,
            &format!(
                r#"{{"id":"req_11_{source}","method":"pane.read","params":{{"pane_id":"{}","source":"{source}","lines":2}}}}"#,
                pane_id
            ),
        );
        let text = limited["result"]["read"]["text"].as_str().unwrap();
        assert!(
            text.lines().count() <= 2,
            "{source} ignored its two-line limit: {text:?}"
        );
    }

    let waited = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_12","method":"pane.wait_for_output","params":{{"pane_id":"{}","source":"recent","lines":40,"match":{{"type":"substring","value":"gamma"}},"timeout_ms":2000}}}}"#,
            legacy_pane_id
        ),
    );
    assert_eq!(waited["result"]["type"], "output_matched");
    assert_eq!(waited["result"]["pane_id"], pane_id);
    assert_eq!(waited["result"]["read"]["pane_id"], pane_id);
    assert!(waited["result"]["matched_line"]
        .as_str()
        .unwrap()
        .contains("gamma"));
    assert!(waited["result"]["read"]["text"]
        .as_str()
        .unwrap()
        .contains("gamma"));

    let send_input = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_12b","method":"pane.send_input","params":{{"pane_id":"{}","text":"echo delta","keys":["Enter"]}}}}"#,
            pane_id
        ),
    );
    assert_eq!(send_input["result"]["type"], "ok");

    let waited_delta = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_12c","method":"pane.wait_for_output","params":{{"pane_id":"{}","source":"recent","lines":40,"match":{{"type":"substring","value":"delta"}},"timeout_ms":2000}}}}"#,
            pane_id
        ),
    );
    assert_eq!(waited_delta["result"]["type"], "output_matched");
    assert_eq!(waited_delta["result"]["pane_id"], pane_id);
    assert_eq!(waited_delta["result"]["read"]["pane_id"], pane_id);
    assert!(waited_delta["result"]["matched_line"]
        .as_str()
        .unwrap()
        .contains("delta"));

    let waited_regex = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_13","method":"pane.wait_for_output","params":{{"pane_id":"{}","source":"recent","lines":40,"match":{{"type":"regex","value":"alp.*gamma"}},"timeout_ms":2000}}}}"#,
            pane_id
        ),
    );
    assert_eq!(waited_regex["result"]["type"], "output_matched");
    assert_eq!(waited_regex["result"]["pane_id"], pane_id);
    assert_eq!(waited_regex["result"]["read"]["pane_id"], pane_id);
    assert!(waited_regex["result"]["matched_line"]
        .as_str()
        .unwrap()
        .contains("alpha"));

    let timeout = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_14","method":"pane.wait_for_output","params":{{"pane_id":"{}","source":"recent","lines":10,"match":{{"type":"substring","value":"definitely-not-there"}},"timeout_ms":200}}}}"#,
            pane_id
        ),
    );
    assert_eq!(timeout["error"]["code"], "timeout");

    cleanup_spawned_kitsune(child, base);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn tab_methods_round_trip_over_socket() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");

    let child = spawn_kitsune(&config_home, &runtime_dir, &socket_path);
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_t1","method":"workspace.create","params":{{"cwd":"{}","focus":true}}}}"#,
            base.display()
        ),
    );
    let workspace_id = created["result"]["workspace"]["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();
    let first_tab_id = created["result"]["workspace"]["active_tab_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(first_tab_id, format!("{workspace_id}:t1"));

    let tab_created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_t2","method":"tab.create","params":{{"workspace_id":"{}","focus":true}}}}"#,
            workspace_id
        ),
    );
    assert_eq!(tab_created["result"]["type"], "tab_created");
    let second_tab_id = tab_created["result"]["tab"]["tab_id"]
        .as_str()
        .unwrap()
        .to_string();
    let second_root_pane_id = tab_created["result"]["root_pane"]["pane_id"]
        .as_str()
        .unwrap()
        .to_string();
    let second_root_terminal_id = tab_created["result"]["root_pane"]["terminal_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(second_root_terminal_id.starts_with("term_"));
    assert_ne!(second_root_terminal_id, second_root_pane_id);
    assert_eq!(second_tab_id, format!("{workspace_id}:t2"));
    assert_eq!(tab_created["result"]["tab"]["focused"], true);
    assert_eq!(tab_created["result"]["root_pane"]["tab_id"], second_tab_id);

    let tab_list = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_t3","method":"tab.list","params":{{"workspace_id":"{}"}}}}"#,
            workspace_id
        ),
    );
    let tabs = tab_list["result"]["tabs"].as_array().unwrap();
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0]["tab_id"], first_tab_id);

    let panes = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_t3b","method":"pane.list","params":{{"workspace_id":"{}"}}}}"#,
            workspace_id
        ),
    );
    let panes = panes["result"]["panes"].as_array().unwrap();
    assert!(panes.iter().any(|pane| {
        pane["pane_id"] == second_root_pane_id && pane["terminal_id"] == second_root_terminal_id
    }));
    assert_eq!(tabs[1]["tab_id"], second_tab_id);

    let tab_get = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_t4","method":"tab.get","params":{{"tab_id":"{}"}}}}"#,
            second_tab_id
        ),
    );
    assert_eq!(tab_get["result"]["tab"]["tab_id"], second_tab_id);

    let renamed = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_t5","method":"tab.rename","params":{{"tab_id":"{}","label":"logs"}}}}"#,
            second_tab_id
        ),
    );
    assert_eq!(renamed["result"]["tab"]["label"], "logs");

    let focused = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_t6","method":"tab.focus","params":{{"tab_id":"{}"}}}}"#,
            first_tab_id
        ),
    );
    assert_eq!(focused["result"]["tab"]["tab_id"], first_tab_id);
    assert_eq!(focused["result"]["tab"]["focused"], true);

    let closed = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_t7","method":"tab.close","params":{{"tab_id":"{}"}}}}"#,
            second_tab_id
        ),
    );
    assert_eq!(closed["result"]["type"], "ok");

    cleanup_spawned_kitsune(child, base);
}

#[cfg(target_os = "linux")]
#[test]
fn pane_info_reports_foreground_cwd_without_changing_pane_cwd() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let foreground = base.join("foreground-process");
    let marker = base.join("foreground-ready");
    let pid_file = base.join("foreground.pid");
    fs::create_dir_all(&foreground).unwrap();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");

    let child = spawn_kitsune_with_shell(&config_home, &runtime_dir, &socket_path, "/bin/bash");
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"fg_ws","method":"workspace.create","params":{{"cwd":"{}","focus":true}}}}"#,
            base.display()
        ),
    );
    let pane_id = created["result"]["root_pane"]["pane_id"]
        .as_str()
        .unwrap()
        .to_string();
    let command = format!(
        "/bin/sh -c 'cd {} && printf %s $$ > {} && touch {} && sleep 30; :'",
        foreground.display(),
        pid_file.display(),
        marker.display()
    );
    let send_text = send_request(
        &socket_path,
        &serde_json::json!({
            "id": "fg_send",
            "method": "pane.send_text",
            "params": {
                "pane_id": pane_id,
                "text": command,
            },
        })
        .to_string(),
    );
    assert_eq!(send_text["result"]["type"], "ok");
    let send_enter = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"fg_enter","method":"pane.send_keys","params":{{"pane_id":"{}","keys":["Enter"]}}}}"#,
            pane_id
        ),
    );
    assert_eq!(send_enter["result"]["type"], "ok");
    wait_for_path(&marker, Duration::from_secs(5));

    let foreground_pid: u32 = fs::read_to_string(&pid_file).unwrap().parse().unwrap();
    assert_eq!(
        fs::read_link(format!("/proc/{foreground_pid}/cwd")).unwrap(),
        foreground
    );

    let pane = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"fg_pane","method":"pane.get","params":{{"pane_id":"{}"}}}}"#,
            pane_id
        ),
    );
    assert_eq!(pane["result"]["pane"]["cwd"], base.display().to_string());
    assert_eq!(
        pane["result"]["pane"]["foreground_cwd"],
        foreground.display().to_string()
    );

    let panes = send_request(
        &socket_path,
        r#"{"id":"fg_panes","method":"pane.list","params":{}}"#,
    );
    assert_eq!(
        panes["result"]["panes"][0]["cwd"],
        base.display().to_string()
    );
    assert_eq!(
        panes["result"]["panes"][0]["foreground_cwd"],
        foreground.display().to_string()
    );

    let process_info = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"fg_process_info","method":"pane.process_info","params":{{"pane_id":"{}"}}}}"#,
            pane_id
        ),
    );
    let process_info = &process_info["result"]["process_info"];
    assert!(process_info["shell_pid"].is_number());
    assert_eq!(process_info["foreground_process_group_id"], foreground_pid);
    assert!(process_info.get("tty").is_none());
    let foreground_processes = process_info["foreground_processes"].as_array().unwrap();
    let foreground_shell = foreground_processes
        .iter()
        .find(|process| process["pid"] == foreground_pid)
        .expect("foreground shell should be reported");
    assert_eq!(foreground_shell["name"], "sh");
    assert_eq!(foreground_shell["cwd"], foreground.display().to_string());
    assert!(foreground_shell.get("argv0").is_none());
    assert!(foreground_shell["argv"].is_array());
    assert!(foreground_shell["cmdline"].is_string());
    let foreground_sleep = foreground_processes
        .iter()
        .find(|process| process["name"] == "sleep" && process["pid"] != foreground_pid)
        .expect("foreground sleep child should be reported separately");
    assert_eq!(foreground_sleep["cwd"], foreground.display().to_string());

    let reported = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"fg_report","method":"pane.report_agent","params":{{"pane_id":"{}","source":"test","agent":"probe","state":"working"}}}}"#,
            pane_id
        ),
    );
    assert_eq!(reported["error"]["code"], "feature_retired");

    let agents = send_request(
        &socket_path,
        r#"{"id":"fg_agents","method":"agent.list","params":{}}"#,
    );
    assert_eq!(agents["error"]["code"], "feature_retired");

    let split = send_request(
        &socket_path,
        &serde_json::json!({
            "id": "fg_split",
            "method": "pane.split",
            "params": {
                "target_pane_id": pane_id,
                "direction": "right",
                "focus": false,
            },
        })
        .to_string(),
    );
    assert_eq!(
        split["result"]["pane"]["cwd"],
        foreground.display().to_string()
    );

    cleanup_spawned_kitsune(child, base);
}

#[cfg(target_os = "linux")]
#[test]
fn new_terminal_cwd_follow_ignores_nonleader_group_member_cwd() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let helper_cwd = base.join("plugin-cache");
    let helper_marker = base.join("helper-cwd-ready");
    let marker = base.join("helper-ready");
    let leader_pid_file = base.join("leader.pid");
    let helper_pid_file = base.join("helper.pid");
    fs::create_dir_all(&helper_cwd).unwrap();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");

    let child = spawn_kitsune_with_shell(&config_home, &runtime_dir, &socket_path, "/bin/bash");
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"member_ws","method":"workspace.create","params":{{"cwd":"{}","focus":true}}}}"#,
            base.display()
        ),
    );
    let pane_id = created["result"]["root_pane"]["pane_id"]
        .as_str()
        .unwrap()
        .to_string();
    let workspace_id = created["result"]["workspace"]["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();
    let command = format!(
        "/bin/sh -c 'printf %s $$ > {}; (cd {} && touch {} && sleep 30) & printf %s $! > {}; while [ ! -e {} ]; do sleep 0.01; done; touch {}; wait'",
        leader_pid_file.display(),
        helper_cwd.display(),
        helper_marker.display(),
        helper_pid_file.display(),
        helper_marker.display(),
        marker.display()
    );
    let send_text = send_request(
        &socket_path,
        &serde_json::json!({
            "id": "member_send",
            "method": "pane.send_text",
            "params": {
                "pane_id": pane_id,
                "text": command,
            },
        })
        .to_string(),
    );
    assert_eq!(send_text["result"]["type"], "ok");
    let send_enter = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"member_enter","method":"pane.send_keys","params":{{"pane_id":"{}","keys":["Enter"]}}}}"#,
            pane_id
        ),
    );
    assert_eq!(send_enter["result"]["type"], "ok");
    wait_for_path(&marker, Duration::from_secs(5));

    let leader_pid: u32 = fs::read_to_string(&leader_pid_file)
        .unwrap()
        .parse()
        .unwrap();
    let helper_pid: u32 = fs::read_to_string(&helper_pid_file)
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        unsafe { libc::getpgid(leader_pid as libc::pid_t) },
        leader_pid as libc::pid_t
    );
    assert_eq!(
        unsafe { libc::getpgid(helper_pid as libc::pid_t) },
        leader_pid as libc::pid_t
    );
    assert_eq!(
        fs::read_link(format!("/proc/{leader_pid}/cwd")).unwrap(),
        base
    );
    assert_eq!(
        fs::read_link(format!("/proc/{helper_pid}/cwd")).unwrap(),
        helper_cwd
    );

    let pane = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"member_pane","method":"pane.get","params":{{"pane_id":"{}"}}}}"#,
            pane_id
        ),
    );
    assert_eq!(pane["result"]["pane"]["cwd"], base.display().to_string());
    assert_eq!(
        pane["result"]["pane"]["foreground_cwd"],
        helper_cwd.display().to_string()
    );

    let split = send_request(
        &socket_path,
        &serde_json::json!({
            "id": "member_split",
            "method": "pane.split",
            "params": {
                "target_pane_id": pane_id,
                "direction": "right",
                "focus": false,
            },
        })
        .to_string(),
    );
    assert_eq!(split["result"]["pane"]["cwd"], base.display().to_string());

    let tab = send_request(
        &socket_path,
        &serde_json::json!({
            "id": "member_tab",
            "method": "tab.create",
            "params": {
                "workspace_id": workspace_id,
                "focus": false,
            },
        })
        .to_string(),
    );
    assert_eq!(
        tab["result"]["root_pane"]["cwd"],
        base.display().to_string()
    );

    cleanup_spawned_kitsune(child, base);
}

#[test]
fn tab_create_with_no_focus_preserves_active_tab() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");

    let child = spawn_kitsune(&config_home, &runtime_dir, &socket_path);
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_nf_1","method":"workspace.create","params":{{"cwd":"{}","focus":true}}}}"#,
            base.display()
        ),
    );
    let workspace_id = created["result"]["workspace"]["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();
    let first_tab_id = created["result"]["tab"]["tab_id"]
        .as_str()
        .unwrap()
        .to_string();

    let tab_created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_nf_2","method":"tab.create","params":{{"workspace_id":"{}","focus":false}}}}"#,
            workspace_id
        ),
    );
    assert_eq!(tab_created["result"]["type"], "tab_created");
    let second_tab_id = tab_created["result"]["tab"]["tab_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(second_tab_id, format!("{workspace_id}:t2"));
    assert_eq!(tab_created["result"]["tab"]["focused"], false);

    let tab_list = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_nf_3","method":"tab.list","params":{{"workspace_id":"{}"}}}}"#,
            workspace_id
        ),
    );
    let tabs = tab_list["result"]["tabs"].as_array().unwrap();
    assert_eq!(tabs[0]["tab_id"], first_tab_id);
    assert_eq!(tabs[0]["focused"], true);
    assert_eq!(tabs[1]["tab_id"], second_tab_id);
    assert_eq!(tabs[1]["focused"], false);

    cleanup_spawned_kitsune(child, base);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn events_subscribe_streams_workspace_tab_and_agent_events() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");
    let bin_dir = base.join("bin");

    fs::create_dir_all(&bin_dir).unwrap();
    let fake_pi = bin_dir.join("pi");
    fs::write(
        &fake_pi,
        "#!/bin/sh\nprintf 'Working...\\n'\nsleep 1\nprintf '\\033[2J\\033[Hdone\\n'\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&fake_pi).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_pi, perms).unwrap();
    }

    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let path_override = format!("{}:{}", bin_dir.display(), inherited_path);
    let child = spawn_kitsune_with_path(
        &config_home,
        &runtime_dir,
        &socket_path,
        Some(Path::new(&path_override)),
    );
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let mut reader = open_subscription(
        &socket_path,
        r#"{"id":"sub_life_a","method":"events.subscribe","params":{"subscriptions":[{"type":"workspace.created"},{"type":"workspace.focused"},{"type":"tab.created"},{"type":"tab.focused"},{"type":"tab.renamed"},{"type":"pane.created"},{"type":"pane.focused"},{"type":"pane.agent_detected"}]}}"#,
    );

    let ack = reader.read_json_line(Duration::from_secs(2));
    assert_eq!(ack["id"], "sub_life_a");
    assert_eq!(ack["result"]["type"], "subscription_started");

    let created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_l1","method":"workspace.create","params":{{"cwd":"{}","focus":true}}}}"#,
            base.display()
        ),
    );
    let workspace_id = created["result"]["workspace"]["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    let initial_events = wait_for_events(
        &mut reader,
        &[
            "workspace_created",
            "workspace_focused",
            "tab_created",
            "tab_focused",
            "pane_created",
            "pane_focused",
        ],
        Duration::from_secs(2),
    );

    let workspace_created = event_by_kind(&initial_events, "workspace_created");
    assert_eq!(
        workspace_created["data"]["workspace"]["workspace_id"],
        workspace_id
    );
    let workspace_focused = event_by_kind(&initial_events, "workspace_focused");
    assert_eq!(workspace_focused["data"]["workspace_id"], workspace_id);

    let first_tab_id = format!("{workspace_id}:t1");
    let tab_created = event_by_kind(&initial_events, "tab_created");
    assert_eq!(tab_created["data"]["tab"]["tab_id"], first_tab_id);
    let tab_focused = event_by_kind(&initial_events, "tab_focused");
    assert_eq!(tab_focused["data"]["tab_id"], first_tab_id);

    let pane_created = event_by_kind(&initial_events, "pane_created");
    let pane_id = pane_created["data"]["pane"]["pane_id"]
        .as_str()
        .unwrap()
        .to_string();
    let pane_focused = event_by_kind(&initial_events, "pane_focused");
    assert_eq!(pane_focused["data"]["pane_id"], pane_id);

    let send_pi = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_l2","method":"pane.send_text","params":{{"pane_id":"{}","text":"pi"}}}}"#,
            pane_id
        ),
    );
    assert_eq!(send_pi["result"]["type"], "ok");
    let send_enter = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_l3","method":"pane.send_keys","params":{{"pane_id":"{}","keys":["Enter"]}}}}"#,
            pane_id
        ),
    );
    assert_eq!(send_enter["result"]["type"], "ok");

    assert!(reader
        .try_read_json_line(Duration::from_millis(100))
        .is_none());

    let new_tab = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_l4","method":"tab.create","params":{{"workspace_id":"{}","focus":true}}}}"#,
            workspace_id
        ),
    );
    let second_tab_id = new_tab["result"]["tab"]["tab_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(second_tab_id, format!("{workspace_id}:t2"));

    let created_tab_event = wait_for_event(&mut reader, "tab_created", Duration::from_secs(2));
    assert_eq!(created_tab_event["data"]["tab"]["tab_id"], second_tab_id);
    let focused_tab_event = wait_for_event(&mut reader, "tab_focused", Duration::from_secs(2));
    assert_eq!(focused_tab_event["data"]["tab_id"], second_tab_id);

    let renamed_tab = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_l5","method":"tab.rename","params":{{"tab_id":"{}","label":"logs"}}}}"#,
            second_tab_id
        ),
    );
    assert_eq!(renamed_tab["result"]["tab"]["label"], "logs");
    let renamed_event = wait_for_event(&mut reader, "tab_renamed", Duration::from_secs(2));
    assert_eq!(renamed_event["data"]["tab_id"], second_tab_id);
    assert_eq!(renamed_event["data"]["label"], "logs");

    cleanup_spawned_kitsune(child, base);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn events_subscribe_streams_pane_split_and_close_events() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");

    let child = spawn_kitsune(&config_home, &runtime_dir, &socket_path);
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_pc_1","method":"workspace.create","params":{{"cwd":"{}","focus":true}}}}"#,
            base.display()
        ),
    );
    let pane_id = created["result"]["root_pane"]["pane_id"]
        .as_str()
        .unwrap()
        .to_string();
    let workspace_id = created["result"]["workspace"]["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut reader = open_subscription(
        &socket_path,
        r#"{"id":"sub_life_b","method":"events.subscribe","params":{"subscriptions":[{"type":"pane.created"},{"type":"pane.closed"}]}}"#,
    );

    let ack = reader.read_json_line(Duration::from_secs(2));
    assert_eq!(ack["id"], "sub_life_b");
    assert_eq!(ack["result"]["type"], "subscription_started");

    let split = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_pc_2","method":"pane.split","params":{{"target_pane_id":"{}","direction":"right","focus":true}}}}"#,
            pane_id
        ),
    );
    let split_pane_id = split["result"]["pane"]["pane_id"]
        .as_str()
        .unwrap()
        .to_string();

    let split_created = wait_for_event_matching(
        &mut reader,
        "pane_created",
        Duration::from_secs(2),
        |value| value["data"]["pane"]["pane_id"] == split_pane_id,
    );
    assert_eq!(split_created["data"]["pane"]["pane_id"], split_pane_id);

    let closed = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_pc_3","method":"pane.close","params":{{"pane_id":"{}-2"}}}}"#,
            workspace_id
        ),
    );
    assert_eq!(closed["result"]["type"], "ok");

    let pane_closed = wait_for_event_matching(
        &mut reader,
        "pane_closed",
        Duration::from_secs(2),
        |value| value["data"]["pane_id"] == split_pane_id,
    );
    assert_eq!(pane_closed["data"]["pane_id"], split_pane_id);

    cleanup_spawned_kitsune(child, base);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn events_subscribe_streams_tab_and_workspace_close_events() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");

    let child = spawn_kitsune(&config_home, &runtime_dir, &socket_path);
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_tc_1","method":"workspace.create","params":{{"cwd":"{}","focus":true}}}}"#,
            base.display()
        ),
    );
    let workspace_id = created["result"]["workspace"]["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    let new_tab = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_tc_2","method":"tab.create","params":{{"workspace_id":"{}","focus":true}}}}"#,
            workspace_id
        ),
    );
    let second_tab_id = new_tab["result"]["tab"]["tab_id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut reader = open_subscription(
        &socket_path,
        r#"{"id":"sub_life_c","method":"events.subscribe","params":{"subscriptions":[{"type":"workspace.renamed"},{"type":"tab.closed"},{"type":"workspace.closed"}]}}"#,
    );

    let ack = reader.read_json_line(Duration::from_secs(2));
    assert_eq!(ack["id"], "sub_life_c");
    assert_eq!(ack["result"]["type"], "subscription_started");

    let closed_tab = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_tc_3","method":"tab.close","params":{{"tab_id":"{}:2"}}}}"#,
            workspace_id
        ),
    );
    assert_eq!(closed_tab["result"]["type"], "ok");

    let tab_closed = wait_for_event(&mut reader, "tab_closed", Duration::from_secs(2));
    assert_eq!(tab_closed["data"]["tab_id"], second_tab_id);

    let renamed_ws = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_tc_4","method":"workspace.rename","params":{{"workspace_id":"{}","label":"renamed"}}}}"#,
            workspace_id
        ),
    );
    assert_eq!(renamed_ws["result"]["workspace"]["label"], "renamed");

    let workspace_renamed =
        wait_for_event(&mut reader, "workspace_renamed", Duration::from_secs(2));
    assert_eq!(workspace_renamed["data"]["workspace_id"], workspace_id);
    assert_eq!(workspace_renamed["data"]["label"], "renamed");

    let closed_ws = send_request(
        &socket_path,
        r#"{"id":"req_tc_5","method":"workspace.close","params":{"workspace_id":"1"}}"#,
    );
    assert_eq!(closed_ws["result"]["type"], "ok");

    let workspace_closed = wait_for_event(&mut reader, "workspace_closed", Duration::from_secs(2));
    assert_eq!(workspace_closed["data"]["workspace_id"], workspace_id);

    cleanup_spawned_kitsune(child, base);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn events_subscribe_rejects_retired_status_atomically_and_streams_generic_output() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");
    let child = spawn_kitsune(&config_home, &runtime_dir, &socket_path);
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let created = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_20","method":"workspace.create","params":{{"cwd":"{}","focus":true}}}}"#,
            base.display()
        ),
    );
    // A startup workspace may already exist; use the created pane's actual ID
    // for input and both subscriptions instead of guessing a legacy locator.
    let pane_id = created["result"]["root_pane"]["pane_id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut reader = open_subscription(
        &socket_path,
        &format!(
            r#"{{"id":"sub_1","method":"events.subscribe","params":{{"subscriptions":[{{"type":"pane.output_matched","pane_id":"{}","source":"recent","lines":40,"match":{{"type":"substring","value":"hello from socket"}}}},{{"type":"pane.agent_status_changed","pane_id":"{}","agent_status":"done"}}]}}}}"#,
            pane_id, pane_id,
        ),
    );

    let ack = reader.read_json_line(Duration::from_secs(2));
    assert_eq!(ack["id"], "sub_1");
    assert_eq!(ack["error"]["code"], "feature_retired");
    assert!(ack.get("result").is_none());
    drop(reader);

    let mut reader = open_subscription(
        &socket_path,
        &serde_json::json!({
            "id":"generic-output", "method":"events.subscribe",
            "params":{"subscriptions":[{
                "type":"pane.output_matched", "pane_id":pane_id,
                "source":"recent", "lines":40,
                "match":{"type":"substring", "value":"hello from socket"}
            }]}
        })
        .to_string(),
    );
    let ack = reader.read_json_line(Duration::from_secs(2));
    assert_eq!(ack["id"], "generic-output");
    assert_eq!(ack["result"]["type"], "subscription_started");

    let send_text = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_22","method":"pane.send_text","params":{{"pane_id":"{}","text":"echo hello from socket"}}}}"#,
            pane_id
        ),
    );
    assert_eq!(send_text["result"]["type"], "ok");
    let send_enter = send_request(
        &socket_path,
        &format!(
            r#"{{"id":"req_23","method":"pane.send_keys","params":{{"pane_id":"{}","keys":["Enter"]}}}}"#,
            pane_id
        ),
    );
    assert_eq!(send_enter["result"]["type"], "ok");

    let output_event = reader.read_json_line(Duration::from_secs(3));
    assert_eq!(output_event["event"], "pane.output_matched");
    assert_eq!(output_event["data"]["pane_id"], pane_id);
    assert_eq!(output_event["data"]["read"]["pane_id"], pane_id);
    assert!(output_event["data"]["matched_line"]
        .as_str()
        .unwrap()
        .contains("hello from socket"));
    assert!(output_event["data"]["read"]["text"]
        .as_str()
        .unwrap()
        .contains("hello from socket"));

    assert!(reader
        .try_read_json_line(Duration::from_millis(100))
        .is_none());
    cleanup_spawned_kitsune(child, base);
}

#[test]
fn command_submission_real_shell_preserves_argv_and_rejects_busy_and_mismatched_targets() {
    let _lock = test_lock();
    let base = unique_test_dir();
    let config_home = base.join("config");
    let runtime_dir = base.join("runtime");
    let socket_path = runtime_dir.join("kitsune.sock");
    let child = spawn_kitsune(&config_home, &runtime_dir, &socket_path);
    wait_for_socket(&socket_path, Duration::from_secs(5));
    let created = send_request(
        &socket_path,
        &serde_json::json!({
            "id": "command-workspace", "method": "workspace.create",
            "params": {"cwd": base, "focus": false}
        })
        .to_string(),
    );
    let pane = &created["result"]["root_pane"];
    let pane_id = pane["pane_id"].as_str().unwrap();
    let terminal_id = pane["terminal_id"].as_str().unwrap();
    let snapshot = send_request(
        &socket_path,
        r#"{"id":"command-capability","method":"session.snapshot","params":{}}"#,
    );
    assert_eq!(
        snapshot["result"]["snapshot"]["command_submission"]["version"],
        1
    );

    // Observe shell setup without retrying a mutating command or guessing readiness.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let observed = send_request(
            &socket_path,
            &serde_json::json!({
                "id": "command-shell", "method": "pane.process_info", "params": {"pane_id": pane_id}
            })
            .to_string(),
        );
        let process = &observed["result"]["process_info"];
        if process["foreground_processes"]
            .as_array()
            .is_some_and(|rows| {
                rows.len() == 1
                    && rows[0]["pid"] == process["shell_pid"]
                    && process["foreground_process_group_id"] == process["shell_pid"]
            })
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "shell never became observable: {observed}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let executable = base.join("harmless executable with spaces");
    let arguments = base.join("received-arguments");
    let stop = base.join("stop-fixture");
    let injected = base.join("must-not-exist");
    fs::write(
        &executable,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nwhile [ ! -f '{}' ]; do sleep 0.05; done\n",
            arguments.display(),
            stop.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let argv = vec![
        executable.to_string_lossy().into_owned(),
        "a b".into(),
        String::new(),
        format!("$(touch {})", injected.display()),
        "semi; ' quote".into(),
        "λ".into(),
    ];
    let params =
        serde_json::json!({"pane_id": pane_id, "expected_terminal_id": terminal_id, "argv": argv});
    let submitted = send_request(
        &socket_path,
        &serde_json::json!({
            "id": "command-submit", "method": "pane.submit_command", "params": params
        })
        .to_string(),
    );
    assert_eq!(
        submitted["result"]["type"], "pane_command_submitted",
        "{submitted}"
    );
    assert_eq!(submitted["result"]["terminal_id"], terminal_id);
    assert_eq!(submitted["result"]["argv"], params["argv"]);
    let deadline = Instant::now() + Duration::from_secs(5);
    let expected = format!("{}\n", argv[1..].join("\n"));
    while fs::read_to_string(&arguments).ok().as_deref() != Some(expected.as_str()) {
        assert!(
            Instant::now() < deadline,
            "harmless fixture did not receive exact argv"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !injected.exists(),
        "shell interpreted an argument instead of quoting it"
    );
    let busy = send_request(
        &socket_path,
        &serde_json::json!({
            "id": "command-busy", "method": "pane.submit_command", "params": params
        })
        .to_string(),
    );
    assert_eq!(busy["error"]["code"], "pane_command_busy", "{busy}");
    let mut wrong = params;
    wrong["expected_terminal_id"] = serde_json::json!("not-the-terminal");
    let mismatch = send_request(
        &socket_path,
        &serde_json::json!({
            "id": "command-wrong", "method": "pane.submit_command", "params": wrong
        })
        .to_string(),
    );
    assert_eq!(mismatch["error"]["code"], "terminal_mismatch");
    fs::write(stop, "stop").unwrap();
    cleanup_spawned_kitsune(child, base);
}
