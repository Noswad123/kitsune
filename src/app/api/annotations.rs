use std::time::{Duration, Instant};

use crate::annotations::Error;
use crate::api::schema::{PaneAnnotateParams, ResponseResult};
use crate::app::App;

use super::responses::{encode_error, encode_success};

impl App {
    pub(super) fn handle_pane_annotate(
        &mut self,
        id: String,
        params: PaneAnnotateParams,
    ) -> String {
        if params.annotation_epoch != self.state.annotations.epoch() {
            return encode_error(
                id,
                "annotation_epoch_mismatch",
                "Annotation epoch does not match",
            );
        }
        if !self.state.annotations.sources().contains(&params.source) {
            return encode_error(id, "source_disabled", "Annotation source is not selected");
        }
        let Some((ws_idx, pane_id)) = self.parse_current_public_pane_id(&params.pane_id) else {
            return encode_error(id, "pane_not_found", "Current pane not found");
        };
        let Some(terminal_id) = self.state.workspaces[ws_idx].terminal_id(pane_id).cloned() else {
            return encode_error(id, "pane_not_found", "Current pane not found");
        };
        if terminal_id.to_string() != params.expected_terminal_id {
            return encode_error(id, "terminal_mismatch", "Pane terminal does not match");
        }
        if !self.state.terminals.contains_key(&terminal_id) {
            return encode_error(id, "pane_not_found", "Pane terminal not found");
        }
        let now = Instant::now();
        self.state.sync_annotation_deadline(now);
        if let Err(error) = self.state.annotations.replace(
            &params.annotation_epoch,
            &params.source,
            terminal_id,
            params.revision,
            params.text,
            Duration::from_millis(params.ttl_ms),
            now,
        ) {
            let code = match error {
                Error::StaleEpoch => "annotation_epoch_mismatch",
                Error::UnselectedSource => "source_disabled",
                Error::StaleRevision => "annotation_stale_revision",
                Error::InvalidText => "invalid_annotation_text",
                Error::InvalidTtl | Error::DeadlineOverflow => "invalid_annotation_ttl",
                Error::Capacity => "annotation_capacity",
                Error::TimeRegression => "annotation_time_regression",
                Error::InvalidSources => "invalid_annotation_sources",
                #[cfg(test)]
                Error::InvalidTarget => "invalid_annotation_target",
            };
            return encode_error(id, code, format!("Annotation rejected: {error:?}"));
        }
        self.state.sync_annotation_deadline(now);
        encode_success(id, ResponseResult::Ok {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (App, String, String) {
        let mut config = crate::config::Config::default();
        config.annotations.sources = vec!["build".into()];
        let mut app = App::new(
            &config,
            true,
            None,
            tokio::sync::mpsc::unbounded_channel().1,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("shell")];
        app.state.active = Some(0);
        app.state.ensure_test_terminals();
        let pane = app.state.workspaces[0].tabs[0].root_pane;
        let public = app.public_pane_id(0, pane).unwrap();
        let terminal = app.state.terminal_id_for_pane(0, pane).unwrap().to_string();
        (app, public, terminal)
    }

    fn params(app: &App, pane: &str, terminal: &str, revision: u64) -> PaneAnnotateParams {
        serde_json::from_value(serde_json::json!({
            "pane_id": pane, "expected_terminal_id": terminal,
            "annotation_epoch": app.state.annotations.epoch(), "source": "build",
            "revision": revision, "text": "passed", "ttl_ms": 300000
        }))
        .unwrap()
    }

    #[test]
    fn annotations_exact_pin_rejects_replacement_and_noncurrent_locator() {
        let (mut app, pane, terminal) = fixture();
        let mut request = params(&app, &pane, &terminal, 1);
        request.expected_terminal_id = "different".into();
        assert!(app
            .handle_pane_annotate("1".into(), request)
            .contains("terminal_mismatch"));
        let mut request = params(&app, &pane, &terminal, 1);
        request.pane_id = "shell".into();
        assert!(app
            .handle_pane_annotate("2".into(), request)
            .contains("pane_not_found"));
        let request = params(&app, &pane, &terminal, 1);
        assert!(!app
            .handle_pane_annotate("3".into(), request)
            .contains("error"));
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let old_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
        let replacement = crate::terminal::TerminalId::alloc();
        app.state.terminals.insert(
            replacement.clone(),
            crate::terminal::TerminalState::new(replacement.clone(), "/tmp".into()),
        );
        app.state.workspaces[0].tabs[0]
            .panes
            .get_mut(&pane_id)
            .unwrap()
            .attached_terminal_id = replacement.clone();
        let request = params(&app, &pane, &terminal, 2);
        assert!(app
            .handle_pane_annotate("4".into(), request)
            .contains("terminal_mismatch"));
        assert_eq!(
            app.state.annotations.project(&replacement, Instant::now()),
            None
        );
        app.state.remove_unattached_terminal_ids([old_id]);
        app.state.assert_invariants_for_test();
    }

    #[test]
    fn annotations_epoch_source_and_revision_fences_are_atomic() {
        let (mut app, pane, terminal) = fixture();
        let mut request = params(&app, &pane, &terminal, 1);
        request.annotation_epoch = "previous-server".into();
        assert!(app
            .handle_pane_annotate("1".into(), request)
            .contains("annotation_epoch_mismatch"));
        let mut request = params(&app, &pane, &terminal, 1);
        request.source = "other".into();
        assert!(app
            .handle_pane_annotate("2".into(), request)
            .contains("source_disabled"));
        let request = params(&app, &pane, &terminal, 1);
        app.handle_pane_annotate("3".into(), request);
        let request = params(&app, &pane, &terminal, 1);
        assert!(app
            .handle_pane_annotate("4".into(), request)
            .contains("annotation_stale_revision"));
        app.state.assert_invariants_for_test();
    }

    #[test]
    fn annotations_do_not_change_names_or_persisted_session() {
        let (mut app, pane, terminal) = fixture();
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
        app.state
            .terminals
            .get_mut(&id)
            .unwrap()
            .set_manual_label("my shell".into());
        let snapshot = |app: &App| {
            serde_json::to_value(crate::persist::capture(
                &app.state.workspaces,
                &app.state.terminals,
                &app.terminal_runtimes,
                app.state.active,
                app.state.selected,
                app.state.sidebar_width,
                app.state.sidebar_section_split,
                app.state.collapsed_space_keys.clone(),
            ))
            .unwrap()
        };
        let before = snapshot(&app);
        let request = params(&app, &pane, &terminal, 1);
        assert!(!app
            .handle_pane_annotate("1".into(), request)
            .contains("error"));
        assert_eq!(
            app.state.terminals[&id].border_label(false).as_deref(),
            Some("my shell")
        );
        assert_eq!(snapshot(&app), before);
        app.state.assert_invariants_for_test();
    }

    #[test]
    fn annotations_follow_terminal_through_pane_move_and_reject_old_locator() {
        let (mut app, pane, terminal) = fixture();
        app.state.workspaces[0].test_split(ratatui::layout::Direction::Horizontal);
        app.state.ensure_test_terminals();
        let request = params(&app, &pane, &terminal, 1);
        app.handle_pane_annotate("1".into(), request);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.terminal_id_for_pane(0, pane_id).unwrap();
        let moved = app.handle_pane_move(
            "move".into(),
            crate::api::schema::PaneMoveParams {
                pane_id: pane.clone(),
                destination: crate::api::schema::PaneMoveDestination::NewWorkspace {
                    label: Some("destination".into()),
                    tab_label: None,
                },
                focus: true,
            },
        );
        assert!(!moved.contains("error"), "{moved}");
        app.state.sync_annotation_deadline(Instant::now());
        assert_eq!(
            app.state.annotations.project(&terminal_id, Instant::now()),
            Some("passed")
        );
        let (ws_idx, _) = app.find_pane(pane_id).unwrap();
        let current = app.public_pane_id(ws_idx, pane_id).unwrap();
        assert_ne!(current, pane);
        let stale = params(&app, &pane, &terminal, 2);
        assert!(app
            .handle_pane_annotate("stale".into(), stale)
            .contains("pane_not_found"));
        let request = params(&app, &current, &terminal, 2);
        assert!(!app
            .handle_pane_annotate("2".into(), request)
            .contains("error"));
        app.state.assert_invariants_for_test();
    }
}
