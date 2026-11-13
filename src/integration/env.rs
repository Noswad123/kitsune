use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::{Mutex, MutexGuard, OnceLock};

use portable_pty::CommandBuilder;

pub(crate) const KITSUNE_PANE_ID_ENV_VAR: &str = "KITSUNE_PANE_ID";
pub(crate) const KITSUNE_TAB_ID_ENV_VAR: &str = "KITSUNE_TAB_ID";
pub(crate) const KITSUNE_WORKSPACE_ID_ENV_VAR: &str = "KITSUNE_WORKSPACE_ID";

pub(crate) const PI_CODING_AGENT_DIR_ENV_VAR: &str = "PI_CODING_AGENT_DIR";
pub(crate) const CLAUDE_CONFIG_DIR_ENV_VAR: &str = "CLAUDE_CONFIG_DIR";
pub(crate) const CODEX_HOME_ENV_VAR: &str = "CODEX_HOME";
pub(crate) const COPILOT_HOME_ENV_VAR: &str = "COPILOT_HOME";

pub(crate) fn apply_pane_base_env(cmd: &mut CommandBuilder) {
    cmd.env(crate::product::BIN_PATH_ENV_VAR, binary_path_env_value());
    cmd.env(crate::api::SOCKET_PATH_ENV_VAR, crate::api::socket_path());
    apply_runtime_env(
        cmd,
        crate::config::integration_runtime(),
        crate::config::external_agent_management(),
    );
}

fn apply_runtime_env(
    cmd: &mut CommandBuilder,
    runtime: Option<&crate::config::IntegrationRuntimeConfig>,
    external_agent_management: bool,
) {
    // Never let a nested/disabled server inherit its parent's reporting route.
    for key in [
        "KITSUNE_INTEGRATION_SOURCE",
        "KITSUNE_INTEGRATION_REPORT_COMMAND",
        "KITSUNE_AGENT_MANAGEMENT",
    ] {
        cmd.env_remove(key);
    }
    if let Some(runtime) = runtime {
        cmd.env("KITSUNE_INTEGRATION_SOURCE", &runtime.source);
        if let Ok(command) = serde_json::to_string(&runtime.report_command) {
            cmd.env("KITSUNE_INTEGRATION_REPORT_COMMAND", command);
        }
    }
    if external_agent_management {
        cmd.env("KITSUNE_AGENT_MANAGEMENT", "external");
    }
}

pub(crate) fn binary_path_env_value() -> String {
    // On Linux an unlinked running executable is reported with " (deleted)".
    // Validate the path rather than exporting that unusable name to children.
    let paths = std::env::var_os("PATH");
    let cwd = std::env::current_dir().ok();
    resolve_binary_path(
        std::env::current_exe().ok(),
        paths.as_deref(),
        cwd.as_deref(),
    )
    .and_then(|path| path.into_os_string().into_string().ok())
    // Always override inherited values, even when no absolute executable resolves.
    .unwrap_or_else(|| crate::product::CLI_NAME.to_string())
}

fn resolve_binary_path(
    current_exe: Option<PathBuf>,
    paths: Option<&OsStr>,
    cwd: Option<&Path>,
) -> Option<PathBuf> {
    let valid =
        |path: &PathBuf| path.is_absolute() && super::registry::executable_file_exists(path);
    if let Some(path) = current_exe.filter(valid) {
        return Some(path);
    }

    std::env::split_paths(paths?)
        .filter_map(|dir| {
            if dir.is_absolute() {
                Some(dir)
            } else {
                cwd.map(|cwd| cwd.join(dir))
            }
        })
        .flat_map(|dir| super::registry::command_path_candidates(&dir, crate::product::CLI_NAME))
        .find(valid)
}

pub(crate) fn pi_extension_dir() -> io::Result<PathBuf> {
    Ok(
        config_dir_from_env_or_home(PI_CODING_AGENT_DIR_ENV_VAR, &[".pi", "agent"])?
            .join("extensions"),
    )
}

pub(crate) fn claude_dir() -> io::Result<PathBuf> {
    config_dir_from_env_or_home(CLAUDE_CONFIG_DIR_ENV_VAR, &[".claude"])
}

pub(crate) fn codex_dir() -> io::Result<PathBuf> {
    config_dir_from_env_or_home(CODEX_HOME_ENV_VAR, &[".codex"])
}

