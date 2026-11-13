use std::path::{Path, PathBuf};

use tracing::warn;

use super::{
    model::{AnnotationsConfig, LoadedConfig},
    Config, IntegrationsConfig, CONFIG_PATH_ENV_VAR,
};

const KNOWN_TOP_LEVEL_CONFIG_KEYS: &[&str] = &[
    "advanced",
    "agent_detection",
    "annotations",
    "integrations",
    "experimental",
    "keys",
    "onboarding",
    "remote",
    "session",
    "terminal",
    "theme",
    "ui",
    "update",
    "worktrees",
];

pub fn app_dir_name() -> &'static str {
    if cfg!(debug_assertions) {
        crate::product::DEV_CONFIG_DIR_NAME
    } else {
        crate::product::CONFIG_DIR_NAME
    }
}

pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join(app_dir_name());
    }
    platform_config_dir()
}

fn config_file_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join(crate::product::CONFIG_DIR_NAME);
    }
    platform_config_file_dir()
}

#[cfg(windows)]
fn platform_config_file_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("APPDATA") {
        return PathBuf::from(dir).join(crate::product::CONFIG_DIR_NAME);
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile)
            .join("AppData")
            .join("Roaming")
            .join(crate::product::CONFIG_DIR_NAME);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(format!(".config/{}", crate::product::CONFIG_DIR_NAME));
    }
    std::env::temp_dir().join(crate::product::CONFIG_DIR_NAME)
}

#[cfg(not(windows))]
fn platform_config_file_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(format!(".config/{}", crate::product::CONFIG_DIR_NAME))
    } else {
        std::env::temp_dir().join(crate::product::CONFIG_DIR_NAME)
    }
}

pub fn state_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
        return PathBuf::from(dir).join(app_dir_name());
    }
    platform_state_dir()
}

#[cfg(windows)]
fn platform_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("APPDATA") {
        return PathBuf::from(dir).join(app_dir_name());
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile)
            .join("AppData")
            .join("Roaming")
            .join(app_dir_name());
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(format!(".config/{}", app_dir_name()));
    }
    std::env::temp_dir().join(app_dir_name())
}

#[cfg(not(windows))]
fn platform_config_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(format!(".config/{}", app_dir_name()))
    } else {
        std::env::temp_dir().join(app_dir_name())
    }
}

#[cfg(windows)]
fn platform_state_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(dir).join(app_dir_name());
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile)
            .join("AppData")
            .join("Local")
            .join(app_dir_name());
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(format!(".local/state/{}", app_dir_name()));
    }
    std::env::temp_dir().join(format!("{}-state", app_dir_name()))
}

#[cfg(not(windows))]
fn platform_state_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(format!(".local/state/{}", app_dir_name()))
    } else {
        std::env::temp_dir().join(format!("{}-state", app_dir_name()))
    }
}

fn read_optional_config(path: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

impl Config {
    pub fn load() -> LoadedConfig {
        let path = config_path();
        let content = match read_optional_config(&path) {
            Ok(Some(content)) => content,
            Ok(None) => {
                return LoadedConfig {
                    config: Self::default(),
                    diagnostics: Vec::new(),
                    invalid_sections: Vec::new(),
                };
            }
            Err(err) => {
                warn!(err = %err, "config read error, using defaults");
                let mut config = Self::default();
                config.integrations.external_agent_management_requested = true;
                return LoadedConfig {
                    config,
                    diagnostics: vec![format!("config read error: {err}; using defaults; external agent ownership retained fail-closed")],
                    invalid_sections: Vec::new(),
                };
            }
        };

        let external_agent_management_requested = startup_external_agent_management(&content);
        let mut optional_section_errors = Vec::new();
        let parsed = deserialize_with_ignored::<Config, _>(toml::Deserializer::new(&content))
            .or_else(|err| {
                let Ok(mut value) = content.parse::<toml::Value>() else {
                    return Err(err);
                };
                let Some(table) = value.as_table_mut() else {
                    return Err(err);
                };
                if let Some(annotations) = table.get("annotations") {
                    if let Err(section_err) =
                        deserialize_with_ignored::<AnnotationsConfig, _>(annotations.clone())
                    {
                        optional_section_errors.push(("annotations", section_err.to_string()));
                        table.remove("annotations");
                    }
                }
                if let Some(integrations) = table.get("integrations") {
                    if let Err(section_err) =
                        deserialize_with_ignored::<IntegrationsConfig, _>(integrations.clone())
                    {
                        optional_section_errors.push(("integrations", section_err.to_string()));
                        table.remove("integrations");
                    }
                }
                if optional_section_errors.is_empty() {
                    return Err(err);
                }
                // Isolate optional IPC sections; unrelated startup errors still use defaults.
                let parsed = deserialize_with_ignored::<Config, _>(value).map_err(|_| err)?;
                Ok(parsed)
            });
        match parsed {
            Ok((mut config, ignored_keys)) => {
                config.integrations.external_agent_management_requested =
                    external_agent_management_requested;
                let (unknown_sections, mut diagnostics) =
                    unknown_top_level_sections_from_str(&content);
                diagnostics.extend(unknown_config_key_diagnostics(
                    ignored_keys
                        .into_iter()
                        .filter(|path| {
                            !matches!(path.as_slice(), [ConfigKeyPathSegment::Key(key)] if unknown_sections.contains(key))
                        })
                        .collect(),
                    None,
                ));
                diagnostics.extend(config.collect_diagnostics());
                let mut invalid_sections = Vec::new();
                for (section, err) in optional_section_errors {
                    let feature = if section == "integrations" {
                        "integrations"
                    } else {
                        "annotations"
                    };
                    warn!(section, err = %err, "invalid optional config section; feature disabled");
                    diagnostics.push(format!(
                        "invalid {section} config: {err}; {feature} disabled"
                    ));
                    invalid_sections.push(section.into());
                }
                if config.invalid_integrations_diagnostic().is_some() {
                    config.integrations.sources.clear();
                    config.integrations.runtime = None;
                    invalid_sections.push("integrations".into());
                }
                if external_agent_management_requested && config.integrations.runtime.is_none() {
                    diagnostics.push("integration reporting disabled; external agent ownership retained fail-closed".into());
                }
                LoadedConfig {
                    config,
                    diagnostics,
                    invalid_sections,
                }
            }
            Err(err) => {
                warn!(err = %err, "config parse error, using defaults");
                let mut config = Self::default();
                config.integrations.external_agent_management_requested =
                    external_agent_management_requested;
                let mut diagnostics = vec![format!("config parse error: {err}; using defaults")];
                if external_agent_management_requested {
                    diagnostics.push("integration reporting disabled; external agent ownership retained fail-closed".into());
                }
                LoadedConfig {
                    config,
                    diagnostics,
                    invalid_sections: Vec::new(),
                }
            }
        }
    }
}

fn startup_external_agent_management(content: &str) -> bool {
    let Ok(value) = content.parse::<toml::Value>() else {
        // An unreadable ownership request cannot authorize legacy execution.
        return true;
    };
    let Some(runtime) = value
        .get("integrations")
        .and_then(|integrations| integrations.get("runtime"))
    else {
        return false;
    };
    runtime
        .clone()
        .try_into::<super::IntegrationRuntimeConfig>()
        .map(|runtime| runtime.external_agent_management)
        .unwrap_or(true)
}

pub(super) fn resolve_config_relative_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    config_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(path)
}

