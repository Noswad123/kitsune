//! One attached, non-restarting helper owned by the headless server.
use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::IntegrationRuntimeConfig;

pub(crate) struct IntegrationRuntime {
    child: Child,
}

impl IntegrationRuntime {
    /// Call only after the JSON listener is bound and the App's sources are visible.
    pub(crate) fn spawn(config: &IntegrationRuntimeConfig, socket: &Path) -> io::Result<Self> {
        if !IntegrationRuntimeConfig::valid_argv(&config.command) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid integration runtime argv",
            ));
        }
        let mut command = Command::new(&config.command[0]);
        command
            .args(&config.command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for key in [
            "KITSUNE_INTEGRATION_SOURCE",
            "KITSUNE_INTEGRATION_REPORT_COMMAND",
            "KITSUNE_AGENT_MANAGEMENT",
            "KITSUNE_PANE_ID",
            "KITSUNE_TAB_ID",
            "KITSUNE_WORKSPACE_ID",
            "KITSUNE_CLIENT_SOCKET_PATH",
            crate::product::SESSION_ENV_VAR,
            crate::product::BIN_PATH_ENV_VAR,
            crate::product::RUNTIME_ENV_VAR,
        ] {
            command.env_remove(key);
        }
        // The argv remains literal; explicit socket arguments are never rewritten.
        command.env(crate::api::SOCKET_PATH_ENV_VAR, socket);
        Ok(Self {
            child: command.spawn()?,
        })
    }
}

impl Drop for IntegrationRuntime {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Ok(None) | Err(_) => break,
            }
        }
        if let Err(err) = self.child.kill() {
            tracing::warn!(%err, "failed to kill integration runtime");
        }
        if let Err(err) = self.child.wait() {
            tracing::warn!(%err, "failed to reap integration runtime");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Read, Write};
    use std::path::PathBuf;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "kit-ir-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn config(&self) -> IntegrationRuntimeConfig {
            IntegrationRuntimeConfig {
                source: "fixture".into(),
                command: vec![
                    std::env::current_exe().unwrap().to_str().unwrap().into(),
                    "--exact".into(),
                    "server::integration_runtime::tests::attached_fixture".into(),
                    "--ignored".into(),
                    "--nocapture".into(),
                    "--skip".into(),
                    "$HOME ~/literal ; no shell".into(),
                ],
                report_command: vec!["/unused/reporter".into()],
                external_agent_management: true,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    // Standalone test-binary fixture: no provider, shell, installed app or live config.
    #[test]
    #[ignore = "spawned only by attached child lifecycle tests"]
    fn attached_fixture() {
        let socket = PathBuf::from(std::env::var_os(crate::api::SOCKET_PATH_ENV_VAR).unwrap());
        let mut stream = crate::ipc::connect_local_stream(&socket).unwrap();
        let request = crate::api::schema::Request {
            id: "fixture".into(),
            method: crate::api::schema::Method::ViewOpen(crate::api::schema::ViewOpenParams {
                source: "fixture".into(),
                version: 1,
            }),
        };
        writeln!(stream, "{}", serde_json::to_string(&request).unwrap()).unwrap();
        let mut response = String::new();
        std::io::BufReader::new(stream)
            .read_line(&mut response)
            .unwrap();
        let response: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert!(response.get("result").is_some(), "{response}");
        for key in [
            "KITSUNE_INTEGRATION_SOURCE",
            "KITSUNE_INTEGRATION_REPORT_COMMAND",
            "KITSUNE_AGENT_MANAGEMENT",
            "KITSUNE_PANE_ID",
            "KITSUNE_TAB_ID",
            "KITSUNE_WORKSPACE_ID",
            "KITSUNE_CLIENT_SOCKET_PATH",
            crate::product::SESSION_ENV_VAR,
            crate::product::BIN_PATH_ENV_VAR,
            crate::product::RUNTIME_ENV_VAR,
        ] {
            assert!(std::env::var_os(key).is_none(), "{key}");
        }
        assert!(!crate::config::external_agent_management());
        let mut config = crate::config::Config::default();
        config.integrations.sources = vec!["fixture".into()];
        config.integrations.runtime = Some(IntegrationRuntimeConfig {
            source: "fixture".into(),
            command: vec!["/helper".into()],
            report_command: vec!["/reporter".into()],
            external_agent_management: true,
        });
        crate::config::freeze_integration_runtime(&config);
        assert!(crate::config::external_agent_management());
        crate::config::freeze_integration_runtime(&crate::config::Config::default());
        assert!(crate::config::external_agent_management());
        assert!(std::env::args().any(|arg| arg == "$HOME ~/literal ; no shell"));
        std::fs::write(socket.with_extension("ready"), "ready").unwrap();
        if socket.file_name().unwrap() == "stubborn.sock" {
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input).unwrap();
        assert!(input.is_empty());
        std::fs::write(socket.with_extension("eof"), "eof").unwrap();
    }

    #[test]
    fn attached_child_readiness_literal_env_eof_and_forced_cleanup() {
        let fixture = Fixture::new();
        for name in ["cooperative.sock", "stubborn.sock"] {
            let socket = fixture.0.join(name);
            let listener = crate::ipc::bind_local_listener(&socket).unwrap();
            let responder = std::thread::spawn(move || {
                use interprocess::local_socket::traits::Listener;
                let mut config = crate::config::Config::default();
                config.integrations.sources = vec!["fixture".into()];
                let mut app = crate::app::App::new(
                    &config,
                    true,
                    None,
                    tokio::sync::mpsc::unbounded_channel().1,
                    crate::api::EventHub::default(),
                );
                let mut stream = listener.accept().unwrap();
                let mut request = String::new();
                std::io::BufReader::new(&mut stream)
                    .read_line(&mut request)
                    .unwrap();
                let request: crate::api::schema::Request = serde_json::from_str(&request).unwrap();
                writeln!(stream, "{}", app.handle_api_request(request)).unwrap();
            });
            let mut runtime = IntegrationRuntime::spawn(&fixture.config(), &socket).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !socket.with_extension("ready").exists() {
                assert!(
                    runtime.child.try_wait().unwrap().is_none(),
                    "fixture exited before readiness"
                );
                assert!(Instant::now() < deadline, "fixture readiness timed out");
                std::thread::sleep(Duration::from_millis(10));
            }
            responder.join().unwrap();
            let started = Instant::now();
            drop(runtime);
            assert!(started.elapsed() < Duration::from_secs(3));
            assert_eq!(
                socket.with_extension("eof").exists(),
                name == "cooperative.sock"
            );
        }
    }

    #[test]
    fn attached_child_spawn_failure_and_already_exited_cleanup() {
        let fixture = Fixture::new();
        let mut config = fixture.config();
        config.command = vec![fixture.0.join("missing").to_str().unwrap().into()];
        assert!(IntegrationRuntime::spawn(&config, &fixture.0.join("api.sock")).is_err());
        config = fixture.config();
        config.command = vec![
            std::env::current_exe().unwrap().to_str().unwrap().into(),
            "--list".into(),
        ];
        let mut runtime = IntegrationRuntime::spawn(&config, &fixture.0.join("api.sock")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while runtime.child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(runtime);
    }
}
