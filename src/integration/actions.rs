use std::io;

use super::registry::integration_target_label;
use super::targets::{
    uninstall_claude, uninstall_codex, uninstall_copilot, uninstall_opencode, uninstall_pi,
};

pub(crate) fn install_target(
    _target: crate::api::schema::IntegrationTarget,
) -> io::Result<Vec<String>> {
    Err(io::Error::other(super::INSTALL_RETIRED_GUIDANCE))
}

pub(crate) fn uninstall_target(
    target: crate::api::schema::IntegrationTarget,
) -> io::Result<Vec<String>> {
    let messages = match target {
        crate::api::schema::IntegrationTarget::Pi => {
            let result = uninstall_pi()?;
            if result.removed_extension {
                vec![format!(
                    "removed pi integration extension at {}",
                    result.extension_path.display()
                )]
            } else {
                vec![format!(
                    "no pi integration extension found at {}",
                    result.extension_path.display()
                )]
            }
        }
        crate::api::schema::IntegrationTarget::Claude => {
            let result = uninstall_claude()?;
            let mut messages = Vec::new();
            #[cfg(target_os = "linux")]
            messages.push(format!(
                "legacy Kitsune Claude cleanup only; {}",
                super::CLAUDE_COVEN_GUIDANCE
            ));
            if result.removed_hook_file {
                messages.push(format!(
                    "removed claude integration hook at {}",
                    result.hook_path.display()
                ));
            } else {
                messages.push(format!(
                    "no claude integration hook found at {}",
                    result.hook_path.display()
                ));
            }
            if result.updated_settings {
                messages.push(format!(
                    "removed claude hook commands from {}",
                    result.settings_path.display()
                ));
            } else {
                messages.push(format!(
                    "no claude hook commands found in {}",
                    result.settings_path.display()
                ));
            }
            messages
        }
        crate::api::schema::IntegrationTarget::Codex => {
            let result = uninstall_codex()?;
            let mut messages = Vec::new();
            #[cfg(target_os = "linux")]
            messages.push(format!(
                "legacy Kitsune Codex cleanup only; {}",
                super::CODEX_COVEN_GUIDANCE
            ));
            if result.removed_hook_file {
                messages.push(format!(
                    "removed codex integration hook at {}",
                    result.hook_path.display()
                ));
            } else {
                messages.push(format!(
                    "no codex integration hook found at {}",
                    result.hook_path.display()
                ));
            }
            if result.updated_hooks {
                messages.push(format!(
                    "removed codex hook commands from {}",
                    result.hooks_path.display()
                ));
            } else {
                messages.push(format!(
                    "no codex hook commands found in {}",
                    result.hooks_path.display()
                ));
            }
            messages.push(format!(
                "codex config checked at {}",
                result.config_path.display()
            ));
            messages
        }
        crate::api::schema::IntegrationTarget::Copilot => {
            let result = uninstall_copilot()?;
            let mut messages = Vec::new();
            #[cfg(target_os = "linux")]
            messages.push(format!(
                "legacy Kitsune Copilot cleanup only; {}",
                super::COPILOT_COVEN_GUIDANCE
            ));
            if result.removed_hook_file {
                messages.push(format!(
                    "removed copilot integration hook at {}",
                    result.hook_path.display()
                ));
            } else {
                messages.push(format!(
                    "no copilot integration hook found at {}",
                    result.hook_path.display()
                ));
            }
            if result.updated_settings {
                messages.push(format!(
                    "removed copilot hook commands from {}",
                    result.settings_path.display()
                ));
            } else {
                messages.push(format!(
                    "no copilot hook commands found in {}",
                    result.settings_path.display()
                ));
            }
            messages
        }
        crate::api::schema::IntegrationTarget::Opencode => {
            let result = uninstall_opencode()?;
            if result.removed_plugin {
                vec![format!(
                    "removed opencode integration plugin at {}",
                    result.plugin_path.display()
                )]
            } else {
                vec![format!(
                    "no opencode integration plugin found at {}",
                    result.plugin_path.display()
                )]
            }
        }
        crate::api::schema::IntegrationTarget::Djinn => {
            vec!["djinn integration is built in; no external files were removed".to_string()]
        }
    };

    crate::logging::integration_action("uninstall", integration_target_label(target), "ok");
    Ok(messages)
}
