pub mod client;
mod event_hub;
pub mod schema;
mod server;
mod status;
mod subscriptions;
mod wait;

pub use event_hub::EventHub;
pub(crate) use server::cancel_inactive_pane_graphics_streams;
pub use server::{start_server, start_server_with_capabilities, ServerHandle};
pub use status::{read_runtime_status_at, RuntimeStatus};

use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::api::schema::{Method, Request};

pub const SOCKET_PATH_ENV_VAR: &str = crate::product::SOCKET_PATH_ENV_VAR;

/// Global presentation/administrative requests do not need a terminal workspace.
pub(crate) fn request_skips_default_workspace(request: &Request) -> bool {
    schema::legacy_agent_retired_method(&request.method).is_some()
        || matches!(
            &request.method,
            Method::ServerStop(_)
                | Method::ServerAgentManifests(_)
                | Method::ServerReloadAgentManifests(_)
                | Method::TerminalInspect(_)
                | Method::TerminalFocus(_)
                | Method::ServerLiveHandoff(_)
                | Method::ViewOpen(_)
                | Method::ViewReplace(_)
                | Method::ViewPoll(_)
                | Method::ViewClose(_)
        )
}

pub(crate) fn request_changes_ui(request: &Request) -> bool {
    if schema::legacy_agent_retired_method(&request.method).is_some() {
        return false;
    }
    matches!(
        &request.method,
        Method::ServerReloadConfig(_)
            | Method::TerminalFocus(_)
            | Method::ViewOpen(_)
            | Method::ViewReplace(_)
            | Method::ViewPoll(_)
            | Method::ViewClose(_)
            | Method::NotificationShow(_)
            | Method::WorkspaceCreate(_)
            | Method::WorkspaceFocus(_)
            | Method::WorkspaceRename(_)
            | Method::WorkspaceMove(_)
            | Method::WorkspaceReportMetadata(_)
            | Method::WorkspaceClose(_)
            | Method::WorktreeCreate(_)
            | Method::WorktreeOpen(_)
            | Method::WorktreeRemove(_)
            | Method::TabCreate(_)
            | Method::TabFocus(_)
            | Method::TabRename(_)
            | Method::TabMove(_)
            | Method::TabClose(_)
            | Method::LayoutApply(_)
            | Method::LayoutSetSplitRatio(_)
            | Method::PaneSplit(_)
            | Method::PaneSwap(_)
            | Method::PaneMove(_)
            | Method::PaneZoom(_)
            | Method::PaneFocusDirection(_)
            | Method::PaneResize(_)
            | Method::PaneFocus(_)
            | Method::PaneRename(_)
            | Method::PaneAnnotate(_)
            | Method::PaneGraphicsSet(_)
            | Method::PaneGraphicsClear(_)
            | Method::PaneGraphicsStream(_)
            | Method::PaneGraphicsStreamSet(_)
            | Method::PaneGraphicsStreamOpen(_)
            | Method::PaneGraphicsStreamClose(_)
            | Method::PaneReportAgent(_)
            | Method::PaneReportAgentSession(_)
            | Method::PaneReportMetadata(_)
            | Method::PaneClearAgentAuthority(_)
            | Method::PaneReleaseAgent(_)
            | Method::PaneClose(_)
            | Method::PopupClose(_)
    )
}

pub struct ApiRequestMessage {
    pub request: Request,
    pub respond_to: std::sync::mpsc::Sender<String>,
    pub response_write_complete: Option<std::sync::mpsc::Receiver<()>>,
}

pub type ApiRequestSender = mpsc::UnboundedSender<ApiRequestMessage>;

pub fn socket_path() -> PathBuf {
    crate::session::active_api_socket_path()
}

#[cfg(test)]
mod integration_views_tests {
    use super::*;

    #[test]
    fn generic_events_and_uninstall_are_not_global_retirement_tombstones() {
        for value in [
            serde_json::json!({"id":"generic", "method":"events.subscribe", "params":{"subscriptions":[{"type":"pane.scroll_changed", "pane_id":"w1:p1"}, {"type":"workspace.created"}]}}),
            serde_json::json!({"id":"generic", "method":"events.subscribe", "params":{"subscriptions":[{"type":"pane.output_matched", "pane_id":"w1:p1", "source":"recent", "match":{"type":"substring", "value":"ready"}}]}}),
            serde_json::json!({"id":"generic", "method":"events.wait", "params":{"match_event":{"event":"workspace_created"}}}),
            serde_json::json!({"id":"generic", "method":"integration.uninstall", "params":{"target":"pi"}}),
        ] {
            let request: Request = serde_json::from_value(value).unwrap();
            assert!(schema::legacy_agent_retired_method(&request.method).is_none());
            assert!(schema::retired_request_response(&request).is_none());
            assert!(!request_skips_default_workspace(&request));
        }
    }

    #[test]
    fn hook_ingress_and_generic_metadata_are_not_global_retirement_tombstones() {
        for method in [
            "pane.report_agent",
            "pane.report_agent_session",
            "pane.clear_agent_authority",
            "pane.release_agent",
            "pane.report_metadata",
        ] {
            let request: Request = serde_json::from_value(serde_json::json!({
                "id":"hook", "method":method,
                "params":{"pane_id":"w1:p1", "source":"custom:test", "agent":"pi", "state":"idle"},
            }))
            .unwrap();
            assert!(schema::legacy_agent_retired_method(&request.method).is_none());
            assert!(schema::retired_request_response(&request).is_none());
            assert!(request_changes_ui(&request));
            assert!(!request_skips_default_workspace(&request));
        }
    }

    #[test]
    fn integration_views_requests_repaint_and_skip_workspace_bootstrap() {
        for (method, params) in [
            (
                "integration.view.open",
                serde_json::json!({"source":"a", "version":1}),
            ),
            (
                "integration.view.replace",
                serde_json::json!({"source":"a", "lease":"l", "epoch":"e", "revision":1, "items":[]}),
            ),
            (
                "integration.view.poll",
                serde_json::json!({"source":"a", "lease":"l", "epoch":"e"}),
            ),
            (
                "integration.view.close",
                serde_json::json!({"source":"a", "lease":"l", "epoch":"e"}),
            ),
        ] {
            let request: Request = serde_json::from_value(
                serde_json::json!({"id":"test", "method":method, "params":params}),
            )
            .unwrap();
            assert!(request_skips_default_workspace(&request));
            assert!(request_changes_ui(&request));
        }
    }
}