pub fn config_path() -> PathBuf {
    if let Ok(path) = std::env::var(CONFIG_PATH_ENV_VAR) {
        return PathBuf::from(path);
    }
    config_file_dir().join("config.toml")
}

pub fn config_diagnostic_summary(diagnostics: &[String]) -> Option<String> {
    if diagnostics.is_empty() {
        return None;
    }

    let target = config_path()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml")
        .to_string();
    let read_error = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("config read error:"));
    let impact = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("using defaults"))
    {
        if read_error {
            " unreadable; using defaults"
        } else {
            " invalid; using defaults"
        }
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("keeping current config"))
    {
        if read_error {
            " unreadable; keeping current config"
        } else {
            " invalid; keeping current config"
        }
    } else if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.starts_with("unknown config key "))
    {
        " has unknown keys"
    } else {
        ""
    };

    Some(format!("{target}{impact}; kitsune config check"))
}

pub fn load_live_config() -> Result<LoadedConfig, Vec<String>> {
    let path = config_path();
    let content = match read_optional_config(&path) {
        Ok(Some(content)) => content,
        Ok(None) => {
            return load_live_config_from_str("");
        }
        Err(err) => {
            return Err(vec![format!(
                "config read error: {err}; keeping current config"
            )]);
        }
    };
    load_live_config_from_str(&content)
}

fn load_live_config_from_str(content: &str) -> Result<LoadedConfig, Vec<String>> {
    let value = content
        .parse::<toml::Value>()
        .map_err(|err| vec![format!("config parse error: {err}; keeping current config")])?;
    let table = value.as_table().ok_or_else(|| {
        vec![
            "config parse error: top-level config must be a table; keeping current config"
                .to_string(),
        ]
    })?;

    let mut config = Config::default();
    let mut diagnostics = unknown_top_level_section_diagnostics(table);
    diagnostics.extend(unknown_top_level_config_key_diagnostics(table));
    let mut invalid_sections = Vec::new();

    load_live_section(
        table,
        "annotations",
        "annotations config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.annotations = section,
    );
    if crate::annotations::validate_sources(&config.annotations.sources).is_err() {
        diagnostics
            .push("invalid annotations.sources; keeping previous [annotations] settings".into());
        invalid_sections.push("annotations".into());
    }

    load_live_section(
        table,
        "integrations",
        "integrations config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.integrations = section,
    );
    config.integrations.external_agent_management_requested =
        startup_external_agent_management(content);
    if let Some(diagnostic) = config.invalid_integrations_diagnostic() {
        diagnostics.push(format!(
            "{diagnostic}; keeping previous [integrations] settings"
        ));
        invalid_sections.push("integrations".into());
    }
    if let Some(startup) = super::STARTUP_INTEGRATIONS.get() {
        preserve_startup_runtime(
            &mut config,
            startup,
            &mut diagnostics,
            &mut invalid_sections,
        );
    }

    if let Some(value) = table.get("onboarding") {
        match value.clone().try_into::<Option<bool>>() {
            Ok(onboarding) => config.onboarding = onboarding,
            Err(err) => diagnostics.push(format!(
                "invalid onboarding setting: {err}; keeping current onboarding state"
            )),
        }
    }

    load_live_section(
        table,
        "theme",
        "theme config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.theme = section,
    );
    load_live_section(
        table,
        "keys",
        "keybinding config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.keys = section,
    );
    load_live_section(
        table,
        "terminal",
        "terminal config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.terminal = section,
    );
    load_live_section(
        table,
        "session",
        "session config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.session = section,
    );
    if table.contains_key("agent_detection") {
        load_live_section(
            table,
            "agent_detection",
            "agent detection config",
            &mut diagnostics,
            &mut invalid_sections,
            |section| config.agent_detection = section,
        );
    } else {
        load_live_section(
            table,
            "update",
            "legacy update config",
            &mut diagnostics,
            &mut invalid_sections,
            |section| config.agent_detection = section,
        );
    }
    load_live_section(
        table,
        "ui",
        "ui config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.ui = section,
    );
    load_live_section(
        table,
        "advanced",
        "advanced config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.advanced = section,
    );
    load_live_section(
        table,
        "worktrees",
        "worktree config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.worktrees = section,
    );
    load_live_section(
        table,
        "experimental",
        "experimental config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.experimental = section,
    );
    load_live_section(
        table,
        "remote",
        "remote config",
        &mut diagnostics,
        &mut invalid_sections,
        |section| config.remote = section,
    );

    Ok(LoadedConfig {
        config,
        diagnostics,
        invalid_sections,
    })
}

fn preserve_startup_runtime(
    config: &mut Config,
    startup: &IntegrationsConfig,
    diagnostics: &mut Vec<String>,
    invalid_sections: &mut Vec<String>,
) {
    if config.integrations.runtime != startup.runtime
        || config.integrations.external_agent_management_requested
            != startup.external_agent_management_requested
        || startup
            .runtime
            .as_ref()
            .is_some_and(|runtime| !config.integrations.sources.contains(&runtime.source))
    {
        diagnostics.push("integrations.runtime is startup-only; server restart required; keeping previous [integrations] settings".into());
        if !invalid_sections
            .iter()
            .any(|section| section == "integrations")
        {
            invalid_sections.push("integrations".into());
        }
        config.integrations.sources = startup.sources.clone();
    }
    config.integrations.runtime = startup.runtime.clone();
    config.integrations.external_agent_management_requested =
        startup.external_agent_management_requested;
}

fn unknown_top_level_sections_from_str(content: &str) -> (Vec<String>, Vec<String>) {
    let Ok(value) = content.parse::<toml::Value>() else {
        return (Vec::new(), Vec::new());
    };
    let Some(table) = value.as_table() else {
        return (Vec::new(), Vec::new());
    };

    let mut keys = Vec::new();
    let mut diagnostics = Vec::new();
    for (key, value) in table {
        if let Some(diagnostic) = unknown_top_level_section_diagnostic(key, value) {
            keys.push(key.clone());
            diagnostics.push(diagnostic);
        }
    }
    (keys, diagnostics)
}

fn unknown_top_level_section_diagnostics(
    table: &toml::map::Map<String, toml::Value>,
) -> Vec<String> {
    table
        .iter()
        .filter_map(|(key, value)| unknown_top_level_section_diagnostic(key, value))
        .collect()
}

fn unknown_top_level_section_diagnostic(key: &str, value: &toml::Value) -> Option<String> {
    if key == "external_view" {
        return Some("obsolete [external_view] config; use integrations.sources in [integrations]; ignoring section".into());
    }
    if KNOWN_TOP_LEVEL_CONFIG_KEYS.contains(&key) {
        return None;
    }

    let header = if value.is_table() {
        format!("[{key}]")
    } else if value
        .as_array()
        .is_some_and(|items| !items.is_empty() && items.iter().all(toml::Value::is_table))
    {
        format!("[[{key}]]")
    } else {
        return None;
    };

    if key == "toast" {
        Some(format!(
            "unknown config section {header}; did you mean [ui.toast]? ignoring section"
        ))
    } else {
        Some(format!("unknown config section {header}; ignoring section"))
    }
}

