use crate::api::schema::{IntegrationInstallResult, IntegrationUninstallResult, ResponseResult};
use crate::app::App;

use super::responses::{encode_error, encode_success};

impl App {
    pub(super) fn handle_integration_install(
        &mut self,
        id: String,
        params: crate::api::schema::IntegrationInstallParams,
    ) -> String {
        if self.state.external_agent_management {
            return encode_error(
                id,
                "external_agent_management",
                "legacy integration installation is disabled",
            );
        }
        let target = params.target;
        let messages = match crate::integration::install_target(target) {
            Ok(messages) => messages,
            Err(err) => return encode_error(id, "integration_install_failed", err.to_string()),
        };

        encode_success(
            id,
            ResponseResult::IntegrationInstall {
                target,
                details: IntegrationInstallResult { messages },
            },
        )
    }

    pub(super) fn handle_integration_uninstall(
        &mut self,
        id: String,
        params: crate::api::schema::IntegrationUninstallParams,
    ) -> String {
        let target = params.target;
        let messages = match crate::integration::uninstall_target(target) {
            Ok(messages) => messages,
            Err(err) => return encode_error(id, "integration_uninstall_failed", err.to_string()),
        };

        encode_success(
            id,
            ResponseResult::IntegrationUninstall {
                target,
                details: IntegrationUninstallResult { messages },
            },
        )
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn opencode_install_api_returns_coven_migration_error() {
        let mut app = crate::app::test_support::app_from_state(crate::app::AppState::test_new());
        let request = serde_json::from_value(serde_json::json!({
            "id": "retired-opencode-install",
            "method": "integration.install",
            "params": {"target": "opencode"}
        }))
        .unwrap();
        let response: serde_json::Value =
            serde_json::from_str(&app.handle_api_request(request)).unwrap();
        assert_eq!(response["id"], "retired-opencode-install");
        assert_eq!(response["error"]["code"], "integration_install_failed");
        let message = response["error"]["message"].as_str().unwrap();
        for expected in [
            "on Linux",
            "managed by Coven",
            "coven agent extensions plan",
            "coven agent extensions install",
            "--provider opencode",
            "--extension-root ABS",
            "--migrate-kitsune",
            "--apply",
            "drops legacy working/blocked/idle",
            "fresh lifecycle state unknown",
            "v1.19.5",
            "611cee9 is UNSUPPORTED",
            "not installed-binary certification",
            "session.created/session.updated full info.id",
            "exact directory instance",
            "multiple roots",
            "not the selected chat/pane",
            "no SDK enumeration or universal resume capture",
            "stop/restart the native server",
            "not merely attach a TUI",
            "no automatic restart",
        ] {
            assert!(message.contains(expected), "missing {expected}: {message}");
        }
        assert!(response.get("result").is_none());
        assert_eq!(app.terminal_runtimes.len(), 0);
    }

    #[test]
    fn pi_install_api_returns_coven_migration_error() {
        let mut app = crate::app::test_support::app_from_state(crate::app::AppState::test_new());
        let request = serde_json::from_value(serde_json::json!({
            "id": "retired-pi-install",
            "method": "integration.install",
            "params": {"target": "pi"}
        }))
        .unwrap();
        let response: serde_json::Value =
            serde_json::from_str(&app.handle_api_request(request)).unwrap();
        assert_eq!(response["id"], "retired-pi-install");
        assert_eq!(response["error"]["code"], "integration_install_failed");
        let message = response["error"]["message"].as_str().unwrap();
        for expected in [
            "on Linux",
            "managed by Coven",
            "coven agent extensions plan",
            "coven agent extensions install",
            "--provider pi",
            "--extension-root ABS",
            "--migrate-kitsune",
            "--apply",
            "session_start/getSessionId",
            "drops legacy working/blocked/idle",
            "hasUI === true",
            "TUI or RPC",
            "not root or native activation proof",
        ] {
            assert!(message.contains(expected), "missing {expected}: {message}");
        }
        assert!(response.get("result").is_none());
        assert_eq!(app.terminal_runtimes.len(), 0);
    }

    #[test]
    fn copilot_install_api_returns_coven_migration_error() {
        let mut app = crate::app::test_support::app_from_state(crate::app::AppState::test_new());
        let request = serde_json::from_value(serde_json::json!({
            "id": "retired-copilot-install",
            "method": "integration.install",
            "params": {"target": "copilot"}
        }))
        .unwrap();

        let response: serde_json::Value =
            serde_json::from_str(&app.handle_api_request(request)).unwrap();

        assert_eq!(response["id"], "retired-copilot-install");
        assert_eq!(response["error"]["code"], "integration_install_failed");
        let message = response["error"]["message"].as_str().unwrap();
        for expected in [
            "on Linux",
            "managed by Coven",
            "coven agent hooks plan",
            "coven agent hooks install",
            "--provider copilot",
            "--config-root",
            "--migrate-kitsune",
            "direct hooks.SessionStart",
            "reference-only startup capture",
            "not native approval",
            "does not prove operational capture",
        ] {
            assert!(message.contains(expected), "missing {expected}: {message}");
        }
        assert!(response.get("result").is_none());
        assert_eq!(app.terminal_runtimes.len(), 0);
    }

    #[test]
    fn codex_install_api_returns_coven_migration_error() {
        let mut app = crate::app::test_support::app_from_state(crate::app::AppState::test_new());
        let request = serde_json::from_value(serde_json::json!({
            "id": "retired-codex-install",
            "method": "integration.install",
            "params": {"target": "codex"}
        }))
        .unwrap();

        let response: serde_json::Value =
            serde_json::from_str(&app.handle_api_request(request)).unwrap();

        assert_eq!(response["id"], "retired-codex-install");
        assert_eq!(response["error"]["code"], "integration_install_failed");
        let message = response["error"]["message"].as_str().unwrap();
        assert!(message.contains("managed by Coven"));
        assert!(message.contains("coven agent hooks plan"));
        assert!(message.contains("coven agent hooks install"));
        assert!(message.contains("--migrate-kitsune"));
        assert!(message.contains("user-managed"));
        assert!(message.contains("does not prove operational capture"));
        assert!(response.get("result").is_none());
        assert_eq!(app.terminal_runtimes.len(), 0);
    }

    #[test]
    fn claude_install_api_returns_coven_migration_error() {
        let mut app = crate::app::test_support::app_from_state(crate::app::AppState::test_new());
        let request = serde_json::from_value(serde_json::json!({
            "id": "retired-claude-install",
            "method": "integration.install",
            "params": {"target": "claude"}
        }))
        .unwrap();

        let response: serde_json::Value =
            serde_json::from_str(&app.handle_api_request(request)).unwrap();

        assert_eq!(response["id"], "retired-claude-install");
        assert_eq!(response["error"]["code"], "integration_install_failed");
        let message = response["error"]["message"].as_str().unwrap();
        assert!(message.contains("managed by Coven"));
        assert!(message.contains("coven agent hooks plan"));
        assert!(message.contains("coven agent hooks install"));
        assert!(response.get("result").is_none());
        assert_eq!(app.terminal_runtimes.len(), 0);
    }
}
