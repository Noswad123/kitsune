use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntegrationStatus {
    pub target: crate::api::schema::IntegrationTarget,
    pub path: PathBuf,
    pub state: IntegrationStatusKind,
    pub installed_version: Option<u32>,
    pub expected_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegrationStatusKind {
    NotInstalled,
    Current,
    Outdated,
}

#[derive(Debug)]
pub(crate) struct PiUninstallResult {
    pub extension_path: PathBuf,
    pub removed_extension: bool,
}

#[derive(Debug)]
pub(crate) struct OpenCodeUninstallResult {
    pub plugin_path: PathBuf,
    pub removed_plugin: bool,
}

#[derive(Debug)]
pub(crate) struct ClaudeUninstallResult {
    pub hook_path: PathBuf,
    pub settings_path: PathBuf,
    pub removed_hook_file: bool,
    pub updated_settings: bool,
}

#[derive(Debug)]
pub(crate) struct CodexUninstallResult {
    pub hook_path: PathBuf,
    pub hooks_path: PathBuf,
    pub config_path: PathBuf,
    pub removed_hook_file: bool,
    pub updated_hooks: bool,
}

#[derive(Debug)]
pub(crate) struct CopilotUninstallResult {
    pub hook_path: PathBuf,
    pub settings_path: PathBuf,
    pub removed_hook_file: bool,
    pub updated_settings: bool,
}
