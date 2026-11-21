//! Fresh legacy installer tombstones. No real provider, user root or server is used.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("ki-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "failed to clean installer fixture {}: {error}",
                self.0.display()
            );
            assert!(std::thread::panicking(), "fixture cleanup failed");
        }
    }
}

fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(base: &Path, path: &Path, result: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let bytes = if path.is_file() {
                fs::read(&path).unwrap()
            } else {
                Vec::new()
            };
            result.push((path.strip_prefix(base).unwrap().to_owned(), bytes));
            if path.is_dir() {
                visit(base, &path, result);
            }
        }
    }
    let mut result = Vec::new();
    visit(root, root, &mut result);
    result.sort();
    result
}

#[test]
fn cli_all_six_installs_retire_without_api_protocol_provider_or_file_side_effects() {
    let fixture = Fixture::new();
    let root = &fixture.0;
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    let marker = root.join("native-command-ran");
    for name in [
        "pi", "claude", "codex", "copilot", "opencode", "djinn", "coven",
    ] {
        let path = bin.join(name);
        fs::write(
            &path,
            format!("#!/bin/sh\n: > '{}'\nexit 99\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let api_socket = root.join("a.sock");
    let protocol_socket = root.join("p.sock");
    let api = UnixListener::bind(&api_socket).unwrap();
    let protocol = UnixListener::bind(&protocol_socket).unwrap();
    api.set_nonblocking(true).unwrap();
    protocol.set_nonblocking(true).unwrap();
    let config_home = root.join("config");
    fs::create_dir_all(config_home.join("kitsune")).unwrap();
    let legacy = root.join("legacy");
    fs::create_dir(&legacy).unwrap();
    for (name, contents) in [
        ("settings.json", "invalid JSON; preserve exactly\n"),
        ("hooks.json", "invalid hooks; preserve exactly\n"),
        (
            "config.toml",
            "# user trust choice\n[features]\nhooks = false\n",
        ),
        ("kitsune-agent-state.sh", "frozen user preimage\n"),
    ] {
        fs::write(legacy.join(name), contents).unwrap();
    }
    let invalid_home = root.join("not-a-directory");
    fs::write(&invalid_home, "not a home\n").unwrap();
    let mut expected_error = None;
    for external in [false, true] {
        fs::write(config_home.join("kitsune/config.toml"), format!(
            "[integrations]\nsources=[]\n[integrations.runtime]\nsource='custom'\ncommand=['/not-installed']\nreport_command=['/not-installed']\nexternal_agent_management={external}\n"
        )).unwrap();
        let before = tree(root);
        for target in ["pi", "claude", "codex", "copilot", "opencode", "djinn"] {
            let output = Command::new(env!("CARGO_BIN_EXE_kitsune"))
                .args(["integration", "install", target])
                .env("HOME", &invalid_home)
                .env("PATH", &bin)
                .env("XDG_CONFIG_HOME", &config_home)
                .env("KITSUNE_SOCKET_PATH", &api_socket)
                .env("KITSUNE_CLIENT_SOCKET_PATH", &protocol_socket)
                .env("PI_CODING_AGENT_DIR", &legacy)
                .env("CLAUDE_CONFIG_DIR", &legacy)
                .env("CODEX_HOME", &legacy)
                .env("COPILOT_HOME", &legacy)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{target}: {output:?}");
            assert!(output.stdout.is_empty(), "{target}: {output:?}");
            let error = String::from_utf8(output.stderr).unwrap();
            assert!(
                error.starts_with("feature_retired: integration.install:"),
                "{target}: {error}"
            );
            if let Some(expected) = &expected_error {
                assert_eq!(&error, expected);
            } else {
                expected_error = Some(error);
            }
            assert_eq!(tree(root), before, "{target}: file mutation");
            assert!(!marker.exists());
            for listener in [&api, &protocol] {
                assert!(
                    matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
                    "{target}: unexpected connection"
                );
            }
        }
    }
}

#[test]
fn cli_invalid_install_syntax_keeps_usage_exit_two() {
    let fixture = Fixture::new();
    for args in [
        vec!["integration", "install"],
        vec!["integration", "install", "unknown"],
        vec!["integration", "install", "pi", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_kitsune"))
            .args(&args)
            .env("HOME", &fixture.0)
            .env("XDG_CONFIG_HOME", &fixture.0)
            .env("PATH", "")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("feature_retired"));
        assert!(tree(&fixture.0).is_empty());
    }
}
