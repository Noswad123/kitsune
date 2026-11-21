use crate::api::schema::{
    legacy_agent_retirement_error, IntegrationUninstallResult, ResponseResult,
};
use crate::app::App;

use super::responses::{encode_error, encode_error_body, encode_success};

impl App {
    pub(super) fn handle_integration_install(
        &mut self,
        id: String,
        params: crate::api::schema::IntegrationInstallParams,
    ) -> String {
        // Keep direct typed callers behind the same side-effect-free library boundary.
        let _ = crate::integration::install_target(params.target);
        encode_error_body(id, legacy_agent_retirement_error("integration.install"))
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

#[cfg(test)]
mod tests {
    use crate::api::schema::{IntegrationInstallParams, IntegrationTarget};

    #[test]
    fn direct_install_is_retired_for_every_target_in_both_ownership_modes() {
        for external in [false, true] {
            let mut state = crate::app::AppState::test_new();
            state.external_agent_management = external;
            let mut app = crate::app::test_support::app_from_state(state);
            for target in IntegrationTarget::ALL {
                let response = app.handle_integration_install(
                    "retired-install".to_string(),
                    IntegrationInstallParams { target },
                );
                let response: crate::api::schema::ErrorResponse =
                    serde_json::from_str(&response).unwrap();
                assert_eq!(response.id, "retired-install");
                assert_eq!(
                    response.error,
                    super::legacy_agent_retirement_error("integration.install")
                );
                assert_eq!(app.terminal_runtimes.len(), 0);
            }
        }
    }
}
