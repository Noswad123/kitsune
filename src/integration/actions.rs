use std::io;

use super::registry::{integration_target_label, integration_target_supported};
use super::targets::{
    install_claude, install_codex, install_copilot, install_opencode, install_pi, uninstall_claude,
    uninstall_codex, uninstall_copilot, uninstall_opencode, uninstall_pi,
};
use super::version::{agent_version_requirement, enforce_agent_version};

pub(crate) fn install_target(
    target: crate::api::schema::IntegrationTarget,
) -> io::Result<Vec<String>> {
    let result = install_target_inner(target);
    let outcome = if result.is_ok() { "ok" } else { "error" };
    crate::logging::integration_action("install", integration_target_label(target), outcome);
    result
}

fn install_target_inner(target: crate::api::schema::IntegrationTarget) -> io::Result<Vec<String>> {
    if !integration_target_supported(target) {
        return Err(io::Error::other(format!(
            "{} integration is not supported on Windows",
            integration_target_label(target)
        )));
    }

    let version_warning = match agent_version_requirement(target) {
        Some(requirement) => enforce_agent_version(&requirement)?,
        None => None,
    };

    let mut messages = match target {
        crate::api::schema::IntegrationTarget::Pi => {
            let path = install_pi()?;
            vec![format!("installed pi integration to {}", path.display())]
        }
        crate::api::schema::IntegrationTarget::Claude => {
            let installed = install_claude()?;
            vec![
                format!(
                    "installed claude integration hook to {}",
                    installed.hook_path.display()
                ),
                format!(
                    "ensured claude settings at {}",
                    installed.settings_path.display()
                ),
            ]
        }
        crate::api::schema::IntegrationTarget::Codex => {
            let installed = install_codex()?;
            vec![
                format!(
                    "installed codex integration hook to {}",
                    installed.hook_path.display()
                ),
                format!("ensured codex hooks at {}", installed.hooks_path.display()),
                format!(
                    "ensured codex config at {}",
                    installed.config_path.display()
                ),
            ]
        }
        crate::api::schema::IntegrationTarget::Copilot => {
            let installed = install_copilot()?;
            vec![
                format!(
                    "installed copilot integration hook to {}",
                    installed.hook_path.display()
                ),
                format!(
                    "ensured copilot settings at {}",
                    installed.settings_path.display()
                ),
            ]
        }
        crate::api::schema::IntegrationTarget::Opencode => {
            let installed = install_opencode()?;
            vec![format!(
                "installed opencode integration plugin to {}",
                installed.plugin_path.display()
            )]
        }
        crate::api::schema::IntegrationTarget::Djinn => {
            vec!["djinn integration is built in; no external installer is required".to_string()]
        }
    };

    if let Some(warning) = version_warning {
        messages.push(warning);
    }

    Ok(messages)
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