pub(crate) fn copilot_dir() -> io::Result<PathBuf> {
    config_dir_from_env_or_home(COPILOT_HOME_ENV_VAR, &[".copilot"])
}

pub(crate) fn config_dir_from_env_or_home(
    env_var: &str,
    home_relative_segments: &[&str],
) -> io::Result<PathBuf> {
    if let Some(value) = std::env::var_os(env_var).filter(|value| !value.is_empty()) {
        return expand_tilde_path(PathBuf::from(value));
    }

    let mut path = home_dir()?;
    for segment in home_relative_segments {
        path.push(segment);
    }
    Ok(path)
}

pub(crate) fn expand_tilde_path(path: PathBuf) -> io::Result<PathBuf> {
    let Some(raw) = path.to_str() else {
        return Ok(path);
    };

    if raw == "~" {
        return home_dir();
    }

    if let Some(rest) = raw
        .strip_prefix("~/")
        .or_else(|| raw.strip_prefix("~\\"))
        .or_else(|| raw.strip_prefix('~'))
    {
        return Ok(home_dir()?.join(rest));
    }

    Ok(path)
}

pub(crate) fn opencode_dir() -> io::Result<PathBuf> {
    Ok(home_dir()?.join(".config/opencode"))
}

pub(crate) fn home_dir() -> io::Result<PathBuf> {
    if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home));
    }

    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
            return Ok(PathBuf::from(profile));
        }
        if let (Some(drive), Some(path)) = (
            std::env::var_os("HOMEDRIVE").filter(|value| !value.is_empty()),
            std::env::var_os("HOMEPATH").filter(|value| !value.is_empty()),
        ) {
            let mut home = PathBuf::from(drive);
            home.push(path);
            return Ok(home);
        }
    }

    Err(io::Error::other(
        "home directory is not set; cannot locate home directory",
    ))
}

