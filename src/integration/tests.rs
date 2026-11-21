use super::command::*;
use super::config_edit::*;
use super::env::*;
use super::registry::*;
use super::targets::*;
use super::*;

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

struct Fixture {
    _lock: std::sync::MutexGuard<'static, ()>,
    home: PathBuf,
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl Fixture {
    fn new() -> Self {
        let lock = integration_env_lock();
        let home =
            std::env::temp_dir().join(format!("kitsune-legacy-fixture-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&home).unwrap();
        let saved = [
            "HOME",
            "USERPROFILE",
            "HOMEDRIVE",
            "HOMEPATH",
            "PATH",
            "XDG_CONFIG_HOME",
            PI_CODING_AGENT_DIR_ENV_VAR,
            CLAUDE_CONFIG_DIR_ENV_VAR,
            CODEX_HOME_ENV_VAR,
            COPILOT_HOME_ENV_VAR,
        ]
        .into_iter()
        .map(|key| {
            let value = std::env::var_os(key);
            std::env::remove_var(key);
            (key, value)
        })
        .collect();
        std::env::set_var("HOME", &home);
        Self {
            _lock: lock,
            home,
            saved,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        if let Err(error) = fs::remove_dir_all(&self.home) {
            eprintln!("failed to clean fixture {}: {error}", self.home.display());
            assert!(std::thread::panicking(), "fixture cleanup failed");
        }
    }
}

#[test]
fn every_target_install_is_retired_without_home_or_path() {
    let _fixture = Fixture::new();
    std::env::remove_var("HOME");
    for target in crate::api::schema::IntegrationTarget::ALL {
        assert_eq!(
            install_target(target).unwrap_err().to_string(),
            INSTALL_RETIRED_GUIDANCE
        );
    }
}

#[test]
fn all_platform_install_source_has_no_discovery_or_fresh_writer_branches() {
    let actions = include_str!("actions.rs");
    let install = actions
        .split("pub(crate) fn uninstall_target")
        .next()
        .unwrap();
    assert!(install.contains("Err(io::Error::other(super::INSTALL_RETIRED_GUIDANCE))"));
    for forbidden in [
        "#[cfg",
        "install_target_inner",
        "version",
        "which",
        "fs::",
        "logging::",
    ] {
        assert!(
            !install.contains(forbidden),
            "install boundary contains {forbidden}"
        );
    }
    for source in [include_str!("targets.rs"), include_str!("config_edit.rs")] {
        for forbidden in [
            "legacy_install_",
            "ensure_command_hook",
            "ensure_direct_command_hook",
            "ensure_hooks_object",
            "hooks = true",
            "create_dir",
            "make_executable",
        ] {
            assert!(
                !source.contains(forbidden),
                "fresh writer remains: {forbidden}"
            );
        }
    }
}

#[test]
fn every_target_refuses_before_executable_config_or_filesystem_access() {
    let fixture = Fixture::new();
    let bin = fixture.home.join("bin");
    fs::create_dir(&bin).unwrap();
    let marker = fixture.home.join("executed");
    for name in [
        "pi", "claude", "codex", "copilot", "opencode", "djinn", "coven",
    ] {
        let (filename, body) = if cfg!(windows) {
            (
                format!("{name}.cmd"),
                format!(
                    "@echo off\r\necho ran > \"{}\"\r\nexit /b 99\r\n",
                    marker.display()
                ),
            )
        } else {
            (
                name.to_string(),
                format!("#!/bin/sh\n: > '{}'\nexit 99\n", marker.display()),
            )
        };
        let path = bin.join(filename);
        fs::write(&path, body).unwrap();
        fs::set_permissions(
            path,
            fs::metadata(std::env::current_exe().unwrap())
                .unwrap()
                .permissions(),
        )
        .unwrap();
    }
    std::env::set_var("PATH", &bin);
    let invalid_home = fixture.home.join("not-a-directory");
    fs::write(&invalid_home, "not a home\n").unwrap();
    std::env::set_var("HOME", &invalid_home);
    let config = fixture.home.join("legacy");
    fs::create_dir(&config).unwrap();
    let preimages = [
        ("settings.json", "not JSON; preserve exactly\n"),
        ("hooks.json", "invalid hooks; preserve exactly\n"),
        (
            "config.toml",
            "# user trust flags\n[features]\nhooks = false\n",
        ),
        ("kitsune-agent-state.sh", "legacy; preserve exactly\n"),
    ];
    for (name, content) in preimages {
        fs::write(config.join(name), content).unwrap();
    }
    for key in [
        PI_CODING_AGENT_DIR_ENV_VAR,
        CLAUDE_CONFIG_DIR_ENV_VAR,
        CODEX_HOME_ENV_VAR,
        COPILOT_HOME_ENV_VAR,
        "XDG_CONFIG_HOME",
    ] {
        std::env::set_var(key, &config);
    }
    for target in crate::api::schema::IntegrationTarget::ALL {
        assert_eq!(
            install_target(target).unwrap_err().to_string(),
            INSTALL_RETIRED_GUIDANCE
        );
    }
    // Explicit legacy status may inspect paths, but never runs provider version probes.
    let _ = installed_integration_statuses();
    assert!(!marker.exists());
    assert_eq!(fs::read_dir(&fixture.home).unwrap().count(), 3);
    assert_eq!(fs::read_dir(&config).unwrap().count(), preimages.len());
    assert_eq!(fs::read_to_string(invalid_home).unwrap(), "not a home\n");
    for (name, content) in preimages {
        assert_eq!(fs::read_to_string(config.join(name)).unwrap(), content);
    }
}

#[test]
fn integration_target_labels_match_all_six_compatibility_targets() {
    let targets = crate::api::schema::IntegrationTarget::ALL;
    assert_eq!(targets.len(), 6);
    for (target, label) in targets
        .into_iter()
        .zip(["pi", "claude", "codex", "copilot", "opencode", "djinn"])
    {
        assert_eq!(integration_target_label(target), label);
        assert!(integration_target_supported(target));
    }
}

#[test]
fn legacy_status_is_read_only_and_never_recommends_install_or_update_on_any_platform() {
    let fixture = Fixture::new();
    let paths = [
        fixture
            .home
            .join(".pi/agent/extensions")
            .join(PI_EXTENSION_INSTALL_NAME),
        fixture
            .home
            .join(".claude/hooks")
            .join(CLAUDE_HOOK_INSTALL_NAME),
        fixture.home.join(".codex").join(CODEX_HOOK_INSTALL_NAME),
        fixture
            .home
            .join(".copilot/hooks")
            .join(COPILOT_HOOK_INSTALL_NAME),
        fixture
            .home
            .join(".config/opencode/plugins")
            .join(OPENCODE_PLUGIN_INSTALL_NAME),
    ];
    for path in &paths {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "// KITSUNE_INTEGRATION_VERSION=1\n").unwrap();
    }
    let statuses = installed_integration_statuses();
    assert_eq!(statuses.len(), 5);
    for (status, path) in statuses.into_iter().zip(paths) {
        assert_eq!(status.path, path);
        assert_eq!(status.state, IntegrationStatusKind::Outdated);
        assert_eq!(status.installed_version, Some(1));
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "// KITSUNE_INTEGRATION_VERSION=1\n"
        );
    }
    let registry = include_str!("registry.rs");
    for removed in [
        "integration_recommendations",
        "outdated_installed_integrations",
        "integration_update_instructions",
        "print_outdated_update_notice",
    ] {
        assert!(
            !registry.contains(removed),
            "retired recommendation policy remains: {removed}"
        );
    }
}

#[test]
fn custom_legacy_roots_remain_recognized_for_cleanup_status() {
    let fixture = Fixture::new();
    for (env, relative, name) in [
        (
            PI_CODING_AGENT_DIR_ENV_VAR,
            "extensions",
            PI_EXTENSION_INSTALL_NAME,
        ),
        (CLAUDE_CONFIG_DIR_ENV_VAR, "hooks", CLAUDE_HOOK_INSTALL_NAME),
        (CODEX_HOME_ENV_VAR, "", CODEX_HOOK_INSTALL_NAME),
        (COPILOT_HOME_ENV_VAR, "hooks", COPILOT_HOOK_INSTALL_NAME),
    ] {
        let root = fixture.home.join(env);
        std::env::set_var(env, &root);
        let path = root.join(relative).join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "# KITSUNE_INTEGRATION_VERSION=1\n").unwrap();
        assert!(installed_integration_statuses()
            .iter()
            .any(|status| status.path == path && status.installed_version == Some(1)));
    }
}

#[test]
fn uninstall_pi_removes_frozen_legacy_extension_only() {
    let fixture = Fixture::new();
    let dir = fixture.home.join(".pi/agent/extensions");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(PI_EXTENSION_INSTALL_NAME);
    fs::write(&path, PI_EXTENSION_ASSET).unwrap();
    let other = dir.join("coven-agent-reference.ts");
    fs::write(&other, "user-owned; preserve exactly\n").unwrap();
    let result = uninstall_pi().unwrap();
    assert!(result.removed_extension);
    assert_eq!(result.extension_path, path);
    assert!(!path.exists());
    assert_eq!(
        fs::read_to_string(other).unwrap(),
        "user-owned; preserve exactly\n"
    );
    assert!(!uninstall_pi().unwrap().removed_extension);
}

#[test]
fn uninstall_claude_removes_frozen_hook_and_owned_entry_preserving_user_settings() {
    let fixture = Fixture::new();
    let dir = fixture.home.join(".claude");
    fs::create_dir_all(dir.join("hooks")).unwrap();
    let hook = dir.join("hooks").join(CLAUDE_HOOK_INSTALL_NAME);
    fs::write(&hook, CLAUDE_HOOK_ASSET).unwrap();
    let user_hook = json!({"type": "command", "command": "user-managed-hook", "timeout": 20});
    let settings = json!({
        "permissions": {"allow": ["user permission"]},
        "hooks": {"SessionStart": [{"matcher": "*", "hooks": [
            {"type": "command", "command": hook_command(&hook, Some("session")), "timeout": 10},
            user_hook.clone()
        ]}]}
    });
    fs::write(
        dir.join("settings.json"),
        serde_json::to_string(&settings).unwrap(),
    )
    .unwrap();
    let result = uninstall_claude().unwrap();
    assert!(result.removed_hook_file);
    assert!(result.updated_settings);
    assert_eq!(result.hook_path, hook);
    assert!(!hook.exists());
    let settings: Value =
        serde_json::from_str(&fs::read_to_string(result.settings_path).unwrap()).unwrap();
    assert_eq!(
        settings["permissions"],
        json!({"allow": ["user permission"]})
    );
    assert_eq!(
        settings["hooks"]["SessionStart"][0]["hooks"],
        json!([user_hook])
    );
    assert!(!uninstall_claude().unwrap().updated_settings);
}

#[test]
fn uninstall_codex_removes_frozen_hook_preserving_user_hooks_config_and_comments() {
    let fixture = Fixture::new();
    let dir = fixture.home.join("custom-codex");
    fs::create_dir(&dir).unwrap();
    std::env::set_var(CODEX_HOME_ENV_VAR, &dir);
    let hook = dir.join(CODEX_HOOK_INSTALL_NAME);
    fs::write(&hook, CODEX_HOOK_ASSET).unwrap();
    let config = "# keep my trust decisions\nmodel = 'user-model'\n[features]\nhooks = false # user disabled\ncodex_hooks = true\n";
    fs::write(dir.join("config.toml"), config).unwrap();
    let user_hook = json!({"hooks": [{"type": "command", "command": "user-managed-hook"}]});
    fs::write(dir.join("hooks.json"), serde_json::to_string(&json!({"hooks": {"SessionStart": [
        {"hooks": [{"type": "command", "command": hook_command(&hook, Some("session")), "timeout": 10}]}, user_hook.clone()
    ]}})).unwrap()).unwrap();
    let messages = uninstall_target(crate::api::schema::IntegrationTarget::Codex).unwrap();
    assert!(messages
        .iter()
        .any(|message| message.contains("removed codex integration hook")));
    assert!(!hook.exists());
    let hooks: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("hooks.json")).unwrap()).unwrap();
    assert_eq!(hooks["hooks"]["SessionStart"], json!([user_hook]));
    assert_eq!(fs::read_to_string(dir.join("config.toml")).unwrap(), config);
    let result = uninstall_codex().unwrap();
    assert_eq!(result.config_path, dir.join("config.toml"));
    assert!(!result.updated_hooks);
    assert!(!result.removed_hook_file);
}

