use std::time::Instant;

use crate::api::schema::{ResponseResult, ViewOpenParams, ViewOwnerParams, ViewReplaceParams};
use crate::app::App;

use super::responses::{encode_error, encode_success};

impl App {
    pub(super) fn handle_view_open(&mut self, id: String, params: ViewOpenParams) -> String {
        let result = self.state.integration_views.open(&params, Instant::now());
        self.state.sync_annotation_deadline(Instant::now());
        match result {
            Ok(opened) => encode_success(
                id,
                ResponseResult::ViewOpened {
                    lease: opened.lease,
                    epoch: opened.epoch,
                    revision: opened.revision,
                    ttl_ms: opened.ttl_ms,
                },
            ),
            Err(code) => encode_error(id, code, code),
        }
    }

    pub(super) fn handle_view_replace(&mut self, id: String, params: ViewReplaceParams) -> String {
        let result = self
            .state
            .integration_views
            .replace(&params, Instant::now());
        self.state.sync_annotation_deadline(Instant::now());
        match result {
            Ok(replaced) => encode_success(
                id,
                ResponseResult::ViewReplaced {
                    epoch: replaced.epoch,
                    revision: replaced.revision,
                },
            ),
            Err(code) => encode_error(id, code, code),
        }
    }

    pub(super) fn handle_view_poll(&mut self, id: String, params: ViewOwnerParams) -> String {
        let result = self.state.integration_views.poll(&params, Instant::now());
        self.state.sync_annotation_deadline(Instant::now());
        match result {
            Ok(polled) => encode_success(
                id,
                ResponseResult::ViewIntents {
                    epoch: polled.epoch,
                    revision: polled.revision,
                    intents: polled.intents,
                    ttl_ms: polled.ttl_ms,
                },
            ),
            Err(code) => encode_error(id, code, code),
        }
    }

    pub(super) fn handle_view_close(&mut self, id: String, params: ViewOwnerParams) -> String {
        let result = self.state.integration_views.close(&params, Instant::now());
        self.state.sync_annotation_deadline(Instant::now());
        match result {
            Ok(()) => encode_success(id, ResponseResult::ViewClosed {}),
            Err(code) => encode_error(id, code, code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::schema::{ErrorResponse, IntegrationViewItem, SuccessResponse};

    #[test]
    fn integration_views_handlers_return_typed_results_and_static_errors() {
        let mut state = crate::app::AppState::test_new();
        state
            .integration_views
            .reconfigure(vec!["test".into()])
            .unwrap();
        let mut app = crate::app::test_support::app_from_state(state);
        let response = app.handle_view_open(
            "open".into(),
            ViewOpenParams {
                source: "test".into(),
                version: 1,
            },
        );
        let response: SuccessResponse = serde_json::from_str(&response).unwrap();
        assert_eq!(response.id, "open");
        let ResponseResult::ViewOpened {
            lease,
            epoch,
            revision,
            ttl_ms,
        } = response.result
        else {
            panic!("expected view_opened");
        };
        assert_eq!((revision, ttl_ms), (0, 5000));
        let owner = ViewOwnerParams {
            source: "test".into(),
            lease,
            epoch,
        };
        let replacement = ViewReplaceParams {
            source: owner.source.clone(),
            lease: owner.lease.clone(),
            epoch: owner.epoch.clone(),
            revision: 1,
            items: vec![IntegrationViewItem {
                id: "item".into(),
                label: "Label".into(),
                detail: String::new(),
            }],
        };
        let response: SuccessResponse =
            serde_json::from_str(&app.handle_view_replace("replace".into(), replacement)).unwrap();
        assert_eq!(
            response.result,
            ResponseResult::ViewReplaced {
                epoch: owner.epoch.clone(),
                revision: 1
            }
        );
        assert!(app.state.integration_views.activate(
            &owner.source,
            &owner.epoch,
            1,
            "item",
            Instant::now()
        ));
        let response = app.handle_view_poll("poll".into(), owner.clone());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&response).unwrap()["result"]["type"],
            "view_intents"
        );
        let response: SuccessResponse = serde_json::from_str(&response).unwrap();
        let ResponseResult::ViewIntents {
            epoch,
            revision,
            intents,
            ttl_ms,
        } = response.result
        else {
            panic!("expected view_intents");
        };
        assert_eq!(epoch, owner.epoch);
        assert_eq!((revision, ttl_ms, intents.len()), (1, 5000, 1));
        let response: SuccessResponse =
            serde_json::from_str(&app.handle_view_close("close".into(), owner.clone())).unwrap();
        assert_eq!(response.result, ResponseResult::ViewClosed {});
        let response: ErrorResponse =
            serde_json::from_str(&app.handle_view_poll("expired".into(), owner)).unwrap();
        assert_eq!(
            response.error.code,
            "integration_view_lease_invalid_or_expired"
        );
        assert_eq!(response.error.message, response.error.code);
    }
}
