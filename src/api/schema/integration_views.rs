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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[schemars(with = "IntegrationViewPresentation")]
    pub presentation: Option<IntegrationViewPresentation>,
}

// Missing is neutral; explicit null is malformed, including on legacy leases.
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrationViewPresentation {
    pub label: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    #[schemars(with = "IntegrationViewStatus")]
    pub status: Option<IntegrationViewStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationViewStatus {
    Pending,
    Running,
    Attention,
    Error,
    Success,
    Unknown,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_versions: Option<Vec<u64>>,
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
    fn integration_presentation_strict_optional_nonnullable_json() {
        for invalid in [
            r#"null"#,
            r#"{"label":"Kit","status":null}"#,
            r#"{"label":"Kit","status":"idle"}"#,
            r#"{"label":"Kit","status":"running","status":"error"}"#,
            r#"{"label":"Kit","label":"other"}"#,
            r#"{"label":"Kit","icon":"x"}"#,
        ] {
            let raw = format!(r#"{{"id":"i","label":"l","detail":"d","presentation":{invalid}}}"#);
            assert!(
                serde_json::from_str::<IntegrationViewItem>(&raw).is_err(),
                "{raw}"
            );
        }
        assert!(serde_json::from_str::<IntegrationViewItem>(r#"{"id":"i","label":"l","detail":"d","presentation":{"label":"Kit"},"presentation":{"label":"Kit"}}"#).is_err());
        for status in [
            None,
            Some("pending"),
            Some("running"),
            Some("attention"),
            Some("error"),
            Some("success"),
            Some("unknown"),
        ] {
            let mut p = serde_json::json!({"label":"Kit"});
            if let Some(status) = status {
                p["status"] = status.into();
            }
            let item: IntegrationViewItem = serde_json::from_value(
                serde_json::json!({"id":"i","label":"l","detail":"d","presentation":p}),
            )
            .unwrap();
            assert_eq!(
                item.presentation.unwrap().status.is_some(),
                status.is_some()
            );
        }
        let legacy: IntegrationViewItem =
            serde_json::from_str(r#"{"id":"i","label":"l","detail":"d"}"#).unwrap();
        assert!(legacy.presentation.is_none());
        assert_eq!(
            serde_json::to_value(legacy)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            3
        );
        let legacy: IntegrationViewsInfo =
            serde_json::from_str(r#"{"version":1,"sources":[]}"#).unwrap();
        assert_eq!(legacy.supported_versions, None);
    }

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
