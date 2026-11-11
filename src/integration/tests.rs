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
fn legacy_install_pi_writes_embedded_asset_to_extensions_dir() {
    let (_lock, home) = setup_home();
    let extension_dir = home.join(".pi/agent/extensions");
    fs::create_dir_all(&extension_dir).unwrap();

    let installed = legacy_install_pi().unwrap();

    assert_eq!(installed, extension_dir.join(PI_EXTENSION_INSTALL_NAME));
    assert_eq!(fs::read_to_string(installed).unwrap(), PI_EXTENSION_ASSET);
}

#[cfg(target_os = "linux")]
#[test]
fn pi_install_returns_coven_guidance_without_environment_setup() {
    let error = install_target(crate::api::schema::IntegrationTarget::Pi).unwrap_err();
    assert_eq!(error.to_string(), PI_COVEN_GUIDANCE);
}

#[cfg(target_os = "linux")]
#[test]
fn pi_install_preserves_custom_legacy_extension_without_running_commands() {
    use std::os::unix::fs::PermissionsExt;

    let (_lock, home) = setup_home();
    let custom = home.join("custom-pi");
    let extensions = custom.join("extensions");
    fs::create_dir_all(&extensions).unwrap();
    let path = extensions.join(PI_EXTENSION_INSTALL_NAME);
    fs::write(&path, "legacy extension; preserve exactly\n").unwrap();
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let marker = home.join("command-ran");
    for name in ["pi", "coven"] {
        let command = bin.join(name);
        fs::write(&command, format!("#!/bin/sh\n: > '{}'\n", marker.display())).unwrap();
        fs::set_permissions(command, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let original_path = std::env::var_os("PATH");
    std::env::set_var("PATH", &bin);
    let invalid_home = home.join("not-a-directory");
    fs::write(&invalid_home, "not a home\n").unwrap();
    std::env::set_var("HOME", &invalid_home);
    std::env::set_var(PI_CODING_AGENT_DIR_ENV_VAR, &custom);

    let error = install_target(crate::api::schema::IntegrationTarget::Pi).unwrap_err();

    assert_eq!(error.to_string(), PI_COVEN_GUIDANCE);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "legacy extension; preserve exactly\n"
    );
    assert_eq!(fs::read_dir(&extensions).unwrap().count(), 1);
    assert_eq!(fs::read_to_string(&invalid_home).unwrap(), "not a home\n");
    assert!(!marker.exists());
    assert!(!home.join(".pi").exists());
    if let Some(path) = original_path {
        std::env::set_var("PATH", path);
    } else {
        std::env::remove_var("PATH");
    }
    std::env::set_var("HOME", &home);
    std::env::remove_var(PI_CODING_AGENT_DIR_ENV_VAR);
}

#[test]
fn pi_candidates_follow_platform_policy_and_preserve_legacy_status() {
    use crate::api::schema::IntegrationTarget;

    let (_lock, home) = setup_home();
    let dir = home.join(".pi/agent/extensions");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(PI_EXTENSION_INSTALL_NAME);
    let legacy = "// KITSUNE_INTEGRATION_VERSION=1\n";
    fs::write(&path, legacy).unwrap();
    let status = installed_integration_statuses()
        .into_iter()
        .find(|status| status.target == IntegrationTarget::Pi)
        .unwrap();
    assert_eq!(status.path, path);
    assert_eq!(status.state, IntegrationStatusKind::Outdated);
    assert_eq!(status.installed_version, Some(1));
    let legacy_installable = !cfg!(target_os = "linux");
    let recommendations = integration_recommendations();
    assert_eq!(
        recommendations
            .iter()
            .any(|item| item.target == IntegrationTarget::Pi),
        legacy_installable
    );
    assert_eq!(
        recommendations
            .iter()
            .any(|item| item.target == IntegrationTarget::Pi && item.needs_install()),
        legacy_installable
    );
    assert_eq!(
        outdated_installed_integrations()
            .iter()
            .any(|item| item.target == IntegrationTarget::Pi),
        legacy_installable
    );
    let instructions = integration_update_instructions(&[IntegrationTarget::Pi]);
    assert_eq!(
        instructions.contains("kitsune integration install pi"),
        legacy_installable
    );
    #[cfg(target_os = "linux")]
    assert!(instructions.contains(PI_COVEN_GUIDANCE));
    assert_eq!(fs::read_to_string(path).unwrap(), legacy);
}

#[cfg(not(target_os = "linux"))]
#[test]
fn pi_install_target_retains_legacy_result_path() {
    let (_lock, home) = setup_home();
    let dir = home.join(".pi/agent/extensions");
    fs::create_dir_all(&dir).unwrap();
    let messages = install_target(crate::api::schema::IntegrationTarget::Pi).unwrap();
    assert_eq!(
        messages,
        vec![format!(
            "installed pi integration to {}",
            dir.join(PI_EXTENSION_INSTALL_NAME).display()
        )]
    );
    assert_eq!(
        fs::read_to_string(dir.join(PI_EXTENSION_INSTALL_NAME)).unwrap(),
        PI_EXTENSION_ASSET
    );
}

#[test]
fn uninstall_pi_removes_installed_extension() {
    let (_lock, home) = setup_home();
    let extension_dir = home.join(".pi/agent/extensions");
    fs::create_dir_all(&extension_dir).unwrap();
    let extension_path = extension_dir.join(PI_EXTENSION_INSTALL_NAME);
    fs::write(&extension_path, PI_EXTENSION_ASSET).unwrap();
    let coven_path = extension_dir.join("coven-agent-reference.ts");
    fs::write(&coven_path, "Coven-owned; preserve exactly\n").unwrap();

    let result = uninstall_pi().unwrap();

    assert!(result.removed_extension);
    assert_eq!(result.extension_path, extension_path);
    assert!(!result.extension_path.exists());
    assert_eq!(
        fs::read_to_string(coven_path).unwrap(),
        "Coven-owned; preserve exactly\n"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn install_target_claude_returns_coven_guidance_without_environment_setup() {
    let error = install_target(crate::api::schema::IntegrationTarget::Claude).unwrap_err();

    assert_eq!(error.to_string(), CLAUDE_COVEN_GUIDANCE);
    assert!(error.to_string().contains("managed by Coven"));
    assert!(error.to_string().contains("coven agent hooks plan"));
    assert!(error.to_string().contains("coven agent hooks install"));
    assert!(error.to_string().contains("--migrate-kitsune"));
}

#[cfg(target_os = "linux")]
#[test]
fn install_target_claude_preserves_custom_legacy_config_with_invalid_home() {
    let (_lock, home) = setup_home();
    let custom = home.join("custom-claude");
    let hooks = custom.join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let settings_path = custom.join("settings.json");
    let hook_path = hooks.join(CLAUDE_HOOK_INSTALL_NAME);
    // Invalid JSON must not be read or repaired by the retired installer.
    fs::write(&settings_path, "not JSON; preserve exactly\n").unwrap();
    fs::write(&hook_path, "legacy hook; preserve exactly\n").unwrap();
    let invalid_home = home.join("not-a-directory");
    fs::write(&invalid_home, "not a home\n").unwrap();
    std::env::set_var("HOME", &invalid_home);
    std::env::set_var(CLAUDE_CONFIG_DIR_ENV_VAR, &custom);

    let error = install_target(crate::api::schema::IntegrationTarget::Claude).unwrap_err();

    assert_eq!(error.to_string(), CLAUDE_COVEN_GUIDANCE);
    assert_eq!(
        fs::read_to_string(&settings_path).unwrap(),
        "not JSON; preserve exactly\n"
    );
    assert_eq!(
        fs::read_to_string(&hook_path).unwrap(),
        "legacy hook; preserve exactly\n"
    );
    assert_eq!(fs::read_dir(&custom).unwrap().count(), 2);
    assert_eq!(fs::read_dir(&hooks).unwrap().count(), 1);
    assert_eq!(fs::read_to_string(&invalid_home).unwrap(), "not a home\n");
    assert!(!home.join(".claude").exists());

    std::env::set_var("HOME", &home);
    std::env::remove_var(CLAUDE_CONFIG_DIR_ENV_VAR);
    fs::remove_dir_all(home.parent().unwrap()).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn legacy_claude_status_remains_read_only_but_is_never_an_install_or_update_candidate() {
    use crate::api::schema::IntegrationTarget;

    let (_lock, home) = setup_home();
    let hooks = home.join(".claude/hooks");
    fs::create_dir_all(&hooks).unwrap();
    let path = hooks.join(CLAUDE_HOOK_INSTALL_NAME);
    let legacy = "# KITSUNE_INTEGRATION_VERSION=1\n";
    fs::write(&path, legacy).unwrap();
    let pi_dir = home.join(".pi/agent/extensions");
    fs::create_dir_all(&pi_dir).unwrap();
    fs::write(pi_dir.join(PI_EXTENSION_INSTALL_NAME), "// legacy\n").unwrap();

    let statuses = installed_integration_statuses();
    let claude = statuses
        .iter()
        .find(|status| status.target == IntegrationTarget::Claude)
        .unwrap();
    assert_eq!(claude.path, path);
    assert_eq!(claude.state, IntegrationStatusKind::Outdated);
    assert_eq!(claude.installed_version, Some(1));
    assert!(!integration_recommendations()
        .iter()
        .any(|item| item.target == IntegrationTarget::Claude));
    let outdated = outdated_installed_integrations();
    assert!(!outdated
        .iter()
        .any(|item| item.target == IntegrationTarget::Claude));
    assert!(!outdated
        .iter()
        .any(|item| item.target == IntegrationTarget::Pi));
    let instructions =
        integration_update_instructions(&[IntegrationTarget::Claude, IntegrationTarget::Pi]);
    assert!(!instructions.contains("kitsune integration install claude"));
    assert!(instructions.contains("managed by Coven"));
    assert!(instructions.contains(PI_COVEN_GUIDANCE));
    assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
}

#[test]
fn legacy_claude_fixture_writes_hook_and_session_start_entry() {
    let (_lock, home) = setup_home();
    let claude_dir = home.join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();

    let installed = legacy_install_claude().unwrap();
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
    let installed = legacy_install_claude().unwrap();

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

#[cfg(target_os = "linux")]
#[test]
fn codex_install_returns_coven_guidance_without_environment_setup() {
    let error = install_target(crate::api::schema::IntegrationTarget::Codex).unwrap_err();

    assert_eq!(error.to_string(), CODEX_COVEN_GUIDANCE);
    assert!(error.to_string().contains("coven agent hooks plan"));
    assert!(error.to_string().contains("coven agent hooks install"));
    assert!(error.to_string().contains("--migrate-kitsune"));
    assert!(error
        .to_string()
        .contains("trust for each new Codex release is user-managed"));
    assert!(error
        .to_string()
        .contains("does not prove operational capture"));
}

#[cfg(target_os = "linux")]
#[test]
fn codex_install_preserves_custom_legacy_config_without_running_commands() {
    use std::os::unix::fs::PermissionsExt;

    let (_lock, home) = setup_home();
    let custom = home.join("custom-codex");
    fs::create_dir_all(&custom).unwrap();
    let fixtures = [
        ("hooks.json", "not JSON; preserve exactly\n"),
        ("config.toml", "not TOML; preserve exactly\n"),
        (CODEX_HOOK_INSTALL_NAME, "legacy hook; preserve exactly\n"),
    ];
    for (name, contents) in fixtures {
        fs::write(custom.join(name), contents).unwrap();
    }
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let marker = home.join("command-ran");
    for name in ["codex", "coven"] {
        let path = bin.join(name);
        fs::write(
            &path,
            format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let original_path = std::env::var_os("PATH");
    std::env::set_var("PATH", &bin);
    let invalid_home = home.join("not-a-directory");
    fs::write(&invalid_home, "not a home\n").unwrap();
    std::env::set_var("HOME", &invalid_home);
    std::env::set_var(CODEX_HOME_ENV_VAR, &custom);

    let error = install_target(crate::api::schema::IntegrationTarget::Codex).unwrap_err();

    assert_eq!(error.to_string(), CODEX_COVEN_GUIDANCE);
    for (name, contents) in fixtures {
        assert_eq!(fs::read_to_string(custom.join(name)).unwrap(), contents);
    }
    assert_eq!(fs::read_dir(&custom).unwrap().count(), fixtures.len());
    assert_eq!(fs::read_to_string(&invalid_home).unwrap(), "not a home\n");
    assert!(!marker.exists());
    assert!(!home.join(".codex").exists());

    if let Some(path) = original_path {
        std::env::set_var("PATH", path);
    } else {
        std::env::remove_var("PATH");
    }
    std::env::set_var("HOME", &home);
    std::env::remove_var(CODEX_HOME_ENV_VAR);
}

#[cfg(target_os = "linux")]
#[test]
fn codex_install_and_update_candidates_exclude_read_only_legacy_status() {
    use crate::api::schema::IntegrationTarget;

    let (_lock, home) = setup_home();
    let custom = home.join("custom-codex");
    fs::create_dir_all(&custom).unwrap();
    std::env::set_var(CODEX_HOME_ENV_VAR, &custom);
    let path = custom.join(CODEX_HOOK_INSTALL_NAME);
    let legacy = "# KITSUNE_INTEGRATION_VERSION=1\n";
    fs::write(&path, legacy).unwrap();
    let pi_dir = home.join(".pi/agent/extensions");
    fs::create_dir_all(&pi_dir).unwrap();
    fs::write(pi_dir.join(PI_EXTENSION_INSTALL_NAME), "// legacy\n").unwrap();

    let statuses = installed_integration_statuses();
    let codex = statuses
        .iter()
        .find(|status| status.target == IntegrationTarget::Codex)
        .unwrap();
    assert_eq!(codex.path, path);
    assert_eq!(codex.state, IntegrationStatusKind::Outdated);
    assert_eq!(codex.installed_version, Some(1));
    assert!(!integration_recommendations()
        .iter()
        .any(|item| item.target == IntegrationTarget::Codex));
    let outdated = outdated_installed_integrations();
    assert!(!outdated
        .iter()
        .any(|item| item.target == IntegrationTarget::Codex));
    assert!(!outdated
        .iter()
        .any(|item| item.target == IntegrationTarget::Pi));
    let instructions =
        integration_update_instructions(&[IntegrationTarget::Codex, IntegrationTarget::Pi]);
    assert!(!instructions.contains("kitsune integration install codex"));
    assert!(instructions.contains(CODEX_COVEN_GUIDANCE));
    assert!(instructions.contains(PI_COVEN_GUIDANCE));
    assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
    std::env::remove_var(CODEX_HOME_ENV_VAR);
}

#[test]
fn legacy_codex_fixture_writes_hook_and_enables_config_hooks() {
    let (_lock, home) = setup_home();
    let codex_dir = home.join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("config.toml"),
        "[features]\ncodex_hooks = true\n",
    )
    .unwrap();

    let installed = legacy_install_codex().unwrap();
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
fn uninstall_codex_removes_legacy_hook_and_preserves_user_config() {
    let (_lock, home) = setup_home();
    let custom = home.join("custom-codex");
    fs::create_dir_all(&custom).unwrap();
    std::env::set_var(CODEX_HOME_ENV_VAR, &custom);
    let installed = legacy_install_codex().unwrap();
    let config = fs::read_to_string(&installed.config_path).unwrap();
    let mut hooks: Value =
        serde_json::from_str(&fs::read_to_string(&installed.hooks_path).unwrap()).unwrap();
    let user_hook = json!({"hooks": [{"type": "command", "command": "user-managed-hook"}]});
    hooks["hooks"]["SessionStart"]
        .as_array_mut()
        .unwrap()
        .push(user_hook.clone());
    fs::write(
        &installed.hooks_path,
        serde_json::to_string(&hooks).unwrap(),
    )
    .unwrap();

    let messages = uninstall_target(crate::api::schema::IntegrationTarget::Codex).unwrap();

    assert!(!installed.hook_path.exists());
    let hooks: Value =
        serde_json::from_str(&fs::read_to_string(&installed.hooks_path).unwrap()).unwrap();
    assert_eq!(hooks["hooks"]["SessionStart"], json!([user_hook]));
    assert_eq!(fs::read_to_string(&installed.config_path).unwrap(), config);
    #[cfg(target_os = "linux")]
    assert!(messages
        .iter()
        .any(|message| message.contains("legacy Kitsune Codex cleanup only")));
    assert!(messages
        .iter()
        .any(|message| message.contains("removed codex integration hook")));
    std::env::remove_var(CODEX_HOME_ENV_VAR);
}

#[test]
fn legacy_copilot_fixture_writes_direct_hook_entry() {
    let (_lock, home) = setup_home();
    let copilot_dir = home.join(".copilot");
    fs::create_dir_all(&copilot_dir).unwrap();

    let installed = legacy_install_copilot().unwrap();
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

#[cfg(target_os = "linux")]
#[test]
fn copilot_install_returns_coven_guidance_without_environment_setup() {
    let error = install_target(crate::api::schema::IntegrationTarget::Copilot).unwrap_err();

    assert_eq!(error.to_string(), COPILOT_COVEN_GUIDANCE);
    assert!(error.to_string().contains("on Linux"));
    assert!(error.to_string().contains("coven agent hooks plan"));
    assert!(error.to_string().contains("coven agent hooks install"));
    assert!(error.to_string().contains("--migrate-kitsune"));
}

#[cfg(target_os = "linux")]
#[test]
fn copilot_install_preserves_custom_legacy_config_without_running_commands() {
    use std::os::unix::fs::PermissionsExt;

    let (_lock, home) = setup_home();
    let custom = home.join("custom-copilot");
    let hooks = custom.join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let settings_path = custom.join("settings.json");
    let hook_path = hooks.join(COPILOT_HOOK_INSTALL_NAME);
    fs::write(&settings_path, "not JSON; preserve exactly\n").unwrap();
    fs::write(&hook_path, "legacy hook; preserve exactly\n").unwrap();
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let marker = home.join("command-ran");
    for name in ["copilot", "coven"] {
        let path = bin.join(name);
        fs::write(
            &path,
            format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let original_path = std::env::var_os("PATH");
    std::env::set_var("PATH", &bin);
    let invalid_home = home.join("not-a-directory");
    fs::write(&invalid_home, "not a home\n").unwrap();
    std::env::set_var("HOME", &invalid_home);
    std::env::set_var(COPILOT_HOME_ENV_VAR, &custom);

    let error = install_target(crate::api::schema::IntegrationTarget::Copilot).unwrap_err();

    assert_eq!(error.to_string(), COPILOT_COVEN_GUIDANCE);
    assert_eq!(
        fs::read_to_string(&settings_path).unwrap(),
        "not JSON; preserve exactly\n"
    );
    assert_eq!(
        fs::read_to_string(&hook_path).unwrap(),
        "legacy hook; preserve exactly\n"
    );
    assert_eq!(fs::read_dir(&custom).unwrap().count(), 2);
    assert_eq!(fs::read_dir(&hooks).unwrap().count(), 1);
    assert_eq!(fs::read_to_string(&invalid_home).unwrap(), "not a home\n");
    assert!(!marker.exists());
    assert!(!home.join(".copilot").exists());

    if let Some(path) = original_path {
        std::env::set_var("PATH", path);
    } else {
        std::env::remove_var("PATH");
    }
    std::env::set_var("HOME", &home);
    std::env::remove_var(COPILOT_HOME_ENV_VAR);
}

#[test]
fn copilot_candidates_follow_platform_policy_and_preserve_legacy_status() {
    use crate::api::schema::IntegrationTarget;

    let (_lock, home) = setup_home();
    let custom = home.join("custom-copilot");
    let hooks = custom.join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    std::env::set_var(COPILOT_HOME_ENV_VAR, &custom);
    let path = hooks.join(COPILOT_HOOK_INSTALL_NAME);
    let legacy = "# KITSUNE_INTEGRATION_VERSION=1\n";
    fs::write(&path, legacy).unwrap();
    let pi_dir = home.join(".pi/agent/extensions");
    fs::create_dir_all(&pi_dir).unwrap();
    fs::write(pi_dir.join(PI_EXTENSION_INSTALL_NAME), "// legacy\n").unwrap();

    let statuses = installed_integration_statuses();
    let copilot = statuses
        .iter()
        .find(|status| status.target == IntegrationTarget::Copilot)
        .unwrap();
    assert_eq!(copilot.path, path);
    assert_eq!(copilot.state, IntegrationStatusKind::Outdated);
    assert_eq!(copilot.installed_version, Some(1));
    let recommendations = integration_recommendations();
    let bulk_candidates: Vec<_> = recommendations
        .iter()
        .filter(|item| item.needs_install())
        .map(|item| item.target)
        .collect();
    let outdated = outdated_installed_integrations();
    let legacy_installable = !cfg!(target_os = "linux");
    assert_eq!(
        recommendations
            .iter()
            .any(|item| item.target == IntegrationTarget::Copilot),
        legacy_installable
    );
    assert_eq!(
        bulk_candidates.contains(&IntegrationTarget::Copilot),
        legacy_installable
    );
    assert_eq!(
        outdated
            .iter()
            .any(|item| item.target == IntegrationTarget::Copilot),
        legacy_installable
    );
    assert_eq!(
        bulk_candidates.contains(&IntegrationTarget::Pi),
        legacy_installable
    );
    assert_eq!(
        outdated
            .iter()
            .any(|item| item.target == IntegrationTarget::Pi),
        legacy_installable
    );
    let instructions =
        integration_update_instructions(&[IntegrationTarget::Copilot, IntegrationTarget::Pi]);
    assert_eq!(
        instructions.contains("kitsune integration install copilot"),
        legacy_installable
    );
    #[cfg(target_os = "linux")]
    assert!(instructions.contains(COPILOT_COVEN_GUIDANCE));
    assert_eq!(
        instructions.contains("kitsune integration install pi"),
        legacy_installable
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
    std::env::remove_var(COPILOT_HOME_ENV_VAR);
}

#[test]
fn uninstall_copilot_removes_hook_and_owned_settings_entry() {
    let (_lock, home) = setup_home();
    let copilot_dir = home.join(".copilot");
    fs::create_dir_all(&copilot_dir).unwrap();
    let installed = legacy_install_copilot().unwrap();

    let mut settings: Value =
        serde_json::from_str(&fs::read_to_string(&installed.settings_path).unwrap()).unwrap();
    let user_hook = json!({"type": "command", direct_command_field(): "user-managed-hook"});
    settings["hooks"]["SessionStart"]
        .as_array_mut()
        .unwrap()
        .push(user_hook.clone());
    fs::write(
        &installed.settings_path,
        serde_json::to_string(&settings).unwrap(),
    )
    .unwrap();

    let messages = uninstall_target(crate::api::schema::IntegrationTarget::Copilot).unwrap();
    let settings: Value =
        serde_json::from_str(&fs::read_to_string(&installed.settings_path).unwrap()).unwrap();

    assert!(!installed.hook_path.exists());
    assert_eq!(settings["hooks"]["SessionStart"], json!([user_hook]));
    #[cfg(target_os = "linux")]
    assert!(messages
        .iter()
        .any(|message| message.contains("legacy Kitsune Copilot cleanup only")));
    assert!(messages
        .iter()
        .any(|message| message.contains("removed copilot integration hook")));
}

#[test]
fn legacy_install_opencode_writes_plugin() {
    let (_lock, home) = setup_home();
    let opencode_dir = home.join(".config/opencode");
    fs::create_dir_all(&opencode_dir).unwrap();

    let installed = legacy_install_opencode().unwrap();

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

#[cfg(target_os = "linux")]
#[test]
fn opencode_install_returns_coven_guidance_without_environment_setup() {
    let error = install_target(crate::api::schema::IntegrationTarget::Opencode).unwrap_err();
    assert_eq!(error.to_string(), OPENCODE_COVEN_GUIDANCE);
}

#[cfg(target_os = "linux")]
#[test]
fn opencode_install_preserves_custom_root_without_running_commands() {
    use std::os::unix::fs::PermissionsExt;

    let (_lock, home) = setup_home();
    let config = home.join("custom-config");
    let root = config.join("opencode/plugins");
    fs::create_dir_all(&root).unwrap();
    let legacy = root.join(OPENCODE_PLUGIN_INSTALL_NAME);
    fs::write(&legacy, "custom legacy; preserve exactly\n").unwrap();
    let json = config.join("opencode/opencode.json");
    fs::write(&json, "invalid JSON; preserve exactly\n").unwrap();
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let marker = home.join("command-ran");
    for name in ["opencode", "coven"] {
        let command = bin.join(name);
        fs::write(&command, format!("#!/bin/sh\n: > '{}'\n", marker.display())).unwrap();
        fs::set_permissions(command, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let original_path = std::env::var_os("PATH");
    std::env::set_var("PATH", &bin);
    let invalid_home = home.join("not-a-directory");
    fs::write(&invalid_home, "not a home\n").unwrap();
    std::env::set_var("HOME", &invalid_home);
    std::env::set_var("XDG_CONFIG_HOME", &config);

    let error = install_target(crate::api::schema::IntegrationTarget::Opencode).unwrap_err();
    assert_eq!(error.to_string(), OPENCODE_COVEN_GUIDANCE);
    assert_eq!(
        fs::read_to_string(&legacy).unwrap(),
        "custom legacy; preserve exactly\n"
    );
    assert_eq!(
        fs::read_to_string(json).unwrap(),
        "invalid JSON; preserve exactly\n"
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    assert_eq!(fs::read_to_string(invalid_home).unwrap(), "not a home\n");
    assert!(!marker.exists());
    assert!(!home.join(".config").exists());
    if let Some(path) = original_path {
        std::env::set_var("PATH", path);
    } else {
        std::env::remove_var("PATH");
    }
    std::env::set_var("HOME", &home);
    std::env::remove_var("XDG_CONFIG_HOME");
}

#[test]
fn opencode_candidates_follow_platform_policy_and_preserve_legacy_status() {
    use crate::api::schema::IntegrationTarget;

    let (_lock, home) = setup_home();
    let dir = home.join(".config/opencode/plugins");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(OPENCODE_PLUGIN_INSTALL_NAME);
    let legacy = "// KITSUNE_INTEGRATION_VERSION=1\n";
    fs::write(&path, legacy).unwrap();
    let status = installed_integration_statuses()
        .into_iter()
        .find(|status| status.target == IntegrationTarget::Opencode)
        .unwrap();
    assert_eq!(status.path, path);
    assert_eq!(status.state, IntegrationStatusKind::Outdated);
    assert_eq!(status.installed_version, Some(1));
    let legacy_installable = !cfg!(target_os = "linux");
    let recommendations = integration_recommendations();
    assert_eq!(
        recommendations
            .iter()
            .any(|item| item.target == IntegrationTarget::Opencode),
        legacy_installable
    );
    assert_eq!(
        recommendations
            .iter()
            .any(|item| item.target == IntegrationTarget::Opencode && item.needs_install()),
        legacy_installable
    );
    let outdated = outdated_installed_integrations();
    assert_eq!(
        outdated
            .iter()
            .any(|item| item.target == IntegrationTarget::Opencode),
        legacy_installable
    );
    #[cfg(target_os = "linux")]
    {
        assert!(recommendations.is_empty());
        assert!(outdated.is_empty());
        assert!(!print_outdated_update_notice());
    }
    let instructions = integration_update_instructions(&[IntegrationTarget::Opencode]);
    assert_eq!(
        instructions.contains("kitsune integration install opencode"),
        legacy_installable
    );
    #[cfg(target_os = "linux")]
    assert!(instructions.contains(OPENCODE_COVEN_GUIDANCE));
    assert_eq!(fs::read_to_string(path).unwrap(), legacy);
}

#[cfg(not(target_os = "linux"))]
#[test]
fn opencode_install_target_retains_legacy_result_path() {
    let (_lock, home) = setup_home();
    let dir = home.join(".config/opencode");
    fs::create_dir_all(&dir).unwrap();
    let messages = install_target(crate::api::schema::IntegrationTarget::Opencode).unwrap();
    let path = dir.join("plugins").join(OPENCODE_PLUGIN_INSTALL_NAME);
    assert_eq!(
        messages,
        vec![format!(
            "installed opencode integration plugin to {}",
            path.display()
        )]
    );
    assert_eq!(fs::read_to_string(path).unwrap(), OPENCODE_PLUGIN_ASSET);
}

#[test]
fn uninstall_opencode_removes_plugin() {
    let (_lock, home) = setup_home();
    let opencode_dir = home.join(".config/opencode/plugins");
    fs::create_dir_all(&opencode_dir).unwrap();
    let plugin_path = opencode_dir.join(OPENCODE_PLUGIN_INSTALL_NAME);
    fs::write(&plugin_path, OPENCODE_PLUGIN_ASSET).unwrap();
    let coven_path = opencode_dir.join("coven-agent-reference.js");
    fs::write(&coven_path, "Coven-owned; preserve exactly\n").unwrap();

    let result = uninstall_opencode().unwrap();

    assert!(result.removed_plugin);
    assert_eq!(result.plugin_path, plugin_path);
    assert!(!result.plugin_path.exists());
    assert_eq!(
        fs::read_to_string(coven_path).unwrap(),
        "Coven-owned; preserve exactly\n"
    );
}

#[test]
fn djinn_install_and_uninstall_are_built_in_noops() {
    let messages = install_target(crate::api::schema::IntegrationTarget::Djinn).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("built in"));

    let messages = uninstall_target(crate::api::schema::IntegrationTarget::Djinn).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("built in"));
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