fn unknown_top_level_config_key_diagnostics(
    table: &toml::map::Map<String, toml::Value>,
) -> Vec<String> {
    let paths = table
        .iter()
        .filter(|(key, value)| {
            !KNOWN_TOP_LEVEL_CONFIG_KEYS.contains(&key.as_str())
                && unknown_top_level_section_diagnostic(key, value).is_none()
        })
        .map(|(key, _)| vec![ConfigKeyPathSegment::Key(key.clone())])
        .collect();
    unknown_config_key_diagnostics(paths, None)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ConfigKeyPathSegment {
    Key(String),
    Index(usize),
}

fn config_key_path(path: &serde_ignored::Path<'_>) -> Vec<ConfigKeyPathSegment> {
    fn visit(path: &serde_ignored::Path<'_>, segments: &mut Vec<ConfigKeyPathSegment>) {
        match path {
            serde_ignored::Path::Root => {}
            serde_ignored::Path::Seq { parent, index } => {
                visit(parent, segments);
                segments.push(ConfigKeyPathSegment::Index(*index));
            }
            serde_ignored::Path::Map { parent, key } => {
                visit(parent, segments);
                segments.push(ConfigKeyPathSegment::Key(key.clone()));
            }
            serde_ignored::Path::Some { parent }
            | serde_ignored::Path::NewtypeStruct { parent }
            | serde_ignored::Path::NewtypeVariant { parent } => visit(parent, segments),
        }
    }

    let mut segments = Vec::new();
    visit(path, &mut segments);
    segments
}

fn format_config_key_path(path: &[ConfigKeyPathSegment]) -> String {
    path.iter()
        .map(|segment| match segment {
            ConfigKeyPathSegment::Key(key)
                if !key.is_empty()
                    && key.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
                    }) =>
            {
                key.clone()
            }
            ConfigKeyPathSegment::Key(key) => toml::Value::String(key.clone()).to_string(),
            ConfigKeyPathSegment::Index(index) => index.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn unknown_config_key_diagnostics(
    paths: Vec<Vec<ConfigKeyPathSegment>>,
    section: Option<&str>,
) -> Vec<String> {
    let mut paths: Vec<Vec<ConfigKeyPathSegment>> = paths
        .into_iter()
        .map(|mut path| {
            if let Some(section) = section {
                path.insert(0, ConfigKeyPathSegment::Key(section.to_string()));
            }
            path
        })
        .collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .map(|path| {
            format!(
                "unknown config key {}; ignoring key",
                format_config_key_path(&path)
            )
        })
        .collect()
}

fn deserialize_with_ignored<'de, T, D>(
    deserializer: D,
) -> Result<(T, Vec<Vec<ConfigKeyPathSegment>>), D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    let mut ignored = Vec::new();
    let value = serde_ignored::deserialize(deserializer, |path| {
        ignored.push(config_key_path(&path));
    })?;
    Ok((value, ignored))
}

fn load_live_section<T>(
    table: &toml::map::Map<String, toml::Value>,
    section: &'static str,
    label: &str,
    diagnostics: &mut Vec<String>,
    invalid_sections: &mut Vec<String>,
    apply: impl FnOnce(T),
) where
    T: serde::de::DeserializeOwned,
{
    let Some(value) = table.get(section) else {
        return;
    };

    match deserialize_with_ignored(value.clone()) {
        Ok((section_config, ignored_keys)) => {
            diagnostics.extend(unknown_config_key_diagnostics(ignored_keys, Some(section)));
            apply(section_config);
        }
        Err(err) => {
            diagnostics.push(format!(
                "invalid {label}: {err}; keeping current {section} settings"
            ));
            invalid_sections.push(section.to_string());
        }
    }
}

pub(crate) fn upsert_top_level_bool(content: &str, key: &str, value: bool) -> String {
    let replacement = format!("{key} = {value}");
    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    let mut in_section = false;

    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = true;
            continue;
        }
        if in_section {
            continue;
        }
        if trimmed.starts_with(&format!("{key} ")) || trimmed.starts_with(&format!("{key}=")) {
            *line = replacement.clone();
            return lines.join("\n") + "\n";
        }
    }

    if lines.is_empty() {
        format!("{replacement}\n")
    } else {
        format!("{replacement}\n{}\n", lines.join("\n").trim_end())
    }
}

/// Write a key = value pair in a TOML section (creates section if missing).
pub fn upsert_section_value(content: &str, section: &str, key: &str, value: &str) -> String {
    upsert_section_raw(content, section, key, value)
}

pub fn upsert_section_bool(content: &str, section: &str, key: &str, value: bool) -> String {
    upsert_section_raw(content, section, key, &value.to_string())
}

pub fn remove_section_key(content: &str, section: &str, key: &str) -> String {
    let header = format!("[{section}]");
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    let mut in_section = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == header;
            result.push(line.to_string());
            i += 1;
            continue;
        }

        if in_section
            && (trimmed.starts_with(&format!("{key} ")) || trimmed.starts_with(&format!("{key}=")))
        {
            i += 1;
            continue;
        }

        result.push(line.to_string());
        i += 1;
    }

    result.join("\n") + "\n"
}

pub fn remove_keybinding_config_sections(content: &str) -> (String, bool) {
    let mut result = Vec::new();
    let mut removed = false;
    let mut skipping_key_section = false;
    let mut in_table = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(table_name) = toml_table_header_name(trimmed) {
            in_table = true;
            skipping_key_section = is_keys_table_name(table_name);
            if skipping_key_section {
                removed = true;
                continue;
            }
        } else if skipping_key_section || (!in_table && is_top_level_keys_assignment(trimmed)) {
            removed = true;
            continue;
        }

        result.push(line.to_string());
    }

    let mut updated = result.join("\n");
    if content.ends_with('\n') || !updated.is_empty() {
        updated.push('\n');
    }
    (updated, removed)
}

fn toml_table_header_name(trimmed: &str) -> Option<&str> {
    if let Some(name) = trimmed
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    {
        return Some(name.trim());
    }
    trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map(str::trim)
}

fn is_keys_table_name(name: &str) -> bool {
    name == "keys" || name.starts_with("keys.")
}

fn is_top_level_keys_assignment(trimmed: &str) -> bool {
    trimmed.starts_with("keys ") || trimmed.starts_with("keys=") || trimmed.starts_with("keys.")
}

