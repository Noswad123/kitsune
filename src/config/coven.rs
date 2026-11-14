//! Startup-only capability negotiation. No installation or runtime health checks.
use super::IntegrationRuntimeConfig;
use serde::Deserialize;
use std::{
    io::Read,
    path::{Component, Path},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Doctor {
    contract: String,
    version: String,
    executable: String,
    capabilities: Capabilities,
    defaults: Defaults,
    diagnostics: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capabilities {
    integration_runtime: Versions,
    native_event_reporting: Versions,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Versions {
    versions: Vec<u32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Defaults {
    integration_state_root: Option<String>,
}

pub(super) fn resolve() -> Result<IntegrationRuntimeConfig, String> {
    let paths = std::env::var_os("PATH").unwrap_or_default();
    resolve_with_path(&paths, Duration::from_secs(3))
}

fn resolve_with_path(
    paths: &std::ffi::OsStr,
    timeout: Duration,
) -> Result<IntegrationRuntimeConfig, String> {
    crate::platform::prepare_coven_doctor(&mut Command::new("coven"))
        .map_err(|err| err.to_string())?;
    let candidate = std::env::split_paths(paths)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("coven"))
        .find(|path| crate::platform::coven_executable(path, false))
        .ok_or("coven not found on absolute startup PATH; install/update Coven")?;
    let mut command = Command::new(candidate);
    command
        .args(["doctor", "status", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Discovery must not inherit a parent server's identity or reporting route.
    for (key, _) in std::env::vars_os() {
        if key.to_str().is_some_and(|key| key.starts_with("KITSUNE_")) {
            command.env_remove(key);
        }
    }
    let bytes =
        doctor_output(command, timeout).map_err(|err| format!("doctor status failed: {err}"))?;
    parse_doctor(&bytes)
}

fn doctor_output(mut command: Command, timeout: Duration) -> std::io::Result<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    crate::platform::prepare_coven_doctor(&mut command)?;
    let mut child = command.spawn()?;
    let result = (|| {
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("missing doctor stdout"))?;
        crate::platform::nonblocking_coven_stdout(&stdout)?;
        let mut bytes = Vec::new();
        let mut eof = false;
        loop {
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "doctor deadline exceeded",
                ));
            }
            if !eof {
                let mut buffer = [0; 4096];
                match stdout.read(&mut buffer) {
                    Ok(0) => eof = true,
                    Ok(count) => {
                        if bytes.len() + count > 16384 {
                            return Err(std::io::Error::other("doctor stdout exceeds 16384 bytes"));
                        }
                        bytes.extend_from_slice(&buffer[..count]);
                    }
                    Err(err)
                        if matches!(
                            err.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(err) => return Err(err),
                }
            }
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return Err(std::io::Error::other(format!("doctor exited {status}")));
                }
                if eof {
                    return Ok(bytes);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })();
    // Also close descendants holding stdout, without any reader thread to strand.
    crate::platform::kill_coven_doctor_group(child.id());
    let _ = child.kill();
    let waited = child.wait();
    match result {
        Ok(bytes) => {
            waited?;
            Ok(bytes)
        }
        Err(err) => Err(err),
    }
}

fn safe_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
        && Path::new(value).is_absolute()
        && !Path::new(value)
            .components()
            .any(|part| matches!(part, Component::ParentDir))
}

fn parse_doctor(bytes: &[u8]) -> Result<IntegrationRuntimeConfig, String> {
    // Deserialize directly: duplicate contract fields must not disappear in a Value map.
    // serde_json retains its bounded recursion limit, including diagnostics.
    let doctor: Doctor = serde_json::from_slice(bytes)
        .map_err(|err| format!("invalid doctor JSON: {err}; update Coven"))?;
    if doctor.contract != "coven.doctor-status.v1"
        || !doctor
            .capabilities
            .integration_runtime
            .versions
            .contains(&1)
        || !doctor
            .capabilities
            .native_event_reporting
            .versions
            .contains(&1)
    {
        return Err("doctor contract requires integration runtime v1 and native event reporting v1; update Coven".into());
    }
    let _ = (&doctor.version, &doctor.diagnostics);
    let exe = &doctor.executable;
    if !safe_path(exe)
        || !crate::platform::coven_executable(Path::new(exe), true)
        || std::fs::canonicalize(exe)
            .ok()
            .and_then(|path| path.into_os_string().into_string().ok())
            .as_deref()
            != Some(exe.as_str())
    {
        return Err("doctor executable must be a canonical absolute safe native executable (not group/world writable)".into());
    }
    let root = doctor
        .defaults
        .integration_state_root
        .filter(|root| safe_path(root))
        .ok_or(
            "doctor integration_state_root missing, null, or unsafe; configure Coven's state root",
        )?;
    let runtime = IntegrationRuntimeConfig {
        source: "coven".into(),
        command: vec![
            exe.clone(),
            "integration".into(),
            "runtime".into(),
            "--state-root".into(),
            root.clone(),
        ],
        report_command: vec![
            exe.clone(),
            "agent".into(),
            "report-native".into(),
            "--from-kitsune".into(),
            "--state-root".into(),
            root,
        ],
        external_agent_management: true,
    };
    if !runtime.valid_for_sources(&["coven".into()]) {
        return Err("doctor route exceeds argv limits".into());
    }
    Ok(runtime)
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "kitsune-coven-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn script(&self, body: &str) {
            let path = self.0.join("coven");
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        fn resolve(&self) -> Result<IntegrationRuntimeConfig, String> {
            resolve_with_path(self.0.as_os_str(), Duration::from_millis(200))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn doctor() -> serde_json::Value {
        serde_json::json!({
            "contract": "coven.doctor-status.v1", "version": "unknown-future-build",
            "executable": std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap(),
            "capabilities": {"integration_runtime": {"versions": [1]}, "native_event_reporting": {"versions": [1]}},
            "defaults": {"integration_state_root": "/state root/with spaces"}, "diagnostics": []
        })
    }

    #[test]
    fn coven_doctor_wrapper_freezes_native_and_literal_root() {
        let fixture = Fixture::new();
        let json = doctor().to_string();
        std::fs::write(fixture.0.join("status.json"), &json).unwrap();
        // Arcana-shaped dispatcher: the public entrypoint is not the native identity.
        fixture.script(
            "[ \"$*\" = 'doctor status --json' ] || exit 2\nexec /bin/cat \"${0%/*}/status.json\"",
        );
        let runtime = fixture.resolve().unwrap();
        assert_eq!(
            runtime.command,
            [
                doctor()["executable"].as_str().unwrap(),
                "integration",
                "runtime",
                "--state-root",
                "/state root/with spaces"
            ]
        );
        assert_eq!(
            runtime.report_command,
            [
                doctor()["executable"].as_str().unwrap(),
                "agent",
                "report-native",
                "--from-kitsune",
                "--state-root",
                "/state root/with spaces"
            ]
        );
        assert!(runtime.external_agent_management);
        assert_ne!(
            runtime.command[0],
            fixture.0.join("coven").to_str().unwrap()
        );
    }

    #[test]
    fn coven_doctor_requires_contract_both_v1_capabilities_and_root() {
        for pointer in [
            "/contract",
            "/capabilities/integration_runtime/versions",
            "/capabilities/native_event_reporting/versions",
            "/defaults/integration_state_root",
            "/executable",
        ] {
            let mut value = doctor();
            *value.pointer_mut(pointer).unwrap() = match pointer {
                "/contract" => serde_json::json!("coven.doctor-status.v0"),
                "/defaults/integration_state_root" => serde_json::Value::Null,
                "/executable" => serde_json::json!("/not/a/native/executable"),
                _ => serde_json::json!([2]),
            };
            assert!(
                parse_doctor(value.to_string().as_bytes()).is_err(),
                "{pointer}"
            );
        }
        for root in [
            "relative",
            "/state/../escape",
            "/state\nroot",
            "",
            &format!("/{}", "x".repeat(4096)),
        ] {
            let mut value = doctor();
            value["defaults"]["integration_state_root"] = serde_json::json!(root);
            assert!(parse_doctor(value.to_string().as_bytes()).is_err());
        }
        let mut value = doctor();
        value["defaults"]
            .as_object_mut()
            .unwrap()
            .remove("integration_state_root");
        assert!(parse_doctor(value.to_string().as_bytes())
            .unwrap_err()
            .contains("missing"));
    }

    #[test]
    fn coven_doctor_rejects_duplicates_deep_and_malformed_json() {
        let json = doctor().to_string();
        let duplicate = json.replacen('{', "{\"contract\":\"coven.doctor-status.v1\",", 1);
        assert!(parse_doctor(duplicate.as_bytes()).is_err());
        let deep = json.replace(
            "\"diagnostics\":[]",
            &format!("\"diagnostics\":{}0{}", "[".repeat(150), "]".repeat(150)),
        );
        assert!(parse_doctor(deep.as_bytes()).is_err());
        for bytes in [b"old doctor output".as_slice(), b"{}", b"null"] {
            assert!(parse_doctor(bytes).is_err());
        }
    }

    #[test]
    fn coven_doctor_rejects_script_writable_and_noncanonical_native_paths() {
        let fixture = Fixture::new();
        fixture.script("exit 0");
        let mut value = doctor();
        value["executable"] = serde_json::json!(fixture.0.join("coven"));
        assert!(parse_doctor(value.to_string().as_bytes()).is_err());
        let native = fixture.0.join("native");
        std::fs::write(&native, b"\x7fELF").unwrap();
        std::fs::set_permissions(&native, std::fs::Permissions::from_mode(0o775)).unwrap();
        value["executable"] = serde_json::json!(native);
        assert!(parse_doctor(value.to_string().as_bytes()).is_err());
        let link = fixture.0.join("native-link");
        std::os::unix::fs::symlink(std::env::current_exe().unwrap(), &link).unwrap();
        value["executable"] = serde_json::json!(link);
        assert!(parse_doctor(value.to_string().as_bytes()).is_err());
        value["executable"] =
            serde_json::json!(format!("/{}", doctor()["executable"].as_str().unwrap()));
        assert!(parse_doctor(value.to_string().as_bytes()).is_err());
    }

    #[test]
    fn coven_doctor_selects_first_executable_without_fallback() {
        let first = Fixture::new();
        let second = Fixture::new();
        first.script("exit 7");
        std::fs::write(second.0.join("status.json"), doctor().to_string()).unwrap();
        second.script("exec /bin/cat \"${0%/*}/status.json\"");
        let paths = std::env::join_paths([first.0.as_path(), second.0.as_path()]).unwrap();
        assert!(resolve_with_path(&paths, Duration::from_millis(200))
            .unwrap_err()
            .contains("exited"));
        std::fs::set_permissions(
            first.0.join("coven"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(resolve_with_path(&paths, Duration::from_millis(200)).is_ok());
    }

    #[test]
    fn coven_doctor_skips_owner_nonexecutable_candidate() {
        if unsafe { libc::geteuid() } == 0 {
            eprintln!("owner-mode regression requires an ordinary user; skipped as root");
            return;
        }
        let first = Fixture::new();
        let second = Fixture::new();
        first.script("exit 7");
        std::fs::set_permissions(
            first.0.join("coven"),
            std::fs::Permissions::from_mode(0o641),
        )
        .unwrap();
        std::fs::write(second.0.join("status.json"), doctor().to_string()).unwrap();
        second.script("exec /bin/cat \"${0%/*}/status.json\"");
        let paths = std::env::join_paths([first.0.as_path(), second.0.as_path()]).unwrap();
        assert!(!crate::platform::coven_executable(
            &first.0.join("coven"),
            false
        ));
        assert!(resolve_with_path(&paths, Duration::from_millis(200)).is_ok());
    }

    #[test]
    fn coven_doctor_rejects_owner_nonexecutable_reported_native() {
        if unsafe { libc::geteuid() } == 0 {
            eprintln!("owner-mode regression requires an ordinary user; skipped as root");
            return;
        }
        let fixture = Fixture::new();
        let native = fixture.0.join("native");
        let mut magic = [0; 4];
        std::fs::File::open(std::env::current_exe().unwrap())
            .unwrap()
            .read_exact(&mut magic)
            .unwrap();
        std::fs::write(&native, magic).unwrap();
        let mut value = doctor();
        value["executable"] = serde_json::json!(native);
        std::fs::set_permissions(&native, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(parse_doctor(value.to_string().as_bytes()).is_ok());
        std::fs::set_permissions(&native, std::fs::Permissions::from_mode(0o641)).unwrap();
        assert!(parse_doctor(value.to_string().as_bytes())
            .unwrap_err()
            .contains("safe native executable"));
    }

    fn descendant_running(pid: i32) -> bool {
        let output = Command::new("/bin/ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        let state = String::from_utf8(output.stdout).unwrap();
        // kill(pid, 0) also succeeds for dead, unreaped zombies.
        !state.trim().is_empty() && !state.trim().starts_with(['Z', 'X'])
    }

    struct Descendant(i32);
    impl Drop for Descendant {
        fn drop(&mut self) {
            if descendant_running(self.0) {
                unsafe {
                    libc::kill(self.0, libc::SIGKILL);
                }
                let deadline = Instant::now() + Duration::from_secs(2);
                while descendant_running(self.0) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                assert!(
                    !descendant_running(self.0),
                    "fixture descendant {} survived cleanup",
                    self.0
                );
            }
        }
    }

    #[test]
    fn coven_doctor_kills_recorded_descendant_on_timeout_and_output_limit() {
        for (body, error) in [
            ("exit 0", "deadline exceeded"),
            (
                "while :; do printf '0123456789012345678901234567890123456789'; done",
                "exceeds 16384 bytes",
            ),
        ] {
            let fixture = Fixture::new();
            fixture.script(&format!(
                "/bin/sleep 30 &\nprintf '%s\\n' \"$!\" > \"${{0%/*}}/descendant.pid\"\n{body}"
            ));
            let result = fixture.resolve();
            let pid = std::fs::read_to_string(fixture.0.join("descendant.pid"))
                .unwrap()
                .trim()
                .parse::<i32>()
                .unwrap();
            let descendant = Descendant(pid);
            assert!(result.unwrap_err().contains(error));
            let deadline = Instant::now() + Duration::from_secs(2);
            while descendant_running(descendant.0) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(
                !descendant_running(descendant.0),
                "doctor descendant {pid} survived {error}"
            );
        }
    }

    #[test]
    #[ignore = "opt-in real CLI handshake; set KITSUNE_TEST_REAL_COVEN to a built native Coven binary"]
    fn coven_real_cli_smoke() {
        let native = std::fs::canonicalize(
            std::env::var_os("KITSUNE_TEST_REAL_COVEN")
                .expect("set KITSUNE_TEST_REAL_COVEN to the built native Coven binary"),
        )
        .unwrap();
        let fixture = Fixture::new();
        std::fs::write(
            fixture.0.join("native-path"),
            format!("{}\n", native.to_str().unwrap()),
        )
        .unwrap();
        fixture.script("[ \"$*\" = 'doctor status --json' ] || exit 2\nIFS= read -r native < \"${0%/*}/native-path\"\nexec \"$native\" \"$@\"");
        let runtime = resolve_with_path(fixture.0.as_os_str(), Duration::from_secs(3)).unwrap();
        let root = runtime.command[4].as_str();
        assert!(safe_path(root));
        if let Ok(expected) = std::env::var("KITSUNE_TEST_REAL_COVEN_STATE_ROOT") {
            assert_eq!(root, expected);
        }
        assert_eq!(
            runtime.command,
            [
                native.to_str().unwrap(),
                "integration",
                "runtime",
                "--state-root",
                root
            ]
        );
        assert_eq!(
            runtime.report_command,
            [
                native.to_str().unwrap(),
                "agent",
                "report-native",
                "--from-kitsune",
                "--state-root",
                root
            ]
        );
        assert_eq!(runtime.source, "coven");
        assert!(runtime.external_agent_management);
        eprintln!(
            "real Coven doctor accepted: native={}, state_root={root}",
            native.display()
        );
    }

    #[test]
    fn coven_doctor_missing_old_oversize_timeout_and_descendant_pipe() {
        let fixture = Fixture::new();
        assert!(fixture.resolve().unwrap_err().contains("not found"));
        assert!(resolve_with_path(
            std::ffi::OsStr::new(":relative:"),
            Duration::from_millis(20)
        )
        .is_err());
        for body in [
            "exit 1",
            "printf 'old doctor'",
            "while :; do printf '0123456789012345678901234567890123456789'; done",
            "exec /bin/sleep 5",
            "/bin/sleep 5 &\nexit 0",
        ] {
            fixture.script(body);
            let start = Instant::now();
            assert!(fixture.resolve().is_err(), "{body}");
            assert!(start.elapsed() < Duration::from_secs(2), "{body}");
        }
    }

    #[test]
    fn coven_doctor_strips_inherited_routing_environment() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("status.json"), doctor().to_string()).unwrap();
        let key = "KITSUNE_TEST_DOCTOR_PARENT_ROUTE";
        let original = std::env::var_os(key);
        std::env::set_var(key, "parent");
        fixture.script("[ -z \"${KITSUNE_TEST_DOCTOR_PARENT_ROUTE+x}\" ] || exit 3\nexec /bin/cat \"${0%/*}/status.json\"");
        let result = fixture.resolve();
        if let Some(original) = original {
            std::env::set_var(key, original);
        } else {
            std::env::remove_var(key);
        }
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn coven_startup_freezes_once_despite_path_and_config_changes() {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("status.json"), doctor().to_string()).unwrap();
        fixture.script("printf x >> \"${0%/*}/calls\"\nexec /bin/cat \"${0%/*}/status.json\"");
        for mode in ["available", "explicit", "arbitrary", "opted-out", "missing"] {
            if mode == "missing" {
                std::fs::remove_file(fixture.0.join("coven")).unwrap();
            }
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "config::coven::tests::coven_startup_fixture",
                    "--ignored",
                    "--nocapture",
                ])
                .env("KITSUNE_TEST_COVEN_FREEZE", mode)
                .env("PATH", &fixture.0)
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "{mode}");
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("freeze fixture timed out");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        assert_eq!(std::fs::read(fixture.0.join("calls")).unwrap(), b"x");
    }

    #[test]
    #[ignore = "spawned in isolation to exercise the production OnceLock"]
    fn coven_startup_fixture() {
        let mode = std::env::var("KITSUNE_TEST_COVEN_FREEZE").unwrap();
        let content = match mode.as_str() {
            "explicit" => "[integrations]\nsources = ['coven']\n[integrations.coven]\nauto_start = false\n[integrations.runtime]\nsource = 'coven'\ncommand = ['/explicit']\nreport_command = ['/explicit-report']\nexternal_agent_management = true",
            "arbitrary" => "[integrations]\nsources = ['arbitrary']",
            "opted-out" => "[integrations]\nsources = ['coven']\n[integrations.coven]\nauto_start = false",
            _ => "[integrations]\nsources = ['coven']",
        };
        let config: crate::config::Config = toml::from_str(content).unwrap();
        crate::config::freeze_integration_runtime(&config);
        let external = matches!(mode.as_str(), "available" | "missing" | "explicit");
        assert_eq!(crate::config::external_agent_management(), external);
        let runtime = crate::config::integration_runtime().cloned();
        assert_eq!(runtime.is_some(), mode == "available" || mode == "explicit");
        if mode == "explicit" {
            assert_eq!(runtime.as_ref().unwrap().command, ["/explicit"]);
        }
        std::env::set_var("PATH", "/no/discovery/after/startup");
        crate::config::freeze_integration_runtime(&config);
        crate::config::freeze_integration_runtime(&crate::config::Config::default());
        assert_eq!(crate::config::integration_runtime(), runtime.as_ref());
        assert_eq!(crate::config::external_agent_management(), external);
    }
}
