mod actions;
mod command;
// Legacy install helpers remain for non-Linux compatibility and test fixtures.
#[cfg_attr(all(target_os = "linux", not(test)), allow(dead_code))]
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
    apply_pane_base_env, binary_path_env_value, KITSUNE_PANE_ID_ENV_VAR, KITSUNE_TAB_ID_ENV_VAR,
    KITSUNE_WORKSPACE_ID_ENV_VAR,
};
pub(crate) use registry::{
    installed_integration_statuses, integration_recommendations, integration_target_label,
    print_outdated_update_notice,
};
pub(crate) use types::{IntegrationRecommendation, IntegrationStatus, IntegrationStatusKind};

const PI_EXTENSION_INSTALL_NAME: &str = "kitsune-agent-state.ts";
#[cfg(any(test, not(target_os = "linux")))]
const PI_EXTENSION_ASSET: &str = include_str!("assets/pi/kitsune-agent-state.ts");
const PI_INTEGRATION_VERSION: u32 = 7;
#[cfg(target_os = "linux")]
const PI_COVEN_GUIDANCE: &str = "Pi extensions on Linux are managed by Coven; use `coven agent extensions plan --provider pi --extension-root ABS` / `coven agent extensions install --provider pi --extension-root ABS`. The absolute extension root must already exist; Coven does not discover roots or create directories. To migrate verified legacy Kitsune v7, explicitly pass `--migrate-kitsune`; mutation requires explicit `--apply` on Linux. Coven installs coven-agent-reference.ts separately from legacy kitsune-agent-state.ts. This is reference-only session_start/getSessionId capture: migration drops legacy working/blocked/idle reporting. Eligibility requires hasUI === true (TUI or RPC), which is not root or native activation proof. Installed configuration alone does not prove operational capture. Kitsune uninstall is legacy cleanup only.";
const CLAUDE_HOOK_INSTALL_NAME: &str = if cfg!(windows) {
    "kitsune-agent-state.ps1"
} else {
    "kitsune-agent-state.sh"
};
// Legacy non-Linux installer assets; historical test fixtures only on Linux.
#[cfg(any(test, not(target_os = "linux")))]
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
// Legacy non-Linux installer assets; historical test fixtures only on Linux.
#[cfg(any(test, not(target_os = "linux")))]
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
// Legacy non-Linux installer assets; historical test fixtures only on Linux.
#[cfg(any(test, not(target_os = "linux")))]
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
#[cfg(any(test, not(target_os = "linux")))]
const OPENCODE_PLUGIN_ASSET: &str = include_str!("assets/opencode/kitsune-agent-state.js");
const OPENCODE_INTEGRATION_VERSION: u32 = 9;
#[cfg(target_os = "linux")]
const OPENCODE_COVEN_GUIDANCE: &str = "OpenCode plugins on Linux are managed by Coven; use `coven agent extensions plan --provider opencode --extension-root ABS` / `coven agent extensions install --provider opencode --extension-root ABS`. The explicit absolute extension root must already exist; no HOME auto-discovery or JSON/package changes. To migrate verified legacy Kitsune v9, explicitly pass `--migrate-kitsune`; mutation requires explicit `--apply`. Coven installs coven-agent-reference.js separately from legacy kitsune-agent-state.js within that single selected root, without cross-root deduplication. Migration drops legacy working/blocked/idle reporting: reference-only capture leaves fresh lifecycle state unknown. Supported source pin: anomalyco/opencode v1.19.5 (810c0eba1a7348036f8bdf61e6632d0c36c6876c), function plugins only; changed dev API 611cee9 is UNSUPPORTED, not installed-binary certification. The server-side plugin captures session.created/session.updated full info.id for the exact directory instance, which can see multiple roots, not the selected chat/pane; no SDK enumeration or universal resume capture. After explicit manual apply, stop/restart the native server, not merely attach a TUI; no automatic restart. Installed files do not prove operational capture. Kitsune uninstall is legacy cleanup only.";
const INTEGRATION_VERSION_MARKER: &str = "KITSUNE_INTEGRATION_VERSION=";

pub(crate) const INSTALL_WARNING_PREFIX: &str = "warning:";

#[cfg(test)]
mod tests;
