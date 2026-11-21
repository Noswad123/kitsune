use std::fs;
use std::io;

use serde_json::Value;

use super::config_edit::{
    hooks_object_if_present, remove_direct_hook_commands, remove_hook_commands,
};
use super::env::{claude_dir, codex_dir, copilot_dir, opencode_dir, pi_extension_dir};
use super::file_ops::{remove_file_if_exists, remove_legacy_bash_hook_file};
use super::types::{
    ClaudeUninstallResult, CodexUninstallResult, CopilotUninstallResult, OpenCodeUninstallResult,
    PiUninstallResult,
};
use super::{
    CLAUDE_HOOK_INSTALL_NAME, CODEX_HOOK_INSTALL_NAME, COPILOT_HOOK_EVENTS,
    COPILOT_HOOK_INSTALL_NAME, COPILOT_REMOVED_LIFECYCLE_HOOK_EVENTS, OPENCODE_PLUGIN_INSTALL_NAME,
    PI_EXTENSION_INSTALL_NAME,
};

pub(crate) fn uninstall_pi() -> io::Result<PiUninstallResult> {
    let extension_path = pi_extension_dir()?.join(PI_EXTENSION_INSTALL_NAME);
    let removed_extension = remove_file_if_exists(&extension_path)?;

    Ok(PiUninstallResult {
        extension_path,
        removed_extension,
    })
}

pub(crate) fn uninstall_claude() -> io::Result<ClaudeUninstallResult> {
    let hook_path = claude_dir()?.join("hooks").join(CLAUDE_HOOK_INSTALL_NAME);
    let settings_path = claude_dir()?.join("settings.json");
    let mut updated_settings = false;

    if settings_path.is_file() {
        let mut settings = serde_json::from_str::<Value>(&fs::read_to_string(&settings_path)?)
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to parse {}: {err}",
                    settings_path.display()
                ))
            })?;

        if let Some(hooks) = hooks_object_if_present(
            &mut settings,
            &settings_path,
            "claude settings",
            "claude settings hooks",
        )? {
            updated_settings |=
                remove_hook_commands(hooks, "SessionStart", &hook_path, Some("idle"))?;
            updated_settings |=
                remove_hook_commands(hooks, "SessionStart", &hook_path, Some("session"))?;
            updated_settings |=
                remove_hook_commands(hooks, "UserPromptSubmit", &hook_path, Some("working"))?;
            updated_settings |=
                remove_hook_commands(hooks, "PreToolUse", &hook_path, Some("working"))?;
            updated_settings |=
                remove_hook_commands(hooks, "PermissionRequest", &hook_path, Some("blocked"))?;
            updated_settings |=
                remove_hook_commands(hooks, "PostToolUse", &hook_path, Some("working"))?;
            updated_settings |=
                remove_hook_commands(hooks, "PostToolUseFailure", &hook_path, Some("working"))?;
            updated_settings |=
                remove_hook_commands(hooks, "SubagentStop", &hook_path, Some("working"))?;
            updated_settings |= remove_hook_commands(hooks, "Stop", &hook_path, Some("idle"))?;
            updated_settings |=
                remove_hook_commands(hooks, "SessionEnd", &hook_path, Some("release"))?;
        }

        if updated_settings {
            fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
        }
    }

    let removed_hook_file =
        remove_file_if_exists(&hook_path)? | remove_legacy_bash_hook_file(&hook_path)?;

    Ok(ClaudeUninstallResult {
        hook_path,
        settings_path,
        removed_hook_file,
        updated_settings,
    })
}

pub(crate) fn uninstall_codex() -> io::Result<CodexUninstallResult> {
    let codex_dir = codex_dir()?;
    let hook_path = codex_dir.join(CODEX_HOOK_INSTALL_NAME);
    let hooks_path = codex_dir.join("hooks.json");
    let config_path = codex_dir.join("config.toml");
    let mut updated_hooks = false;

    if hooks_path.is_file() {
        let mut hooks_file = serde_json::from_str::<Value>(&fs::read_to_string(&hooks_path)?)
            .map_err(|err| {
                io::Error::other(format!("failed to parse {}: {err}", hooks_path.display()))
            })?;

        if let Some(hooks) = hooks_object_if_present(
            &mut hooks_file,
            &hooks_path,
            "codex hooks file",
            "codex hooks file hooks",
        )? {
            updated_hooks |= remove_hook_commands(hooks, "SessionStart", &hook_path, Some("idle"))?;
            updated_hooks |=
                remove_hook_commands(hooks, "SessionStart", &hook_path, Some("session"))?;
            updated_hooks |=
                remove_hook_commands(hooks, "UserPromptSubmit", &hook_path, Some("working"))?;
            updated_hooks |=
                remove_hook_commands(hooks, "PreToolUse", &hook_path, Some("working"))?;
            updated_hooks |=
                remove_hook_commands(hooks, "PermissionRequest", &hook_path, Some("blocked"))?;
            updated_hooks |= remove_hook_commands(hooks, "Stop", &hook_path, Some("idle"))?;
        }

        if updated_hooks {
            fs::write(&hooks_path, serde_json::to_string_pretty(&hooks_file)?)?;
        }
    }

    let removed_hook_file =
        remove_file_if_exists(&hook_path)? | remove_legacy_bash_hook_file(&hook_path)?;

    Ok(CodexUninstallResult {
        hook_path,
        hooks_path,
        config_path,
        removed_hook_file,
        updated_hooks,
    })
}

pub(crate) fn uninstall_copilot() -> io::Result<CopilotUninstallResult> {
    let copilot_dir = copilot_dir()?;
    let hook_path = copilot_dir.join("hooks").join(COPILOT_HOOK_INSTALL_NAME);
    let settings_path = copilot_dir.join("settings.json");
    let mut updated_settings = false;

    if settings_path.is_file() {
        let mut settings = serde_json::from_str::<Value>(&fs::read_to_string(&settings_path)?)
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to parse {}: {err}",
                    settings_path.display()
                ))
            })?;

        if let Some(hooks) = hooks_object_if_present(
            &mut settings,
            &settings_path,
            "copilot settings",
            "copilot settings hooks",
        )? {
            for event in COPILOT_HOOK_EVENTS {
                updated_settings |= remove_direct_hook_commands(hooks, event, &hook_path, None)?;
            }
            for event in COPILOT_REMOVED_LIFECYCLE_HOOK_EVENTS {
                updated_settings |= remove_direct_hook_commands(hooks, event, &hook_path, None)?;
            }
        }

        if updated_settings {
            fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
        }
    }

    let removed_hook_file =
        remove_file_if_exists(&hook_path)? | remove_legacy_bash_hook_file(&hook_path)?;

    Ok(CopilotUninstallResult {
        hook_path,
        settings_path,
        removed_hook_file,
        updated_settings,
    })
}

pub(crate) fn uninstall_opencode() -> io::Result<OpenCodeUninstallResult> {
    let plugin_path = opencode_dir()?
        .join("plugins")
        .join(OPENCODE_PLUGIN_INSTALL_NAME);
    let removed_plugin = remove_file_if_exists(&plugin_path)?;

    Ok(OpenCodeUninstallResult {
        plugin_path,
        removed_plugin,
    })
}