#[test]
fn uninstall_copilot_removes_frozen_direct_hook_preserving_other_fields_and_handlers() {
    let fixture = Fixture::new();
    let dir = fixture.home.join(".copilot");
    fs::create_dir_all(dir.join("hooks")).unwrap();
    let hook = dir.join("hooks").join(COPILOT_HOOK_INSTALL_NAME);
    fs::write(&hook, COPILOT_HOOK_ASSET).unwrap();
    let command_field = if cfg!(windows) { "powershell" } else { "bash" };
    let user_hook = json!({"type": "command", command_field: "user-managed-hook"});
    fs::write(
        dir.join("settings.json"),
        serde_json::to_string(&json!({
            "userSetting": {"trusted": false},
            "hooks": {"SessionStart": [
                {"type": "command", command_field: hook_command(&hook, None), "timeoutSec": 10},
                user_hook.clone()
            ]}
        }))
        .unwrap(),
    )
    .unwrap();
    let messages = uninstall_target(crate::api::schema::IntegrationTarget::Copilot).unwrap();
    assert!(messages
        .iter()
        .any(|message| message.contains("removed copilot integration hook")));
    assert!(!hook.exists());
    let settings: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["hooks"]["SessionStart"], json!([user_hook]));
    assert_eq!(settings["userSetting"], json!({"trusted": false}));
    assert!(!uninstall_copilot().unwrap().updated_settings);
}

