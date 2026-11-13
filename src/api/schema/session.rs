use serde::{Deserialize, Serialize};

use super::agents::AgentInfo;
use super::panes::{PaneInfo, PaneLayoutSnapshot};
use super::tabs::TabInfo;
use super::workspaces::WorkspaceInfo;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AnnotationInfo {
    pub epoch: String,
    pub sources: Vec<String>,
    /// Supports source validity deadlines on pane.annotate; not a clock-quality guarantee.
    #[serde(default, skip_serializing_if = "super::is_false")]
    pub valid_until_unix_ms: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CommandSubmissionInfo {
    pub version: u32,
}

/// Acquisition semantics of pane.process_info, not a new process provider.
/// This does not promise a complete process tree or stable process generations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProcessObservationInfo {
    pub version: u32,
    pub platform: String,
    pub scope: String,
    pub selection: String,
    /// Named argument-decoding policy, not native authentication, acquisition quality,
    /// completeness, or process-generation assurance. Version 2 names the reviewed
    /// Darwin LP64 layout; version 1 does not provide that argument guarantee.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument_policy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionSnapshot {
    pub version: String,
    pub protocol: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<AnnotationInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_submission: Option<CommandSubmissionInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_observation: Option<ProcessObservationInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_views: Option<super::IntegrationViewsInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_tab_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_pane_id: Option<String>,
    pub workspaces: Vec<WorkspaceInfo>,
    pub tabs: Vec<TabInfo>,
    pub panes: Vec<PaneInfo>,
    pub layouts: Vec<PaneLayoutSnapshot>,
    pub agents: Vec<AgentInfo>,
}
