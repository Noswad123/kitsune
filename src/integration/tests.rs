use super::command::*;
use super::config_edit::*;
use super::env::*;
use super::registry::*;
use super::targets::*;
use super::version::*;
use super::*;

use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

fn clear_integration_path_env() {
    std::env::remove_var(PI_CODING_AGENT_DIR_ENV_VAR);
    std::env::remove_var(CLAUDE_CONFIG_DIR_ENV_VAR);
    std::env::remove_var(CODEX_HOME_ENV_VAR);
    std::env::remove_var(COPILOT_HOME_ENV_VAR);
    std::env::remove_var("XDG_CONFIG_HOME");
}

fn unique_base() -> PathBuf {
    clear_integration_path_env();
    std::env::temp_dir().join(format!(
        "kitsune-integration-install-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn setup_home() -> (std::sync::MutexGuard<'static, ()>, PathBuf) {
    let lock = integration_env_lock();
    let base = unique_base();
    let home = base.join("home");
    fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    (lock, home)
}

#[test]
fn extract_version_triple_parses_common_outputs() {
    assert_eq!(extract_version_triple("0.14.0"), Some((0, 14, 0)));
    assert_eq!(extract_version_triple("v1.2.3"), Some((1, 2, 3)));
    assert_eq!(
        extract_version_triple("agent 0.14.0 (linux/x64)"),
        Some((0, 14, 0))
    );
    assert_eq!(extract_version_triple("0.14"), Some((0, 14, 0)));
    assert_eq!(extract_version_triple("0.14.1-beta.2"), Some((0, 14, 1)));
    assert_eq!(extract_version_triple("no version here"), None);
}

#[test]
fn agent_version_requirement_is_unset_for_supported_targets() {
    for target in crate::api::schema::IntegrationTarget::ALL {
        assert!(agent_version_requirement(target).is_none());
    }
}

#[test]
fn enforce_agent_version_warns_when_binary_missing() {
    let requirement = AgentVersionRequirement {
        label: "agent",
        binary: "kitsune-test-binary-that-does-not-exist",
        args: &["--version"],
        min_version: "0.14.0",
    };
    let warning = enforce_agent_version(&requirement)
        .expect("missing binary must not fail the install")
        .expect("missing binary must produce a warning");
    assert!(warning.contains("could not run"));
    assert!(warning.contains("0.14.0"));
}

#[cfg(unix)]
#[test]
fn command_available_requires_executable_file_on_path() {
    use std::os::unix::fs::PermissionsExt;

    let _lock = integration_env_lock();
    let base = unique_base();
    let bin = base.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let original_path = std::env::var_os("PATH");
    std::env::set_var("PATH", &bin);

    let command = bin.join("pi");
    fs::write(&command, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&command, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!command_available("pi"));

    fs::set_permissions(&command, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(command_available("pi"));

    if let Some(path) = original_path {
        std::env::set_var("PATH", path);
    } else {
        std::env::remove_var("PATH");
    }
    let _ = fs::remove_dir_all(base);
}

#[test]
fn integration_target_labels_match_supported_targets() {
    use crate::api::schema::IntegrationTarget;

    assert_eq!(IntegrationTarget::ALL.len(), 6);
    assert_eq!(integration_target_label(IntegrationTarget::Pi), "pi");
    assert_eq!(
        integration_target_label(IntegrationTarget::Claude),
        "claude"
    );
    assert_eq!(integration_target_label(IntegrationTarget::Codex), "codex");
    assert_eq!(
        integration_target_label(IntegrationTarget::Copilot),
        "copilot"
    );
    assert_eq!(
        integration_target_label(IntegrationTarget::Opencode),
        "opencode"
    );
    assert_eq!(integration_target_label(IntegrationTarget::Djinn), "djinn");
}

#[test]
fn install_pi_writes_embedded_asset_to_extensions_dir() {
    let (_lock, home) = setup_home();
    let extension_dir = home.join(".pi/agent/extensions");
    fs::create_dir_all(&extension_dir).unwrap();

    let installed = install_pi().unwrap();

    assert_eq!(installed, extension_dir.join(PI_EXTENSION_INSTALL_NAME));
    assert_eq!(fs::read_to_string(installed).unwrap(), PI_EXTENSION_ASSET);
}

#[test]
fn uninstall_pi_removes_installed_extension() {
    let (_lock, home) = setup_home();
    let extension_dir = home.join(".pi/agent/extensions");
    fs::create_dir_all(&extension_dir).unwrap();
    let extension_path = extension_dir.join(PI_EXTENSION_INSTALL_NAME);
    fs::write(&extension_path, PI_EXTENSION_ASSET).unwrap();

    let result = uninstall_pi().unwrap();

    assert!(result.removed_extension);
    assert_eq!(result.extension_path, extension_path);
    assert!(!result.extension_path.exists());
}

#[test]
fn install_claude_writes_hook_and_session_start_entry() {
    let (_lock, home) = setup_home();
    let claude_dir = home.join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();

    let installed = install_claude().unwrap();
    let settings: Value =
        serde_json::from_str(&fs::read_to_string(&installed.settings_path).unwrap()).unwrap();

    assert_eq!(
        installed.hook_path,
        claude_dir.join("hooks").join(CLAUDE_HOOK_INSTALL_NAME)
    );
    assert_eq!(
        fs::read_to_string(&installed.hook_path).unwrap(),
        CLAUDE_HOOK_ASSET
    );
    assert_eq!(
        settings["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        hook_command(&installed.hook_path, Some("session"))
    );
}

#[test]
fn uninstall_claude_removes_hook_and_owned_settings_entry() {
    let (_lock, home) = setup_home();
    let claude_dir = home.join(".claude");
    fs::create_dir_all(claude_dir.join("hooks")).unwrap();
    let installed = install_claude().unwrap();

    let result = uninstall_claude().unwrap();
    let settings: Value =
        serde_json::from_str(&fs::read_to_string(&result.settings_path).unwrap()).unwrap();

    assert!(result.removed_hook_file);
    assert!(result.updated_settings);
    assert!(!installed.hook_path.exists());
    assert!(settings["hooks"]
        .as_object()
        .is_some_and(|hooks| hooks.is_empty()));
}

#[test]
fn install_codex_writes_hook_and_enables_config_hooks() {
    let (_lock, home) = setup_home();
    let codex_dir = home.join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("config.toml"),
        "[features]\ncodex_hooks = true\n",
    )
    .unwrap();

    let installed = install_codex().unwrap();
    let hooks: Value =
        serde_json::from_str(&fs::read_to_string(&installed.hooks_path).unwrap()).unwrap();
    let config = fs::read_to_string(&installed.config_path).unwrap();

    assert_eq!(
        fs::read_to_string(&installed.hook_path).unwrap(),
        CODEX_HOOK_ASSET
    );
    assert_eq!(
        hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        hook_command(&installed.hook_path, Some("session"))
    );
    assert!(config.contains("hooks = true"));
    assert!(!config.contains("codex_hooks"));
}

#[test]
fn install_copilot_writes_direct_hook_entry() {
    let (_lock, home) = setup_home();
    let copilot_dir = home.join(".copilot");
    fs::create_dir_all(&copilot_dir).unwrap();

    let installed = install_copilot().unwrap();
    let settings: Value =
        serde_json::from_str(&fs::read_to_string(&installed.settings_path).unwrap()).unwrap();

    assert_eq!(
        fs::read_to_string(&installed.hook_path).unwrap(),
        COPILOT_HOOK_ASSET
    );
    assert_eq!(
        settings["hooks"]["SessionStart"][0][direct_command_field()],
        hook_command(&installed.hook_path, None)
    );
}

#[test]
fn uninstall_copilot_removes_hook_and_owned_settings_entry() {
    let (_lock, home) = setup_home();
    let copilot_dir = home.join(".copilot");
    fs::create_dir_all(&copilot_dir).unwrap();
    let installed = install_copilot().unwrap();

    let result = uninstall_copilot().unwrap();
    let settings: Value =
        serde_json::from_str(&fs::read_to_string(&result.settings_path).unwrap()).unwrap();

    assert!(result.removed_hook_file);
    assert!(result.updated_settings);
    assert!(!installed.hook_path.exists());
    assert!(settings["hooks"]
        .as_object()
        .is_some_and(|hooks| hooks.is_empty()));
}

#[test]
fn install_opencode_writes_plugin() {
    let (_lock, home) = setup_home();
    let opencode_dir = home.join(".config/opencode");
    fs::create_dir_all(&opencode_dir).unwrap();

    let installed = install_opencode().unwrap();

    assert_eq!(
        installed.plugin_path,
        opencode_dir
            .join("plugins")
            .join(OPENCODE_PLUGIN_INSTALL_NAME)
    );
    assert_eq!(
        fs::read_to_string(&installed.plugin_path).unwrap(),
        OPENCODE_PLUGIN_ASSET
    );
}

#[test]
fn uninstall_opencode_removes_plugin() {
    let (_lock, home) = setup_home();
    let opencode_dir = home.join(".config/opencode/plugins");
    fs::create_dir_all(&opencode_dir).unwrap();
    let plugin_path = opencode_dir.join(OPENCODE_PLUGIN_INSTALL_NAME);
    fs::write(&plugin_path, OPENCODE_PLUGIN_ASSET).unwrap();

    let result = uninstall_opencode().unwrap();

    assert!(result.removed_plugin);
    assert_eq!(result.plugin_path, plugin_path);
    assert!(!result.plugin_path.exists());
}

#[test]
fn djinn_install_and_uninstall_are_placeholders() {
    let messages = install_target(crate::api::schema::IntegrationTarget::Djinn).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("reserved for first-class support"));

    let messages = uninstall_target(crate::api::schema::IntegrationTarget::Djinn).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("reserved for first-class support"));
}

#[test]
fn integration_status_uses_version_marker() {
    let (_lock, home) = setup_home();
    let path = home.join("integration.js");
    fs::write(&path, "// KITSUNE_INTEGRATION_VERSION=9\n").unwrap();

    let status = integration_status_at(crate::api::schema::IntegrationTarget::Opencode, path, 9);

    assert_eq!(status.state, IntegrationStatusKind::Current);
    assert_eq!(status.installed_version, Some(9));
}

#[test]
fn integration_status_reports_outdated_missing_marker() {
    let (_lock, home) = setup_home();
    let path = home.join("integration.js");
    fs::write(&path, "// no marker\n").unwrap();

    let status = integration_status_at(crate::api::schema::IntegrationTarget::Opencode, path, 9);

    assert_eq!(status.state, IntegrationStatusKind::Outdated);
    assert_eq!(status.installed_version, None);
}

#[test]
fn build_codex_config_with_hooks_adds_features_table() {
    assert_eq!(
        build_codex_config_with_hooks("model = \"x\"\n"),
        "model = \"x\"\n\n[features]\nhooks = true\n"
    );
}

#[test]
fn direct_hook_helpers_upgrade_legacy_command_field() {
    let mut settings = json!({"hooks": {}});
    let hooks = ensure_hooks_object(
        &mut settings,
        &PathBuf::from("settings.json"),
        "settings",
        "settings hooks",
    )
    .unwrap();
    ensure_direct_command_hook(hooks, "SessionStart", "old".to_string(), 10, None).unwrap();
    ensure_direct_command_hook(hooks, "SessionStart", "old".to_string(), 20, Some("*")).unwrap();

    assert_eq!(
        settings["hooks"]["SessionStart"].as_array().unwrap().len(),
        1
    );
    assert_eq!(settings["hooks"]["SessionStart"][0]["timeoutSec"], 20);
    assert_eq!(settings["hooks"]["SessionStart"][0]["matcher"], "*");
}