#[test]
fn uninstall_opencode_removes_frozen_legacy_plugin_only() {
    let fixture = Fixture::new();
    let dir = fixture.home.join(".config/opencode/plugins");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(OPENCODE_PLUGIN_INSTALL_NAME);
    fs::write(&path, OPENCODE_PLUGIN_ASSET).unwrap();
    let other = dir.join("coven-agent-reference.js");
    fs::write(&other, "user-owned; preserve exactly\n").unwrap();
    let result = uninstall_opencode().unwrap();
    assert!(result.removed_plugin);
    assert_eq!(result.plugin_path, path);
    assert!(!path.exists());
    assert_eq!(
        fs::read_to_string(other).unwrap(),
        "user-owned; preserve exactly\n"
    );
    assert!(!uninstall_opencode().unwrap().removed_plugin);
}

#[test]
fn djinn_cleanup_remains_recognized_without_install_success() {
    let error = install_target(crate::api::schema::IntegrationTarget::Djinn).unwrap_err();
    assert!(error
        .to_string()
        .starts_with("feature_retired: integration.install:"));
    let messages = uninstall_target(crate::api::schema::IntegrationTarget::Djinn).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("no external files were removed"));
}

#[test]
fn integration_status_uses_version_marker() {
    let fixture = Fixture::new();
    let path = fixture.home.join("integration.js");
    fs::write(&path, "// KITSUNE_INTEGRATION_VERSION=9\n").unwrap();
    let status = integration_status_at(crate::api::schema::IntegrationTarget::Opencode, path, 9);
    assert_eq!(status.state, IntegrationStatusKind::Current);
    assert_eq!(status.installed_version, Some(9));
}

