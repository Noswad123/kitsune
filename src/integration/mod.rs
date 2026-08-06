mod actions;
mod command;
mod config_edit;
mod env;
mod file_ops;
mod registry;
mod targets;
mod types;
mod version;

pub(crate) use actions::{install_target, uninstall_target};
#[cfg(test)]
pub(crate) use env::integration_env_lock;
pub(crate) use env::{
    apply_pane_base_env, KITSUNE_PANE_ID_ENV_VAR, KITSUNE_TAB_ID_ENV_VAR,
    KITSUNE_WORKSPACE_ID_ENV_VAR,
};
pub(crate) use registry::{
    installed_integration_statuses, integration_recommendations, integration_target_label,
    print_outdated_update_notice,
};
pub(crate) use types::{IntegrationRecommendation, IntegrationStatus, IntegrationStatusKind};

const PI_EXTENSION_INSTALL_NAME: &str = "kitsune-agent-state.ts";
const PI_EXTENSION_ASSET: &str = include_str!("assets/pi/kitsune-agent-state.ts");
const PI_INTEGRATION_VERSION: u32 = 7;
const CLAUDE_HOOK_INSTALL_NAME: &str = if cfg!(windows) {
    "kitsune-agent-state.ps1"
} else {
    "kitsune-agent-state.sh"
};
const CLAUDE_HOOK_ASSET: &str = if cfg!(windows) {
    include_str!("assets/claude/kitsune-agent-state.ps1")
} else {
    include_str!("assets/claude/kitsune-agent-state.sh")
};
const CLAUDE_INTEGRATION_VERSION: u32 = 7;
const CODEX_HOOK_INSTALL_NAME: &str = if cfg!(windows) {
    "kitsune-agent-state.ps1"
} else {
    "kitsune-agent-state.sh"
};
const CODEX_HOOK_ASSET: &str = if cfg!(windows) {
    include_str!("assets/codex/kitsune-agent-state.ps1")
} else {
    include_str!("assets/codex/kitsune-agent-state.sh")
};
const CODEX_INTEGRATION_VERSION: u32 = 6;
const COPILOT_HOOK_INSTALL_NAME: &str = if cfg!(windows) {
    "kitsune-agent-state.ps1"
} else {
    "kitsune-agent-state.sh"
};
const COPILOT_HOOK_ASSET: &str = if cfg!(windows) {
    include_str!("assets/copilot/kitsune-agent-state.ps1")
} else {
    include_str!("assets/copilot/kitsune-agent-state.sh")
};
const COPILOT_INTEGRATION_VERSION: u32 = 2;
const COPILOT_HOOK_EVENTS: [&str; 1] = ["SessionStart"];
const COPILOT_REMOVED_LIFECYCLE_HOOK_EVENTS: [&str; 9] = [
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "agentStop",
    "SessionEnd",
    "notification",
    "sessionStart",
];
const OPENCODE_PLUGIN_INSTALL_NAME: &str = "kitsune-agent-state.js";
const OPENCODE_PLUGIN_ASSET: &str = include_str!("assets/opencode/kitsune-agent-state.js");
const OPENCODE_INTEGRATION_VERSION: u32 = 9;
const BUDDY_PLUGIN_INSTALL_NAME: &str = "kitsune-agent-state-buddy.js";
const BUDDY_PLUGIN_ASSET: &str = include_str!("assets/buddy/kitsune-agent-state.js");
const BUDDY_INTEGRATION_VERSION: u32 = 1;
const INTEGRATION_VERSION_MARKER: &str = "KITSUNE_INTEGRATION_VERSION=";

pub(crate) const INSTALL_WARNING_PREFIX: &str = "warning:";

#[cfg(test)]
mod tests;