#[cfg(test)]
pub(crate) fn integration_env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct BinaryFixture(PathBuf);

    impl BinaryFixture {
        fn new() -> Self {
            static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let base = std::env::temp_dir().join(format!(
                "kitsune-binary-path-test-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir(&base).unwrap();
            Self(base)
        }

        fn executable(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, b"test executable").unwrap();
            let permissions = fs::metadata(std::env::current_exe().unwrap())
                .unwrap()
                .permissions();
            fs::set_permissions(&path, permissions).unwrap();
            path
        }

        fn cli(&self) -> PathBuf {
            self.executable(&format!(
                "{}{}",
                crate::product::CLI_NAME,
                std::env::consts::EXE_SUFFIX
            ))
        }
    }

    impl Drop for BinaryFixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn binary_path_prefers_valid_current_executable() {
        let fixture = BinaryFixture::new();
        let current = fixture.executable("current");
        fixture.cli();
        assert_eq!(
            resolve_binary_path(Some(current.clone()), Some(fixture.0.as_os_str()), None),
            Some(current)
        );
    }

    #[test]
    fn binary_path_missing_or_deleted_current_uses_path() {
        let fixture = BinaryFixture::new();
        let fallback = fixture.cli();
        for current in [
            None,
            Some(fixture.0.join("missing")),
            Some(fixture.0.join("kitsune (deleted)")),
        ] {
            assert_eq!(
                resolve_binary_path(current, Some(fixture.0.as_os_str()), None),
                Some(fallback.clone())
            );
        }
    }

    #[test]
    fn binary_path_no_candidates_returns_none() {
        let fixture = BinaryFixture::new();
        assert_eq!(resolve_binary_path(None, None, None), None);
        assert_eq!(
            resolve_binary_path(
                Some(fixture.0.join("kitsune (deleted)")),
                Some(fixture.0.as_os_str()),
                None
            ),
            None
        );
    }

    #[test]
    fn binary_path_skips_invalid_path_entries_and_uses_first_executable() {
        let missing = BinaryFixture::new();
        let directory = BinaryFixture::new();
        fs::create_dir(directory.0.join(crate::product::CLI_NAME)).unwrap();
        let first = BinaryFixture::new();
        let expected = first.cli();
        let second = BinaryFixture::new();
        second.cli();
        let paths = std::env::join_paths([&missing.0, &directory.0, &first.0, &second.0]).unwrap();
        assert_eq!(
            resolve_binary_path(None, Some(&paths), None),
            Some(expected)
        );
    }

    #[test]
    fn binary_path_rejects_directories_and_nonexecutables_when_supported() {
        let fixture = BinaryFixture::new();
        let candidate = fixture.0.join(crate::product::CLI_NAME);
        fs::create_dir(&candidate).unwrap();
        assert_eq!(
            resolve_binary_path(Some(candidate.clone()), Some(fixture.0.as_os_str()), None),
            None
        );
        fs::remove_dir(&candidate).unwrap();
        fs::write(&candidate, b"not executable").unwrap();
        // Some platforms use file extensions rather than executable permission bits.
        if !super::super::registry::executable_file_exists(&candidate) {
            assert_eq!(
                resolve_binary_path(Some(candidate), Some(fixture.0.as_os_str()), None),
                None
            );
        }
    }

    #[test]
    fn binary_path_resolves_relative_path_entries_without_canonicalizing() {
        let fixture = BinaryFixture::new();
        let fallback = fixture.cli();
        assert_eq!(
            resolve_binary_path(None, Some(OsStr::new(".")), Some(&fixture.0)),
            Some(fixture.0.join(".").join(fallback.file_name().unwrap()))
        );
        assert_eq!(resolve_binary_path(None, Some(OsStr::new(".")), None), None);
    }

    #[test]
    fn pane_base_env_overwrites_inherited_binary_path_and_preserves_context() {
        let mut cmd = CommandBuilder::new("shell");
        cmd.env(
            crate::product::BIN_PATH_ENV_VAR,
            "/missing/kitsune (deleted)",
        );
        cmd.env(crate::product::SESSION_ENV_VAR, "test-session");
        apply_pane_base_env(&mut cmd);
        let binary = cmd.get_env(crate::product::BIN_PATH_ENV_VAR).unwrap();
        assert!(!binary.is_empty());
        assert_ne!(binary, OsStr::new("/missing/kitsune (deleted)"));
        assert_eq!(
            cmd.get_env(crate::product::SESSION_ENV_VAR),
            Some(OsStr::new("test-session"))
        );
        // Do not recompute the socket path: other tests can change process-global
        // session/environment state after apply_pane_base_env captures it.
        assert!(!cmd
            .get_env(crate::api::SOCKET_PATH_ENV_VAR)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn integration_runtime_env_replaces_nested_routes_and_round_trips_argv() {
        let mut cmd = CommandBuilder::new("shell");
        let mut runtime = crate::config::IntegrationRuntimeConfig {
            source: "example".into(),
            command: vec!["/helper".into()],
            report_command: vec!["/report".into(), "space quote \" $HOME ~/literal".into()],
            external_agent_management: true,
        };
        for key in [
            "KITSUNE_INTEGRATION_SOURCE",
            "KITSUNE_INTEGRATION_REPORT_COMMAND",
            "KITSUNE_AGENT_MANAGEMENT",
        ] {
            cmd.env(key, "inherited-parent-route");
        }
        apply_runtime_env(&mut cmd, Some(&runtime), true);
        assert_eq!(
            cmd.get_env("KITSUNE_INTEGRATION_SOURCE"),
            Some(OsStr::new("example"))
        );
        assert_eq!(
            cmd.get_env("KITSUNE_AGENT_MANAGEMENT"),
            Some(OsStr::new("external"))
        );
        let argv: Vec<String> = serde_json::from_str(
            cmd.get_env("KITSUNE_INTEGRATION_REPORT_COMMAND")
                .unwrap()
                .to_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(argv, runtime.report_command);
        runtime.external_agent_management = false;
        apply_runtime_env(&mut cmd, Some(&runtime), false);
        assert_eq!(cmd.get_env("KITSUNE_AGENT_MANAGEMENT"), None);
        assert!(cmd.get_env("KITSUNE_INTEGRATION_SOURCE").is_some());
        apply_runtime_env(&mut cmd, None, false);
        for key in [
            "KITSUNE_INTEGRATION_SOURCE",
            "KITSUNE_INTEGRATION_REPORT_COMMAND",
            "KITSUNE_AGENT_MANAGEMENT",
        ] {
            assert_eq!(cmd.get_env(key), None);
        }
        apply_runtime_env(&mut cmd, None, true);
        assert_eq!(
            cmd.get_env("KITSUNE_AGENT_MANAGEMENT"),
            Some(OsStr::new("external"))
        );
        assert_eq!(cmd.get_env("KITSUNE_INTEGRATION_SOURCE"), None);
        assert_eq!(cmd.get_env("KITSUNE_INTEGRATION_REPORT_COMMAND"), None);
    }
}