#[test]
fn integration_status_reports_outdated_missing_marker() {
    let fixture = Fixture::new();
    let path = fixture.home.join("integration.js");
    fs::write(&path, "// no marker\n").unwrap();
    let status = integration_status_at(crate::api::schema::IntegrationTarget::Opencode, path, 9);
    assert_eq!(status.state, IntegrationStatusKind::Outdated);
    assert_eq!(status.installed_version, None);
}

#[test]
fn cleanup_without_owned_entries_preserves_exact_settings_bytes() {
    let fixture = Fixture::new();
    let dir = fixture.home.join(".claude");
    fs::create_dir_all(&dir).unwrap();
    let contents = "{\n  \"user\": true, \"hooks\": {}\n}\n";
    fs::write(dir.join("settings.json"), contents).unwrap();
    assert!(!uninstall_claude().unwrap().updated_settings);
    assert_eq!(
        fs::read_to_string(dir.join("settings.json")).unwrap(),
        contents
    );
}

#[test]
fn cleanup_rejects_malformed_settings_without_removing_hook() {
    let fixture = Fixture::new();
    let dir = fixture.home.join(".claude");
    fs::create_dir_all(dir.join("hooks")).unwrap();
    let hook = dir.join("hooks").join(CLAUDE_HOOK_INSTALL_NAME);
    fs::write(&hook, CLAUDE_HOOK_ASSET).unwrap();
    fs::write(dir.join("settings.json"), "invalid user JSON\n").unwrap();
    assert!(uninstall_claude().is_err());
    assert_eq!(fs::read_to_string(hook).unwrap(), CLAUDE_HOOK_ASSET);
    assert_eq!(
        fs::read_to_string(dir.join("settings.json")).unwrap(),
        "invalid user JSON\n"
    );
}

#[test]
fn cleanup_recognizes_legacy_direct_command_fields_without_affecting_other_entries() {
    let path = PathBuf::from("legacy-hook.sh");
    for field in ["command", "bash", "powershell"] {
        let mut hooks = json!({"SessionStart": [
            {"type": "command", field: hook_command(&path, None)},
            {"type": "command", field: "user-hook"},
            {"type": "prompt", field: hook_command(&path, None)}
        ]})
        .as_object()
        .unwrap()
        .clone();
        assert!(remove_direct_hook_commands(&mut hooks, "SessionStart", &path, None).unwrap());
        assert_eq!(hooks["SessionStart"].as_array().unwrap().len(), 2);
        assert_eq!(hooks["SessionStart"][0][field], "user-hook");
    }
}
