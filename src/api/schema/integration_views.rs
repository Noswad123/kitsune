use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewOpenParams {
    pub source: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewOwnerParams {
    pub source: String,
    pub lease: String,
    pub epoch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewReplaceParams {
    pub source: String,
    pub lease: String,
    pub epoch: String,
    pub revision: u64,
    pub items: Vec<IntegrationViewItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrationViewItem {
    pub id: String,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntegrationViewIntent {
    pub epoch: String,
    pub revision: u64,
    pub item_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntegrationViewsInfo {
    pub version: u64,
    pub sources: Vec<IntegrationViewSourceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IntegrationViewSourceInfo {
    pub id: String,
    pub epoch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ViewOpened {
    pub lease: String,
    pub epoch: String,
    pub revision: u64,
    pub ttl_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ViewReplaced {
    pub epoch: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ViewIntents {
    pub epoch: String,
    pub revision: u64,
    pub intents: Vec<IntegrationViewIntent>,
    pub ttl_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integration_views_raw_json_duplicate_fields_are_rejected() {
        // Do not round-trip through Value: a map would hide duplicate keys.
        for raw in [
            r#"{"id":"test","method":"integration.view.open","params":{"source":"a","source":"b","version":1}}"#,
            r#"{"id":"test","method":"integration.view.open","params":{"source":"a","version":1,"version":1}}"#,
            r#"{"id":"test","method":"integration.view.poll","params":{"source":"a","lease":"l","lease":"other","epoch":"e"}}"#,
            r#"{"id":"test","method":"integration.view.replace","params":{"source":"a","lease":"l","epoch":"e","revision":1,"items":[{"id":"i","label":"one","label":"two","detail":"d"}]}}"#,
        ] {
            assert!(serde_json::from_str::<crate::api::schema::Request>(raw).is_err());
        }
    }

    #[test]
    fn integration_views_params_and_items_reject_unknown_fields() {
        assert!(serde_json::from_str::<ViewOpenParams>(
            r#"{"source":"test","version":1,"exec":"no"}"#
        )
        .is_err());
        assert!(serde_json::from_str::<ViewOwnerParams>(
            r#"{"source":"test","lease":"l","epoch":"e","target":"no"}"#
        )
        .is_err());
        assert!(serde_json::from_str::<ViewReplaceParams>(
            r#"{"source":"test","lease":"l","epoch":"e","revision":1,"items":[],"color":"red"}"#
        )
        .is_err());
        assert!(serde_json::from_str::<IntegrationViewItem>(
            r#"{"id":"i","label":"l","detail":"d","exec":"no"}"#
        )
        .is_err());
    }
}
