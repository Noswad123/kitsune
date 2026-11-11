//! Explicit developer integration test; never connects to a running Kitsune.
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, PermissionsExt},
        net::UnixListener,
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use serde_json::{json, Value};

use crate::{
    api::schema,
    app::{test_support::app_from_state, App, AppState},
    ui,
    workspace::Workspace,
};

struct OwnedRoot(PathBuf);

impl OwnedRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("kitsune-coven-{}-{nonce}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}

impl Drop for OwnedRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Output files avoid pipe backpressure and reader threads that could outlive a panic.
struct CovenChild {
    child: Child,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl CovenChild {
    fn spawn(binary: &Path, root: &Path, tag: &str, args: &[&std::ffi::OsStr]) -> Self {
        let stdout = root.join(format!("{tag}.stdout"));
        let stderr = root.join(format!("{tag}.stderr"));
        let child = Command::new(binary)
            .args(args)
            .current_dir(root)
            .env_clear()
            .env("HOME", root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        Self {
            child,
            stdout,
            stderr,
        }
    }

    fn finish(&mut self, deadline: Instant) -> String {
        let status: ExitStatus = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "Coven child exceeded deadline");
            thread::sleep(Duration::from_millis(2));
        };
        let stdout = fs::read_to_string(&self.stdout).unwrap();
        let stderr = fs::read_to_string(&self.stderr).unwrap();
        assert!(
            status.success(),
            "Coven {status}: stdout={stdout} stderr={stderr}"
        );
        stdout
    }
}

impl Drop for CovenChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            if kind.is_dir() {
                visit(root, &entry.path(), result);
            } else {
                result.insert(
                    entry.path().strip_prefix(root).unwrap().into(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

fn draw(state: &mut AppState, terminal: &mut Terminal<TestBackend>) -> Vec<String> {
    ui::compute_view(state, Rect::new(0, 0, 120, 50));
    terminal.draw(|frame| ui::render(state, frame)).unwrap();
    state.commit_integration_views_render();
    let area = state.integration_views_area();
    let buffer = terminal.backend().buffer();
    (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

fn api(app: &mut App, request: Value) -> Value {
    let response = app.handle_api_request(serde_json::from_value(request.clone()).unwrap());
    let response: Value = serde_json::from_str(&response).unwrap();
    assert!(response.get("error").is_none(), "{request}: {response}");
    assert_eq!(response["id"], request["id"]);
    response["result"].clone()
}

fn assert_build_unchanged(app: &mut App, owner: &Value, item: &schema::IntegrationViewItem) {
    let projection = app
        .state
        .integration_views
        .project("build", Instant::now())
        .unwrap();
    assert_eq!(projection.epoch, owner["epoch"].as_str().unwrap());
    assert_eq!(projection.revision, 7);
    assert_eq!(projection.items, std::slice::from_ref(item));
    // A successful poll with the original credentials also proves the lease survived.
    let poll = api(
        app,
        json!({"id":"build-poll", "method":"integration.view.poll", "params":owner}),
    );
    assert_eq!(poll["type"], "view_intents");
    assert_eq!(poll["epoch"], owner["epoch"]);
    assert_eq!(poll["revision"], 7);
    assert_eq!(
        poll["intents"],
        json!([]),
        "Coven selection leaked into build"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicit COVEN_TEST_BIN"]
fn integration_views_real_coven_roundtrip() {
    let binary = PathBuf::from(
        std::env::var_os("COVEN_TEST_BIN")
            .expect("explicit COVEN_TEST_BIN is required; no PATH or sibling binary fallback"),
    );
    assert!(binary.is_absolute() && binary.is_file());
    let root = OwnedRoot::new();
    assert_eq!(fs::metadata(&root.0).unwrap().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(&root.0).unwrap().uid(), unsafe {
        libc::geteuid()
    });
    let workspace = root.0.join("workspace");
    assert!(!workspace.exists());
    CovenChild::spawn(
        &binary,
        &root.0,
        "init",
        &["init".as_ref(), workspace.as_os_str(), "--blank".as_ref()],
    )
    .finish(Instant::now() + Duration::from_secs(2));
    CovenChild::spawn(
        &binary,
        &root.0,
        "create",
        &[
            "group".as_ref(),
            "create".as_ref(),
            workspace.as_os_str(),
            "team".as_ref(),
            "--name".as_ref(),
            "Review Team".as_ref(),
            "--json".as_ref(),
        ],
    )
    .finish(Instant::now() + Duration::from_secs(2));

    // Data-only equivalent of Coven tests/group_presenter_workflow.rs::workspace:
    // retain CLI-created workspace/group IDs and add its v1 participant record.
    let store_path = workspace.join("state/groups.json");
    let mut store: Value = serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
    assert_eq!(store["schema_version"], 1);
    store["content_revision"] = json!(store["content_revision"].as_u64().unwrap() + 1);
    store["groups"][0]["members"] = json!([{
        "participant_id":"00000000-0000-4000-8000-000000000001",
        "key":"reviewer", "role":"Reviewer",
        "binding":{
            "presentation":{"adapter":"kitsune","endpoint":{"path":"/not/used","device":0,"inode":0},
                "terminal_id":"not-used","pane_id_at_binding":"not-used"},
            "evidence":{"provenance":"kitsune.session.snapshot","observed_at":"2024-W01T12Z",
                "agent_kind_hint":null,"native_reference":null,"native_binding":"unestablished"}
        }
    }]);
    fs::write(&store_path, serde_json::to_vec(&store).unwrap()).unwrap();
    let participant_item_id = format!(
        "p:{}",
        store["groups"][0]["members"][0]["participant_id"]
            .as_str()
            .unwrap()
    );
    let before_files = files(&workspace);

    let mut state = AppState::test_new();
    let mut native = Workspace::test_new("native-workspace");
    native.identity_cwd = root.0.clone();
    state.workspaces = vec![native];
    state.ensure_test_terminals();
    for terminal in state.terminals.values_mut() {
        terminal.cwd = root.0.clone();
        assert!(terminal.detected_agent().is_none());
        assert!(terminal.hook_authority.is_none());
        assert_eq!(terminal.state, crate::detect::AgentState::Unknown);
    }
    state.sidebar_width = 36;
    state
        .integration_views
        .reconfigure(vec!["build".into(), "coven.test".into()])
        .unwrap();
    let mut app = app_from_state(state);
    let build_open = api(
        &mut app,
        json!({
            "id":"build-open", "method":"integration.view.open", "params":{"source":"build","version":1}
        }),
    );
    assert_eq!(build_open["type"], "view_opened");
    let build_owner =
        json!({"source":"build","lease":build_open["lease"],"epoch":build_open["epoch"]});
    let build_item = schema::IntegrationViewItem {
        id: participant_item_id.clone(),
        label: "Build running".into(),
        detail: "Independent build provider".into(),
    };
    let build_replace = api(
        &mut app,
        json!({
            "id":"build-replace", "method":"integration.view.replace", "params":{
                "source":"build","lease":build_owner["lease"],"epoch":build_owner["epoch"],
                "revision":7,"items":[build_item]
            }
        }),
    );
    assert_eq!(build_replace["type"], "view_replaced");
    assert_build_unchanged(&mut app, &build_owner, &build_item);
    let native_id = app.state.workspaces[0].id.clone();
    let native_name = app.state.workspaces[0].custom_name.clone();
    let before_terminals: BTreeMap<_, _> = app
        .state
        .terminals
        .iter()
        .map(|(id, t)| {
            (
                id.to_string(),
                (
                    t.cwd.clone(),
                    t.manual_label.clone(),
                    t.terminal_title.clone(),
                    t.agent_name.clone(),
                    t.revision,
                ),
            )
        })
        .collect();
    assert_eq!(app.terminal_runtimes.len(), 0);

    let socket = root.0.join("view.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(fs::metadata(&socket).unwrap().mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(&socket).unwrap().uid(), unsafe {
        libc::geteuid()
    });
    listener.set_nonblocking(true).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(120, 50)).unwrap();
    let initial = draw(&mut app.state, &mut terminal);
    assert_eq!(
        app.state.integration_view_selected.as_deref(),
        Some("build")
    );
    assert!(initial[0].contains("Integrations"), "{initial:?}");
    assert!(initial[1].contains("> build: Available"), "{initial:?}");
    assert!(
        initial[2].contains("coven.test: Unavailable"),
        "{initial:?}"
    );
    assert!(
        initial.iter().any(|line| line.contains("Build running")),
        "{initial:?}"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut child = CovenChild::spawn(
        &binary,
        &root.0,
        "present",
        &[
            "group".as_ref(),
            "present".as_ref(),
            workspace.as_os_str(),
            "team".as_ref(),
            "--socket".as_ref(),
            socket.as_os_str(),
            "--source".as_ref(),
            "coven.test".as_ref(),
            "--duration".as_ref(),
            "1".as_ref(),
            "--json".as_ref(),
        ],
    );
    let mut methods = Vec::new();
    let mut frames = 0;
    let mut selected = false;
    let mut polled_selection = false;
    loop {
        assert!(
            Instant::now() < deadline,
            "private socket roundtrip exceeded two seconds: {methods:?}"
        );
        let (mut stream, _) = match listener.accept() {
            Ok(pair) => pair,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if child.child.try_wait().unwrap().is_some() {
                    break;
                }
                thread::sleep(Duration::from_millis(2));
                continue;
            }
            Err(e) => panic!("private accept: {e}"),
        };
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1));
        stream.set_read_timeout(Some(remaining)).unwrap();
        stream.set_write_timeout(Some(remaining)).unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .take(131_073)
            .read_line(&mut line)
            .unwrap();
        assert!(line.ends_with('\n') && line.len() <= 131_072);
        let wire: Value = serde_json::from_str(&line).unwrap();
        let method = wire["method"].as_str().unwrap();
        assert!(
            matches!(
                method,
                "session.snapshot"
                    | "integration.view.open"
                    | "integration.view.replace"
                    | "integration.view.poll"
                    | "integration.view.close"
            ),
            "execution or unexpected API forbidden: {method}"
        );
        let request: schema::Request = serde_json::from_str(&line).unwrap();
        let response = app.handle_api_request(request);
        let result: Value = serde_json::from_str(&response).unwrap();
        assert!(result.get("error").is_none(), "{method}: {result}");
        assert_eq!(result["id"], wire["id"]);
        methods.push(method.to_owned());
        if method == "session.snapshot" {
            let descriptor = &result["result"]["snapshot"]["integration_views"];
            assert_eq!(descriptor["version"], 1);
            assert_eq!(
                descriptor["sources"],
                json!([{"id":"build","epoch":build_owner["epoch"]},{"id":"coven.test","epoch":null}])
            );
            assert_eq!(
                app.state.integration_view_selected.as_deref(),
                Some("build")
            );
        }
        if method != "session.snapshot" {
            assert_eq!(wire["params"]["source"], "coven.test");
        }
        if method == "integration.view.poll"
            && !result["result"]["intents"].as_array().unwrap().is_empty()
        {
            assert!(selected && !polled_selection);
            assert_eq!(result["result"]["intents"].as_array().unwrap().len(), 1);
            assert_eq!(
                result["result"]["intents"][0]["item_id"],
                participant_item_id
            );
            polled_selection = true;
        }
        if method == "integration.view.replace" {
            frames += 1;
            let items = wire["params"]["items"].as_array().unwrap();
            let index = items
                .iter()
                .position(|item| item["label"] == "reviewer")
                .unwrap();
            assert_eq!(index, 1, "group header precedes participant");
            assert_eq!(items[index]["id"], participant_item_id);
            if frames == 1 {
                // Opening the second provider must not steal the selected source.
                let lines = draw(&mut app.state, &mut terminal);
                assert_eq!(
                    app.state.integration_view_selected.as_deref(),
                    Some("build")
                );
                assert!(lines[0].contains("Integrations"), "{lines:?}");
                assert!(lines[1].contains("> build: Available"), "{lines:?}");
                assert!(lines[2].contains("coven.test: Available"), "{lines:?}");
                assert!(
                    lines.iter().any(|line| line.contains("Build running")),
                    "{lines:?}"
                );
                let source_index = app
                    .state
                    .integration_views
                    .configured_sources()
                    .iter()
                    .position(|source| source == "coven.test")
                    .unwrap();
                let area = app.state.integration_views_area();
                assert!(app.state.activate_integration_view_at(
                    area.x,
                    area.y + 1 + source_index as u16,
                    Instant::now()
                ));
            }
            let lines = draw(&mut app.state, &mut terminal);
            assert_eq!(
                app.state.integration_view_selected.as_deref(),
                Some("coven.test")
            );
            assert!(
                lines[0].contains("Integrations")
                    && lines[1].contains("build: Available")
                    && lines[2].contains("> coven.test: Available"),
                "{lines:?}"
            );
            assert!(
                lines.iter().any(|line| line.contains("Review Team")),
                "{lines:?}"
            );
            assert!(
                lines.iter().any(|line| line.contains("reviewer")),
                "{lines:?}"
            );
            if frames == 1 {
                assert!(!lines.iter().any(|line| line.contains("Selected")));
                let area = app.state.integration_views_area();
                let selector_height = app
                    .state
                    .integration_views
                    .configured_sources()
                    .len()
                    .min(usize::from(area.height.saturating_sub(2)))
                    .min(8);
                let y = area.y + 2 + selector_height as u16 + 2 * index as u16;
                assert!(lines[usize::from(y - area.y)].contains("reviewer"));
                assert!(app
                    .state
                    .activate_integration_view_at(area.x, y, Instant::now()));
                selected = true;
                assert_build_unchanged(&mut app, &build_owner, &build_item);
            } else {
                assert!(polled_selection);
                assert!(items[index]["detail"]
                    .as_str()
                    .unwrap()
                    .starts_with("Selected (read-only): Reviewer"));
                assert!(
                    lines
                        .iter()
                        .any(|line| line.contains("Selected (read-only): Reviewer")),
                    "{lines:?}"
                );
            }
        }
        stream.write_all(response.trim_end().as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
        if method == "integration.view.close" {
            break;
        }
    }
    let report: Value = serde_json::from_str(&child.finish(deadline)).unwrap();
    assert_eq!(report["outcome"], "completed", "{report}");
    assert_eq!(report["cleanup"], "closed", "{report}");
    assert_eq!(report["selections"], 1, "{report}");
    assert!(report["frames"].as_u64().unwrap() >= 2, "{report}");
    assert_eq!(report["frames"].as_u64().unwrap(), frames);
    assert!(selected && polled_selection);
    assert_eq!(
        methods.first().map(String::as_str),
        Some("session.snapshot")
    );
    assert_eq!(
        methods.last().map(String::as_str),
        Some("integration.view.close")
    );
    assert!(app
        .state
        .integration_views
        .project("coven.test", Instant::now())
        .is_none());
    assert_build_unchanged(&mut app, &build_owner, &build_item);
    let closed = draw(&mut app.state, &mut terminal);
    assert!(closed[0].contains("Integrations"), "{closed:?}");
    assert!(closed[1].contains("build: Available"), "{closed:?}");
    assert!(
        closed[2].contains("> coven.test: Unavailable"),
        "{closed:?}"
    );
    assert_eq!(app.state.workspaces.len(), 1);
    assert_eq!(app.state.workspaces[0].id, native_id);
    assert_eq!(app.state.workspaces[0].custom_name, native_name);
    assert_eq!(app.state.workspaces[0].identity_cwd, root.0);
    assert_eq!(app.terminal_runtimes.len(), 0);
    let after_terminals: BTreeMap<_, _> = app
        .state
        .terminals
        .iter()
        .map(|(id, t)| {
            assert!(t.detected_agent().is_none() && t.hook_authority.is_none());
            assert!(t.agent_metadata.is_empty() && t.persisted_agent_session.is_none());
            assert_eq!(t.state, crate::detect::AgentState::Unknown);
            (
                id.to_string(),
                (
                    t.cwd.clone(),
                    t.manual_label.clone(),
                    t.terminal_title.clone(),
                    t.agent_name.clone(),
                    t.revision,
                ),
            )
        })
        .collect();
    assert_eq!(before_terminals, after_terminals);
    assert_eq!(files(&workspace), before_files);
}
