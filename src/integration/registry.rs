use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::env::*;

pub(crate) fn integration_target_label(
    target: crate::api::schema::IntegrationTarget,
) -> &'static str {
    match target {
        crate::api::schema::IntegrationTarget::Pi => "pi",
        crate::api::schema::IntegrationTarget::Claude => "claude",
        crate::api::schema::IntegrationTarget::Codex => "codex",
        crate::api::schema::IntegrationTarget::Copilot => "copilot",
        crate::api::schema::IntegrationTarget::Opencode => "opencode",
        crate::api::schema::IntegrationTarget::Djinn => "djinn",
    }
}

pub(crate) fn integration_target_supported(target: crate::api::schema::IntegrationTarget) -> bool {
    #[cfg(windows)]
    {
        matches!(
            target,
            crate::api::schema::IntegrationTarget::Pi
                | crate::api::schema::IntegrationTarget::Claude
                | crate::api::schema::IntegrationTarget::Codex
                | crate::api::schema::IntegrationTarget::Copilot
                | crate::api::schema::IntegrationTarget::Opencode
                | crate::api::schema::IntegrationTarget::Djinn
        )
    }

    #[cfg(not(windows))]
    {
        let _ = target;
        true
    }
}

pub(crate) fn command_path_candidates(dir: &Path, command: &str) -> Vec<PathBuf> {
    let base = dir.join(command);

    #[cfg(not(windows))]
    {
        vec![base]
    }

    #[cfg(windows)]
    {
        if Path::new(command).extension().is_some() {
            return vec![base];
        }

        let mut candidates = vec![base];
        for extension in [".exe", ".cmd", ".bat", ".ps1"] {
            candidates.push(dir.join(format!("{command}{extension}")));
        }
        candidates
    }
}

pub(crate) fn executable_file_exists(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

// Read-only inspection of retired legacy hooks on every platform.
pub(crate) fn installed_integration_statuses() -> Vec<super::IntegrationStatus> {
    integration_specs()
        .into_iter()
        .filter_map(|(target, path, expected_version)| {
            if !integration_target_supported(target) {
                return None;
            }
            Some(integration_status_at(target, path.ok()?, expected_version))
        })
        .collect()
}

fn integration_specs() -> [(
    crate::api::schema::IntegrationTarget,
    io::Result<PathBuf>,
    u32,
); 5] {
    [
        (
            crate::api::schema::IntegrationTarget::Pi,
            pi_extension_dir().map(|dir| dir.join(super::PI_EXTENSION_INSTALL_NAME)),
            super::PI_INTEGRATION_VERSION,
        ),
        (
            crate::api::schema::IntegrationTarget::Claude,
            claude_dir().map(|dir| dir.join("hooks").join(super::CLAUDE_HOOK_INSTALL_NAME)),
            super::CLAUDE_INTEGRATION_VERSION,
        ),
        (
            crate::api::schema::IntegrationTarget::Codex,
            codex_dir().map(|dir| dir.join(super::CODEX_HOOK_INSTALL_NAME)),
            super::CODEX_INTEGRATION_VERSION,
        ),
        (
            crate::api::schema::IntegrationTarget::Copilot,
            copilot_dir().map(|dir| dir.join("hooks").join(super::COPILOT_HOOK_INSTALL_NAME)),
            super::COPILOT_INTEGRATION_VERSION,
        ),
        (
            crate::api::schema::IntegrationTarget::Opencode,
            opencode_dir().map(|dir| {
                dir.join("plugins")
                    .join(super::OPENCODE_PLUGIN_INSTALL_NAME)
            }),
            super::OPENCODE_INTEGRATION_VERSION,
        ),
    ]
}

pub(crate) fn integration_status_at(
    target: crate::api::schema::IntegrationTarget,
    path: PathBuf,
    expected_version: u32,
) -> super::IntegrationStatus {
    if !path.is_file() {
        return super::IntegrationStatus {
            target,
            path,
            state: super::IntegrationStatusKind::NotInstalled,
            installed_version: None,
            expected_version,
        };
    }

    let installed_version = fs::read_to_string(&path)
        .ok()
        .and_then(|content| parse_integration_version(&content));
    let state = if installed_version.is_some_and(|version| version >= expected_version) {
        super::IntegrationStatusKind::Current
    } else {
        super::IntegrationStatusKind::Outdated
    };

    super::IntegrationStatus {
        target,
        path,
        state,
        installed_version,
        expected_version,
    }
}

pub(crate) fn parse_integration_version(content: &str) -> Option<u32> {
    content.lines().find_map(|line| {
        let marker_line = line
            .trim()
            .trim_start_matches('/')
            .trim_start_matches('#')
            .trim();
        marker_line
            .strip_prefix(super::INTEGRATION_VERSION_MARKER)?
            .trim()
            .parse()
            .ok()
    })
}
