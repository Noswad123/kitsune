mod actions;
mod command;
mod config_edit;
mod env;
mod file_ops;
mod registry;
mod targets;
mod types;

pub(crate) use actions::{install_target, uninstall_target};
#[cfg(test)]
pub(crate) use env::integration_env_lock;
pub(crate) use env::{
    apply_pane_base_env, binary_path_env_value, KITSUNE_PANE_ID_ENV_VAR, KITSUNE_TAB_ID_ENV_VAR,
    KITSUNE_WORKSPACE_ID_ENV_VAR,
};
pub(crate) use registry::{installed_integration_statuses, integration_target_label};
pub(crate) use types::{IntegrationStatus, IntegrationStatusKind};

const PI_EXTENSION_INSTALL_NAME: &str = "kitsune-agent-state.ts";
// Frozen compatibility data for legacy fingerprinting, migration and cleanup tests.
#[cfg(test)]
const PI_EXTENSION_ASSET: &str = include_str!("assets/pi/kitsune-agent-state.ts");
const PI_INTEGRATION_VERSION: u32 = 7;
const CLAUDE_HOOK_INSTALL_NAME: &str = if cfg!(windows) {
    "kitsune-agent-state.ps1"
} else {
    "kitsune-agent-state.sh"
};
// Frozen compatibility data, never a fresh installer payload.
#[cfg(test)]
const CLAUDE_HOOK_ASSET: &str = if cfg!(windows) {
    include_str!("assets/claude/kitsune-agent-state.ps1")
} else {
    include_str!("assets/claude/kitsune-agent-state.sh")
};
const CLAUDE_INTEGRATION_VERSION: u32 = 7;
#[cfg(target_os = "linux")]
const CLAUDE_COVEN_GUIDANCE: &str = "Claude hooks on Linux are managed by Coven; use `coven agent hooks plan` / `coven agent hooks install`. To migrate verified legacy Kitsune hooks, explicitly pass `--migrate-kitsune` to Coven. Kitsune uninstall is legacy cleanup only.";
const CODEX_HOOK_INSTALL_NAME: &str = if cfg!(windows) {
    "kitsune-agent-state.ps1"
} else {
    "kitsune-agent-state.sh"
};
// Frozen compatibility data, never a fresh installer payload.
#[cfg(test)]
const CODEX_HOOK_ASSET: &str = if cfg!(windows) {
    include_str!("assets/codex/kitsune-agent-state.ps1")
} else {
    include_str!("assets/codex/kitsune-agent-state.sh")
};
const CODEX_INTEGRATION_VERSION: u32 = 6;
#[cfg(target_os = "linux")]
const CODEX_COVEN_GUIDANCE: &str = "Codex hooks on Linux are managed by Coven; use `coven agent hooks plan` / `coven agent hooks install`. To migrate verified legacy Kitsune hooks, explicitly pass `--migrate-kitsune` to Coven. Native-hook trust for each new Codex release is user-managed. Installed configuration alone does not prove operational capture. Kitsune uninstall is legacy cleanup only.";
const COPILOT_HOOK_INSTALL_NAME: &str = if cfg!(windows) {
    "kitsune-agent-state.ps1"
} else {
    "kitsune-agent-state.sh"
};
// Frozen compatibility data, never a fresh installer payload.
#[cfg(test)]
const COPILOT_HOOK_ASSET: &str = if cfg!(windows) {
    include_str!("assets/copilot/kitsune-agent-state.ps1")
} else {
    include_str!("assets/copilot/kitsune-agent-state.sh")
};
const COPILOT_INTEGRATION_VERSION: u32 = 2;
#[cfg(target_os = "linux")]
const COPILOT_COVEN_GUIDANCE: &str = "Copilot hooks on Linux are managed by Coven; use `coven agent hooks plan` / `coven agent hooks install` with `--provider copilot` and an explicit `--config-root`. To migrate verified legacy Kitsune hooks, explicitly pass `--migrate-kitsune` to Coven. Coven manages settings.json direct hooks.SessionStart handlers for reference-only startup capture, not native approval. Installed configuration alone does not prove operational capture. Kitsune uninstall is legacy cleanup only.";
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
#[cfg(test)]
const OPENCODE_PLUGIN_ASSET: &str = include_str!("assets/opencode/kitsune-agent-state.js");
const OPENCODE_INTEGRATION_VERSION: u32 = 9;
const INTEGRATION_VERSION_MARKER: &str = "KITSUNE_INTEGRATION_VERSION=";

pub(crate) const INSTALL_RETIRED_GUIDANCE: &str = "feature_retired: integration.install: legacy Kitsune agent integration installation is retired on all platforms. Status and explicit uninstall remain available for legacy cleanup. Use an independently supported external integration; replacement support on Windows/macOS is not implied.";

#[cfg(test)]
mod tests;
