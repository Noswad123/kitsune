use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntegrationInstallParams {
    pub target: IntegrationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntegrationUninstallParams {
    pub target: IntegrationTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationTarget {
    Pi,
    Claude,
    Codex,
    Copilot,
    Opencode,
    Djinn,
}

impl IntegrationTarget {
    pub(crate) const ALL: [Self; 6] = [
        Self::Pi,
        Self::Claude,
        Self::Codex,
        Self::Copilot,
        Self::Opencode,
        Self::Djinn,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntegrationInstallResult {
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntegrationUninstallResult {
    pub messages: Vec<String>,
}