fn upsert_section_raw(content: &str, section: &str, key: &str, value: &str) -> String {
    let header = format!("[{section}]");
    let assignment = format!("{key} = {value}");
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    let mut found_section = false;
    let mut inserted = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed == header {
            found_section = true;
            result.push(line.to_string());
            i += 1;

            while i < lines.len() {
                let current = lines[i];
                let current_trimmed = current.trim();
                if current_trimmed.starts_with('[') && current_trimmed.ends_with(']') {
                    if !inserted {
                        result.push(assignment.clone());
                        inserted = true;
                    }
                    break;
                }

                if current_trimmed.starts_with(&format!("{key} "))
                    || current_trimmed.starts_with(&format!("{key}="))
                {
                    result.push(assignment.clone());
                    inserted = true;
                } else {
                    result.push(current.to_string());
                }
                i += 1;
            }

            continue;
        }

        result.push(line.to_string());
        i += 1;
    }

    if !found_section {
        if !result.is_empty() && !result.last().is_some_and(|line| line.trim().is_empty()) {
            result.push(String::new());
        }
        result.push(header);
        result.push(assignment);
    } else if !inserted {
        result.push(assignment);
    }

    result.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AnnotationConfigFixture {
        path: PathBuf,
        original: Option<std::ffi::OsString>,
    }

    impl AnnotationConfigFixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "kitsune-annotations-config-{}.toml",
                std::process::id()
            ));
            let original = std::env::var_os(CONFIG_PATH_ENV_VAR);
            std::env::set_var(CONFIG_PATH_ENV_VAR, &path);
            Self { path, original }
        }

        fn write(&self, annotations: &str) {
            std::fs::write(
                &self.path,
                format!(
                    "{annotations}\n[keys]\nprefix = 'ctrl+space'\nfocus_pane_left = 'ctrl+h'\n[ui]\nsidebar_start_collapsed = true\n"
                ),
            )
            .unwrap();
        }
    }

    impl Drop for AnnotationConfigFixture {
        fn drop(&mut self) {
            if let Some(original) = &self.original {
                std::env::set_var(CONFIG_PATH_ENV_VAR, original);
            } else {
                std::env::remove_var(CONFIG_PATH_ENV_VAR);
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[test]
    fn integration_views_config_defaults_empty_and_accepts_ordered_sources() {
        assert!(Config::default().integrations.sources.is_empty());
        for content in ["", "[integrations]\n"] {
            let config: Config = toml::from_str(content).unwrap();
            assert!(config.integrations.sources.is_empty());
            assert!(config.invalid_integrations_diagnostic().is_none());
            let loaded = load_live_config_from_str(content).unwrap();
            assert!(loaded.config.integrations.sources.is_empty());
            assert!(loaded.invalid_sections.is_empty());
            assert!(loaded.diagnostics.is_empty());
        }
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        fixture.write("[integrations]\nsources = ['coven', 'build']");
        for loaded in [Config::load(), load_live_config().unwrap()] {
            assert_eq!(loaded.config.integrations.sources, ["coven", "build"]);
            assert!(loaded.invalid_sections.is_empty());
            assert!(loaded.diagnostics.is_empty());
        }
    }

    fn runtime_config() -> Config {
        toml::from_str(
            r#"
[integrations]
sources = ['example']
[integrations.runtime]
source = 'example'
command = ['/absolute/helper', 'run', '$literal', '~/literal']
report_command = ['/absolute/reporter', 'report', 'argument with spaces']
external_agent_management = true
"#,
        )
        .unwrap()
    }

    #[test]
    fn integration_runtime_config_is_explicit_bounded_literal_argv() {
        let config = runtime_config();
        assert!(config.invalid_integrations_diagnostic().is_none());
        let runtime = config.integrations.runtime.unwrap();
        assert_eq!(runtime.command[2..], ["$literal", "~/literal"]);
        assert!(runtime.external_agent_management);
        for argv in [
            vec![],
            vec!["helper".into()],
            vec!["~/helper".into()],
            vec!["$HOME/helper".into()],
            vec!["/helper".into(), "".into()],
            vec!["/helper".into(), "nul\0byte".into()],
            vec!["/helper".into(); 65],
            vec!["/helper".into(), "x".repeat(4097)],
            vec![
                "/helper".into(),
                "x".repeat(4096),
                "y".repeat(4096),
                "z".repeat(4096),
                "w".repeat(4096),
            ],
        ] {
            for report in [false, true] {
                let mut config = runtime_config();
                let runtime = config.integrations.runtime.as_mut().unwrap();
                if report {
                    runtime.report_command = argv.clone();
                } else {
                    runtime.command = argv.clone();
                }
                assert!(
                    config.invalid_integrations_diagnostic().is_some(),
                    "{argv:?}"
                );
            }
        }
        let mut config = runtime_config();
        config.integrations.sources.clear();
        assert!(config.invalid_integrations_diagnostic().is_some());
        config.integrations.sources = vec!["other".into()];
        assert!(config.invalid_integrations_diagnostic().is_some());
        assert!(Config::default().integrations.runtime.is_none());
    }

    #[test]
    fn integration_runtime_invalid_startup_fails_closed() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        for runtime in ["source = 'missing'\ncommand = ['/helper']\nreport_command = ['/report']",
            "source = 'example'\ncommand = ['relative']\nreport_command = ['/report']",
            "source = 'example'\ncommand = ['/helper']\nreport_command = []",
            "source = 'example'\ncommand = false",
            "source = 'example'\ncommand = ['/helper']\nreport_command = ['/report']\nexternal_agent_managment = true",
        ] {
            fixture.write(&format!("[integrations]\nsources = ['example']\n[integrations.runtime]\n{runtime}"));
            let loaded = Config::load();
            assert!(loaded.config.integrations.runtime.is_none());
            assert!(loaded.config.integrations.sources.is_empty());
            assert_eq!(loaded.invalid_sections, ["integrations"]);
            assert!(!loaded.diagnostics.is_empty());
        }
    }

    #[test]
    fn integration_runtime_reload_preserves_startup_ownership_and_route() {
        let startup = runtime_config().integrations;
        for mut proposed in [Config::default(), runtime_config(), runtime_config()] {
            if let Some(runtime) = proposed.integrations.runtime.as_mut() {
                runtime.external_agent_management = false;
            }
            let mut diagnostics = vec![];
            let mut invalid = vec![];
            preserve_startup_runtime(&mut proposed, &startup, &mut diagnostics, &mut invalid);
            assert_eq!(proposed.integrations.runtime, startup.runtime);
            assert_eq!(proposed.integrations.sources, startup.sources);
            assert_eq!(invalid, ["integrations"]);
            assert!(diagnostics[0].contains("restart required"));
        }
        let mut proposed = runtime_config();
        proposed.integrations.sources.clear();
        let mut diagnostics = vec![];
        let mut invalid = vec![];
        preserve_startup_runtime(&mut proposed, &startup, &mut diagnostics, &mut invalid);
        assert_eq!(proposed.integrations.sources, startup.sources);
        assert_eq!(invalid, ["integrations"]);

        let mut proposed = runtime_config();
        let disabled = IntegrationsConfig::default();
        preserve_startup_runtime(&mut proposed, &disabled, &mut diagnostics, &mut invalid);
        assert!(proposed.integrations.runtime.is_none());
        assert!(proposed.integrations.sources.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn integration_runtime_invalid_config_keeps_frozen_ownership_and_saved_refs() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        for content in [
            "[integrations]\nsources = ['example']\n[integrations.runtime]\nsource = 'example'\ncommand = ['relative']\nreport_command = ['/report']\nexternal_agent_management = true",
            "[integrations]\nsources = ['other']\n[integrations.runtime]\nsource = 'example'\ncommand = ['/helper']\nreport_command = ['/report']\nexternal_agent_management = true",
            "[integrations]\nsources = false\n[integrations.runtime]\nsource = 'example'\ncommand = ['/helper']\nreport_command = ['/report']\nexternal_agent_management = true",
            "[integrations]\nruntime = false",
            "[integrations.runtime]\nexternal_agent_management = true\ncommand = false",
            "[integrations.runtime]\nexternal_agent_management = 'true'",
            "[integrations.runtime\nexternal_agent_management = true",
        ] {
            fixture.write(content);
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "config::io::tests::integration_runtime_invalid_frozen_restore_fixture", "--ignored", "--nocapture"])
                .env("KITSUNE_TEST_INVALID_RUNTIME_FIXTURE", "1")
                .env("PATH", "")
                .spawn().unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() { break status; }
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("frozen ownership fixture timed out: {content}");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            assert!(status.success(), "{content}");
        }
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "spawned in isolation to exercise the production OnceLock"]
    fn integration_runtime_invalid_frozen_restore_fixture() {
        assert_eq!(
            std::env::var("KITSUNE_TEST_INVALID_RUNTIME_FIXTURE").as_deref(),
            Ok("1")
        );
        let loaded = Config::load();
        assert!(loaded.config.integrations.runtime.is_none());
        assert!(loaded
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("external agent ownership retained")));
        crate::config::freeze_integration_runtime(&loaded.config);
        assert!(crate::config::external_agent_management());
        assert!(crate::config::integration_runtime().is_none());
        let mut command = portable_pty::CommandBuilder::new("/bin/cat");
        crate::integration::apply_pane_base_env(&mut command);
        assert_eq!(
            command.get_env("KITSUNE_AGENT_MANAGEMENT"),
            Some(std::ffi::OsStr::new("external"))
        );
        assert!(command.get_env("KITSUNE_INTEGRATION_SOURCE").is_none());
        assert!(command
            .get_env("KITSUNE_INTEGRATION_REPORT_COMMAND")
            .is_none());

        // Exercise real restore with resume enabled and historical native references.
        let cwd = std::env::temp_dir();
        let tabs: Vec<_> = ["opencode", "claude", "codex"].into_iter().map(|agent| serde_json::json!({
            "layout": {"Pane": 0}, "zoomed": false,
            "panes": {"0": {"cwd": cwd, "agent_name": "historical", "managed_agent_kind": agent,
                "agent_session": {"source": format!("kitsune:{agent}"), "agent": agent, "kind": "id", "value": "saved-reference"}}}
        })).collect();
        let snapshot: crate::persist::SessionSnapshot = serde_json::from_value(serde_json::json!({
            "version": 3, "active": 0, "selected": 0,
            "workspaces": [{"identity_cwd": cwd, "tabs": tabs}]
        }))
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (events, _rx) = tokio::sync::mpsc::channel(64);
            let (_, terminals, runtimes) = crate::persist::restore(
                &snapshot,
                None,
                24,
                80,
                0,
                "/bin/cat",
                crate::config::ShellModeConfig::NonLogin,
                true,
                events,
                std::sync::Arc::new(tokio::sync::Notify::new()),
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            );
            let runtime_count = runtimes.len();
            for runtime in runtimes.into_values() {
                runtime.shutdown();
            }
            assert_eq!(terminals.len(), 3);
            assert_eq!(
                runtime_count, 3,
                "restore must spawn plain terminals, not queued native resumes"
            );
            for terminal in terminals.values() {
                assert!(terminal.pending_agent_resume_plan.is_none());
                assert!(terminal.detected_agent().is_none());
                assert!(terminal.hook_authority.is_none());
                assert!(terminal.agent_name.is_none());
                assert_eq!(
                    terminal
                        .persisted_agent_session
                        .as_ref()
                        .unwrap()
                        .session_ref
                        .value,
                    "saved-reference"
                );
            }
        });
        let reloaded = load_live_config_from_str("").unwrap();
        assert!(
            reloaded
                .config
                .integrations
                .external_agent_management_requested
        );
        assert!(reloaded
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("restart required")));
        assert!(crate::config::external_agent_management());
    }

    #[test]
    fn integration_views_structural_startup_errors_are_isolated_from_other_settings() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        for section in [
            "integrations = false",
            "[integrations]\nsources = 1",
            "[integrations]\nsources = ['one', 2]",
            "[integrations]\nsources = { name = 'one' }",
        ] {
            fixture.write(section);
            let loaded = Config::load();
            assert_eq!(loaded.config.keys.prefix, "ctrl+space", "{section}");
            assert!(loaded.config.ui.sidebar_start_collapsed);
            assert!(loaded.config.integrations.sources.is_empty());
            assert_eq!(loaded.invalid_sections, ["integrations"]);
            assert_eq!(loaded.diagnostics.len(), 1);
            assert!(loaded.diagnostics[0].contains("invalid integrations config"));
            assert!(loaded.diagnostics[0].contains("integrations disabled"));
        }
        fixture.write("[annotations]\nsources = 1\n[integrations]\nsources = false");
        let loaded = Config::load();
        assert_eq!(loaded.config.keys.prefix, "ctrl+space");
        assert_eq!(loaded.invalid_sections, ["annotations", "integrations"]);
        assert_eq!(loaded.diagnostics.len(), 2);
    }

    #[test]
    fn integration_views_obsolete_section_is_diagnosed_without_alias_or_rewrite() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        for section in [
            "[external_view]\nsource = 'legacy'",
            "external_view = false",
        ] {
            fixture.write(section);
            let original = std::fs::read_to_string(&fixture.path).unwrap();
            for loaded in [Config::load(), load_live_config().unwrap()] {
                assert!(loaded.config.integrations.sources.is_empty());
                assert!(loaded.invalid_sections.is_empty());
                assert_eq!(loaded.diagnostics.len(), 1);
                assert!(loaded.diagnostics[0].contains("obsolete [external_view]"));
                assert!(loaded.diagnostics[0].contains("integrations.sources"));
            }
            assert_eq!(std::fs::read_to_string(&fixture.path).unwrap(), original);
        }
        fixture.write(
            "[external_view]\nsource = 'legacy'\n[integrations]\nsources = ['coven', 'build']",
        );
        for loaded in [Config::load(), load_live_config().unwrap()] {
            assert_eq!(loaded.config.integrations.sources, ["coven", "build"]);
            assert_eq!(loaded.diagnostics.len(), 1);
        }
    }

    #[test]
    fn integration_views_duplicate_and_oversized_lists_disable_only_integrations() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        for sources in ["['a', 'a']", "['0','1','2','3','4','5','6','7','8']"] {
            fixture.write(&format!("[integrations]\nsources = {sources}"));
            let loaded = Config::load();
            assert!(loaded.config.integrations.sources.is_empty());
            assert_eq!(loaded.config.keys.prefix, "ctrl+space");
            assert!(loaded.config.ui.sidebar_start_collapsed);
            assert_eq!(loaded.invalid_sections, ["integrations"]);
            assert_eq!(loaded.diagnostics.len(), 1);
            let reload = load_live_config().unwrap();
            assert_eq!(reload.invalid_sections, ["integrations"]);
        }
    }

    #[test]
    fn integration_views_invalid_startup_sources_are_diagnosed_and_disabled() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        for source in ["", "bad source", "path/name", "\u{e9}", &"a".repeat(65)] {
            fixture.write(&format!("[integrations]\nsources = ['valid', '{source}']"));
            let loaded = Config::load();
            assert_eq!(loaded.config.keys.prefix, "ctrl+space");
            assert!(loaded.config.ui.sidebar_start_collapsed);
            assert!(loaded.config.integrations.sources.is_empty());
            assert_eq!(loaded.invalid_sections, ["integrations"]);
            assert_eq!(loaded.diagnostics.len(), 1);
            assert!(loaded.diagnostics[0].contains("invalid integrations.sources"));
            assert!(loaded.diagnostics[0].contains("integrations disabled"));
            let config: Config =
                toml::from_str(&format!("[integrations]\nsources = ['{source}']")).unwrap();
            assert!(config
                .collect_diagnostics()
                .iter()
                .any(|message| message.contains("invalid integrations.sources")));
        }
    }

    #[test]
    fn integration_views_startup_recovery_preserves_warnings_but_not_unrelated_errors() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        fixture.write("[integrations]\nsources = 1\n[advanced]\nscrollback_limt = 1");
        let loaded = Config::load();
        assert_eq!(loaded.config.keys.prefix, "ctrl+space");
        assert_eq!(loaded.diagnostics.len(), 2);
        assert_eq!(
            loaded.diagnostics[0],
            "unknown config key advanced.scrollback_limt; ignoring key"
        );
        assert!(loaded.diagnostics[1].contains("integrations disabled"));
        std::fs::write(
            &fixture.path,
            "[integrations]\nsources = 1\n[ui]\nsidebar_start_collapsed = 'yes'",
        )
        .unwrap();
        let loaded = Config::load();
        assert!(loaded.invalid_sections.is_empty());
        assert_eq!(loaded.diagnostics.len(), 1);
        assert!(loaded.diagnostics[0].starts_with("config parse error:"));
    }

    #[test]
    fn integration_views_unknown_keys_warn_at_startup_and_reload() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        fixture.write("[integrations]\nsources = ['example']\nsoruce = 'bad'");
        for loaded in [Config::load(), load_live_config().unwrap()] {
            assert_eq!(loaded.config.integrations.sources, ["example"]);
            assert!(loaded.invalid_sections.is_empty());
            assert_eq!(
                loaded.diagnostics,
                ["unknown config key integrations.soruce; ignoring key"]
            );
        }
    }

    #[test]
    fn integration_views_invalid_reload_keeps_previous_sources_and_leases_and_applies_keys() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        let mut config = Config::default();
        config.integrations.sources = vec!["previous".into()];
        let mut app = crate::app::App::new(
            &config,
            true,
            None,
            tokio::sync::mpsc::unbounded_channel().1,
            crate::api::EventHub::default(),
        );
        let opened = app
            .state
            .integration_views
            .open(
                &crate::api::schema::ViewOpenParams {
                    source: "previous".into(),
                    version: 1,
                },
                std::time::Instant::now(),
            )
            .unwrap();
        for section in [
            "integrations = false",
            "[integrations]\nsources = 1",
            "[integrations]\nsources = ['']",
            "[integrations]\nsources = ['bad source']",
            "[integrations]\nsources = ['duplicate', 'duplicate']",
        ] {
            fixture.write(section);
            let loaded = load_live_config().unwrap();
            assert_eq!(loaded.invalid_sections, ["integrations"]);
            assert_eq!(loaded.config.keys.prefix, "ctrl+space");
            assert_eq!(loaded.diagnostics.len(), 1);
            let report = app.reload_config();
            assert_eq!(report.status, crate::config::ConfigReloadStatus::Partial);
            assert_eq!(
                app.state.integration_views.configured_sources(),
                ["previous"]
            );
            assert_eq!(
                app.state
                    .integration_views
                    .descriptor(std::time::Instant::now())
                    .sources[0]
                    .epoch
                    .as_deref(),
                Some(opened.epoch.as_str())
            );
            assert_eq!(app.state.prefix_code, crossterm::event::KeyCode::Char(' '));
            assert_eq!(
                app.state.prefix_mods,
                crossterm::event::KeyModifiers::CONTROL
            );
        }
    }

    #[test]
    fn annotations_structural_startup_error_preserves_keys_and_ui() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        for annotations in [
            "[annotations]\nsources = 'build'",
            "annotations = false",
            "[annotations]\nsources = ['build', 1]",
            "[annotations]\nsources = -1",
            "[annotations]\nsources = { build = true }",
        ] {
            fixture.write(annotations);
            let loaded = Config::load();
            assert_eq!(loaded.config.keys.prefix, "ctrl+space", "{annotations}");
            assert!(loaded.config.ui.sidebar_start_collapsed);
            assert!(loaded.config.annotations.sources.is_empty());
            assert_eq!(loaded.invalid_sections, ["annotations"]);
            assert_eq!(loaded.diagnostics.len(), 1);
            assert!(loaded.diagnostics[0].contains("invalid annotations config"));
            assert!(loaded.diagnostics[0].contains("annotations disabled"));
            let app = crate::app::App::new(
                &loaded.config,
                true,
                None,
                tokio::sync::mpsc::unbounded_channel().1,
                crate::api::EventHub::default(),
            );
            assert!(app.state.annotations.sources().is_empty());
            assert_eq!(app.state.prefix_code, crossterm::event::KeyCode::Char(' '));
            assert_eq!(
                app.state.prefix_mods,
                crossterm::event::KeyModifiers::CONTROL
            );
            assert_eq!(
                app.state.keybinds.focus_pane_left.bindings[0].label,
                "ctrl+h"
            );
        }
    }

    #[test]
    fn legacy_kitty_graphics_flag_is_unknown_at_startup_and_reload() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        for value in [true, false] {
            fixture.write(&format!(
                "[experimental]\nkitty_graphics = {value}\nallow_nested = true\n"
            ));
            for loaded in [Config::load(), load_live_config().unwrap()] {
                assert!(loaded.config.experimental.allow_nested);
                assert!(loaded.invalid_sections.is_empty());
                assert_eq!(
                    loaded.diagnostics,
                    ["unknown config key experimental.kitty_graphics; ignoring key"]
                );
            }
        }
    }

    #[test]
    fn annotations_unknown_key_warning_survives_startup_and_live_parsing() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        fixture.write("[annotations]\nsources = ['build']\nsoruces = ['tests']");
        for loaded in [Config::load(), load_live_config().unwrap()] {
            assert_eq!(loaded.config.annotations.sources, ["build"]);
            assert!(loaded.invalid_sections.is_empty());
            assert_eq!(
                loaded.diagnostics,
                ["unknown config key annotations.soruces; ignoring key"]
            );
        }
    }

    #[test]
    fn annotations_startup_recovery_preserves_unknown_sibling_key_warning() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        fixture.write("[annotations]\nsources = 'build'\n[advanced]\nscrollback_limt = 1");
        let loaded = Config::load();
        assert_eq!(loaded.config.keys.prefix, "ctrl+space");
        assert_eq!(loaded.diagnostics.len(), 2);
        assert_eq!(
            loaded.diagnostics[0],
            "unknown config key advanced.scrollback_limt; ignoring key"
        );
        assert!(loaded.diagnostics[1].contains("annotations disabled"));
    }

    #[test]
    fn annotations_startup_recovery_does_not_mask_other_config_errors() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        for content in [
            "[annotations]\nsources = 'build'\n[ui]\nsidebar_start_collapsed = 'yes'",
            "[annotations]\nsources = ['build']\n[ui]\nsidebar_start_collapsed = 'yes'",
            "[keys]\nprefix = 'ctrl+space'\n[annotations]\nsources = [",
        ] {
            std::fs::write(&fixture.path, content).unwrap();
            let loaded = Config::load();
            assert_eq!(loaded.config.keys.prefix, "ctrl+b");
            assert!(!loaded.config.ui.sidebar_start_collapsed);
            assert!(loaded.invalid_sections.is_empty());
            let ownership_unknown = content.parse::<toml::Value>().is_err();
            assert_eq!(loaded.diagnostics.len(), 1 + usize::from(ownership_unknown));
            assert_eq!(
                loaded
                    .config
                    .integrations
                    .external_agent_management_requested,
                ownership_unknown
            );
            if ownership_unknown {
                assert!(loaded.diagnostics[1].contains("external agent ownership retained"));
            }
            assert!(loaded.diagnostics[0].starts_with("config parse error:"));
            assert!(loaded.diagnostics[0].ends_with("using defaults"));
        }
        assert!(toml::from_str::<Config>("[annotations]\nsources = 'build'").is_err());
    }

    #[test]
    fn annotations_structural_live_error_keeps_selection_and_epoch_applies_keys() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let fixture = AnnotationConfigFixture::new();
        let mut config = Config::default();
        config.annotations.sources = vec!["build".into(), "tests".into()];
        let mut app = crate::app::App::new(
            &config,
            true,
            None,
            tokio::sync::mpsc::unbounded_channel().1,
            crate::api::EventHub::default(),
        );
        let epoch = app.state.annotations.epoch().to_owned();
        for annotations in [
            "[annotations]\nsources = 'build'",
            "annotations = false",
            "[annotations]\nsources = ['build', 1]",
            "[annotations]\nsources = -1",
        ] {
            fixture.write(annotations);
            let loaded = load_live_config().unwrap();
            assert_eq!(loaded.invalid_sections, ["annotations"]);
            assert_eq!(loaded.diagnostics.len(), 1);
            let report = app.reload_config();
            assert_eq!(report.status, crate::config::ConfigReloadStatus::Partial);
            assert_eq!(app.state.annotations.sources(), ["build", "tests"]);
            assert_eq!(app.state.annotations.epoch(), epoch);
            assert_eq!(app.state.prefix_code, crossterm::event::KeyCode::Char(' '));
            assert_eq!(
                app.state.prefix_mods,
                crossterm::event::KeyModifiers::CONTROL
            );
            assert_eq!(
                app.state.keybinds.focus_pane_left.bindings[0].label,
                "ctrl+h"
            );
        }
    }

    #[test]
    fn annotations_live_config_validates_without_activation() {
        let empty = load_live_config_from_str("").unwrap();
        assert!(empty.config.annotations.sources.is_empty());
        let valid =
            load_live_config_from_str("[annotations]\nsources = ['build', 'tests']\n").unwrap();
        assert_eq!(valid.config.annotations.sources, ["build", "tests"]);
        assert!(valid.invalid_sections.is_empty());
        assert!(!valid
            .diagnostics
            .iter()
            .any(|value| value.contains("annotations")));
        for section in [
            "sources = ['build', 'build']",
            "sources = ['bad source']",
            "sources = 1",
        ] {
            let invalid =
                load_live_config_from_str(&format!("[annotations]\n{section}\n")).unwrap();
            assert!(invalid
                .invalid_sections
                .iter()
                .any(|value| value == "annotations"));
            assert!(!invalid.diagnostics.is_empty());
        }
        let config: Config = toml::from_str("[annotations]\nsources = ['bad source']").unwrap();
        assert!(config
            .collect_diagnostics()
            .iter()
            .any(|value| value.contains("annotations.sources")));
    }

    #[test]
    fn upsert_top_level_bool_replaces_existing_value() {
        let content = "onboarding = true\n[keys]\nprefix = \"ctrl+b\"\n";
        let updated = upsert_top_level_bool(content, "onboarding", false);
        assert!(updated.contains("onboarding = false"));
        assert!(!updated.contains("onboarding = true"));
    }

    #[test]
    fn upsert_section_bool_adds_missing_section() {
        let updated = upsert_section_bool("", "ui.toast", "enabled", true);
        assert!(updated.contains("[ui.toast]"));
        assert!(updated.contains("enabled = true"));
    }

    #[test]
    fn remove_section_key_removes_matching_key_from_section() {
        let content =
            "[ui.toast]\nenabled = true\ndelivery = \"kitsune\"\n[ui.sound]\nenabled = true\n";
        let updated = remove_section_key(content, "ui.toast", "enabled");
        assert!(!updated.contains("[ui.toast]\nenabled = true"));
        assert!(updated.contains("delivery = \"kitsune\""));
        assert!(updated.contains("[ui.sound]\nenabled = true"));
    }

    #[test]
    fn config_diagnostic_summary_uses_compact_actionable_banner() {
        let diagnostics = vec![
            "one".to_string(),
            "two".to_string(),
            "three".to_string(),
            "four".to_string(),
            "five".to_string(),
        ];

        assert_eq!(
            config_diagnostic_summary(&diagnostics).as_deref(),
            Some("config.toml; kitsune config check")
        );
    }

    #[test]
    fn config_diagnostic_summary_reports_unknown_keys_compactly() {
        let diagnostics = vec![
            "unknown config key ui.mouse_captur; ignoring key".to_string(),
            "unknown config key keys.new_tabb; ignoring key".to_string(),
        ];

        assert_eq!(
            config_diagnostic_summary(&diagnostics).as_deref(),
            Some("config.toml has unknown keys; kitsune config check")
        );
    }

    #[test]
    fn config_diagnostic_summary_keeps_mixed_diagnostics_generic() {
        let diagnostics = vec![
            "invalid ui config: invalid type: string; keeping current ui settings".to_string(),
            "unknown config key keys.new_tabb; ignoring key".to_string(),
        ];

        assert_eq!(
            config_diagnostic_summary(&diagnostics).as_deref(),
            Some("config.toml; kitsune config check")
        );
    }

    #[test]
    fn config_diagnostic_summary_reports_default_fallback() {
        let diagnostics = vec![
            "config parse error: TOML parse error at line 33, column 8\n   |\n33 | type = \"popup\"\n   |        ^^^^^^^\nunknown variant `popup`; using defaults"
                .to_string(),
        ];

        assert_eq!(
            config_diagnostic_summary(&diagnostics).as_deref(),
            Some("config.toml invalid; using defaults; kitsune config check")
        );
    }

    #[test]
    fn config_diagnostic_summary_reports_unreadable_config_impact() {
        let startup = vec!["config read error: permission denied; using defaults".to_string()];
        assert_eq!(
            config_diagnostic_summary(&startup).as_deref(),
            Some("config.toml unreadable; using defaults; kitsune config check")
        );

        let reload =
            vec!["config read error: permission denied; keeping current config".to_string()];
        assert_eq!(
            config_diagnostic_summary(&reload).as_deref(),
            Some("config.toml unreadable; keeping current config; kitsune config check")
        );
    }

    #[test]
    fn config_diagnostic_summary_reports_retained_live_config() {
        let diagnostics = vec![
            "config parse error: TOML parse error at line 7, column 4; keeping current config"
                .to_string(),
        ];

        assert_eq!(
            config_diagnostic_summary(&diagnostics).as_deref(),
            Some("config.toml invalid; keeping current config; kitsune config check")
        );
    }

    #[test]
    fn config_loaders_report_unreadable_path() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let path =
            std::env::temp_dir().join(format!("kitsune-config-unreadable-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::env::set_var(CONFIG_PATH_ENV_VAR, &path);

        let startup = Config::load();
        assert!(
            startup
                .config
                .integrations
                .external_agent_management_requested
        );
        assert!(startup
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("config read error")
                && diagnostic.contains("using defaults")));

        let reload = load_live_config().unwrap_err();
        assert!(reload.iter().any(|diagnostic| {
            diagnostic.contains("config read error")
                && diagnostic.contains("keeping current config")
        }));

        std::env::remove_var(CONFIG_PATH_ENV_VAR);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn load_live_config_parses_session_section() {
        let loaded = load_live_config_from_str(
            r#"
[session]
resume_agents_on_restore = true
"#,
        )
        .unwrap();

        assert!(loaded.config.session.resume_agents_on_restore);
        assert!(loaded.diagnostics.is_empty());
        assert!(loaded.invalid_sections.is_empty());
    }

    #[test]
    fn load_live_config_parses_agent_detection_section_and_legacy_update_alias() {
        let loaded = load_live_config_from_str(
            r#"
[agent_detection]
manifest_check = true
"#,
        )
        .unwrap();

        assert!(loaded.config.agent_detection.manifest_check);
        assert!(loaded.diagnostics.is_empty());
        assert!(loaded.invalid_sections.is_empty());

        let legacy_loaded = load_live_config_from_str(
            r#"
[update]
manifest_check = true
"#,
        )
        .unwrap();

        assert!(legacy_loaded.config.agent_detection.manifest_check);
        assert!(legacy_loaded.diagnostics.is_empty());
        assert!(legacy_loaded.invalid_sections.is_empty());
    }

    #[test]
    fn load_live_config_reports_invalid_agent_detection_section() {
        let loaded = load_live_config_from_str(
            r#"
[agent_detection]
manifest_check = "yes"
"#,
        )
        .unwrap();

        assert_eq!(loaded.invalid_sections, vec!["agent_detection"]);
        assert!(!loaded.config.agent_detection.manifest_check);
        assert_eq!(loaded.diagnostics.len(), 1);
        assert!(loaded.diagnostics[0].contains("invalid agent detection config"));
    }

    #[test]
    fn load_live_config_warns_about_unknown_top_level_sections() {
        let loaded = load_live_config_from_str(
            r#"
[toast]
delivery = "system"

[ui.toast]
delivery = "kitsune"
"#,
        )
        .unwrap();

        assert_eq!(
            loaded.diagnostics,
            vec!["unknown config section [toast]; did you mean [ui.toast]? ignoring section"]
        );
        assert!(loaded.invalid_sections.is_empty());
        assert_eq!(
            loaded.config.ui.toast.delivery,
            super::super::ToastDelivery::Kitsune
        );
    }

    #[test]
    fn load_live_config_warns_about_unknown_keys_and_applies_known_siblings() {
        let loaded = load_live_config_from_str(
            r##"
plugin = []

[theme.custom]
accentt = "#ffffff"

[advanced]
scrollback_lines = 42

[keys]
fullscreen = "prefix+z"
new_tabb = "prefix+t"

[[keys.command]]
key = "prefix+g"
command = "git status"
descrption = "status"

[ui]
mouse_capture = false
mouse_captur = true
"foo.bar" = true
"foo.?.bar" = false

[ui.toast]
enabled = true
delivry = "system"

[ui.sidebar.agents.rows_by_agent]
claude = [["terminal_title"]]
"##,
        )
        .unwrap();

        assert_eq!(
            loaded.diagnostics,
            vec![
                "unknown config key plugin; ignoring key",
                "unknown config key theme.custom.accentt; ignoring key",
                "unknown config key keys.command.0.descrption; ignoring key",
                "unknown config key keys.new_tabb; ignoring key",
                "unknown config key ui.\"foo.?.bar\"; ignoring key",
                "unknown config key ui.\"foo.bar\"; ignoring key",
                "unknown config key ui.mouse_captur; ignoring key",
                "unknown config key ui.toast.delivry; ignoring key",
            ]
        );
        assert!(loaded.invalid_sections.is_empty());
        assert_eq!(loaded.config.advanced.scrollback_limit_bytes, 42);
        assert!(!loaded.config.ui.mouse_capture);
        assert_eq!(
            loaded.config.ui.toast.delivery,
            super::super::ToastDelivery::Kitsune
        );
        assert!(loaded
            .config
            .keybinds()
            .zoom
            .bindings
            .iter()
            .any(|binding| binding.label == "prefix+z"));
    }

    #[test]
    fn load_live_config_discards_ignored_keys_from_an_invalid_section() {
        let loaded = load_live_config_from_str(
            r#"
[ui]
mouse_capture = "yes"
mouse_captur = true
"#,
        )
        .unwrap();

        assert_eq!(loaded.diagnostics.len(), 1);
        assert!(loaded.diagnostics[0].contains("invalid ui config"));
        assert!(!loaded.diagnostics[0].starts_with("unknown config key"));
        assert_eq!(loaded.invalid_sections, vec!["ui"]);
    }

    #[test]
    fn startup_config_load_warns_about_unknown_top_level_sections() {
        let _guard = crate::config::test_config_env_lock().lock().unwrap();
        let path = std::env::temp_dir().join(format!(
            "kitsune-config-unknown-section-{}.toml",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"
[[plugin]]
id = "example"

[ui.toast]
delivery = "system"
"#,
        )
        .unwrap();
        std::env::set_var(CONFIG_PATH_ENV_VAR, &path);

        let loaded = Config::load();

        assert_eq!(
            loaded.diagnostics,
            vec!["unknown config section [[plugin]]; ignoring section"]
        );
        assert_eq!(
            loaded.config.ui.toast.delivery,
            super::super::ToastDelivery::System
        );

        std::env::remove_var(CONFIG_PATH_ENV_VAR);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn remove_keybinding_config_sections_removes_keys_tables_only() {
        let content = r#"onboarding = false

[theme]
name = "catppuccin"

[keys]
prefix = "ctrl+a"
new_tab = "c"

[[keys.command]]
key = "g"
command = "lazygit"

[keys.indexed]
tabs = "ctrl"

[ui]
mouse_capture = false
"#;

        let (updated, removed) = remove_keybinding_config_sections(content);

        assert!(removed);
        assert!(updated.contains("onboarding = false"));
        assert!(updated.contains("[theme]\nname = \"catppuccin\""));
        assert!(updated.contains("[ui]\nmouse_capture = false"));
        assert!(!updated.contains("[keys]"));
        assert!(!updated.contains("[[keys.command]]"));
        assert!(!updated.contains("[keys.indexed]"));
        assert!(toml::from_str::<toml::Value>(&updated).is_ok());
    }

    #[test]
    fn remove_keybinding_config_sections_reports_noop_without_keys() {
        let content = "[ui]\nmouse_capture = true\n";
        let (updated, removed) = remove_keybinding_config_sections(content);
        assert!(!removed);
        assert_eq!(updated, content);
    }
}
