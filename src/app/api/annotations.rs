use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::annotations::{Error, MAX_VALID_UNTIL_UNIX_MS};
use crate::api::schema::{PaneAnnotateParams, ResponseResult};
use crate::app::App;

use super::responses::{encode_error, encode_success};

impl App {
    pub(super) fn handle_pane_annotate(
        &mut self,
        id: String,
        params: PaneAnnotateParams,
    ) -> String {
        self.handle_pane_annotate_with_clock(id, params, Instant::now, SystemTime::now)
    }

    fn handle_pane_annotate_with_clock(
        &mut self,
        id: String,
        params: PaneAnnotateParams,
        mut monotonic: impl FnMut() -> Instant,
        wall: impl FnOnce() -> SystemTime,
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
        let now = monotonic();
        self.state.sync_annotation_deadline(now);
        let ttl = Duration::from_millis(params.ttl_ms);
        let result = if let Some(valid_until) = params.valid_until_unix_ms {
            self.state.annotations.replace_with_timing(
                &params.annotation_epoch,
                &params.source,
                terminal_id,
                params.revision,
                params.text,
                ttl,
                || {
                    // Sample M before W at final dispatch, after the store fences.
                    let mapped_at = monotonic();
                    let deadline = map_annotation_deadline(ttl, valid_until, mapped_at, wall())?;
                    Ok((monotonic(), deadline))
                },
            )
        } else {
            // Legacy acceptance-relative TTL needs no wall-clock sample.
            self.state.annotations.replace(
                &params.annotation_epoch,
                &params.source,
                terminal_id,
                params.revision,
                params.text,
                ttl,
                now,
            )
        };
        if let Err(error) = result {
            let code = match error {
                Error::StaleEpoch => "annotation_epoch_mismatch",
                Error::UnselectedSource => "source_disabled",
                Error::StaleRevision => "annotation_stale_revision",
                Error::InvalidText => "invalid_annotation_text",
                Error::InvalidTtl | Error::DeadlineOverflow => "invalid_annotation_ttl",
                Error::InvalidDeadline => "invalid_annotation_deadline",
                Error::Expired => "annotation_expired",
                Error::ClockUnavailable => "annotation_clock_unavailable",
                Error::Capacity => "annotation_capacity",
                Error::TimeRegression => "annotation_time_regression",
                Error::InvalidSources => "invalid_annotation_sources",
                #[cfg(test)]
                Error::InvalidTarget => "invalid_annotation_target",
            };
            return encode_error(id, code, format!("Annotation rejected: {error:?}"));
        }
        self.state.sync_annotation_deadline(monotonic());
        encode_success(id, ResponseResult::Ok {})
    }
}

fn map_annotation_deadline(
    ttl: Duration,
    valid_until_unix_ms: u64,
    mapped_at: Instant,
    wall: SystemTime,
) -> Result<Instant, Error> {
    if valid_until_unix_ms > MAX_VALID_UNTIL_UNIX_MS {
        return Err(Error::InvalidDeadline);
    }
    let wall = wall
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::ClockUnavailable)?;
    let remaining = Duration::from_millis(valid_until_unix_ms)
        .checked_sub(wall)
        .filter(|remaining| !remaining.is_zero())
        .ok_or(Error::Expired)?;
    // Preserve sub-ms validity and cap before addition; never round or restart it.
    mapped_at
        .checked_add(ttl.min(remaining))
        .ok_or(Error::DeadlineOverflow)
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

    fn send_at(
        app: &mut App,
        request: PaneAnnotateParams,
        mapped_at: Instant,
        wall: SystemTime,
        insertion_at: Instant,
    ) -> serde_json::Value {
        let mut samples = [mapped_at, mapped_at, insertion_at, insertion_at].into_iter();
        serde_json::from_str(&app.handle_pane_annotate_with_clock(
            "annotation".into(),
            request,
            || samples.next().expect("unexpected clock sample"),
            || wall,
        ))
        .unwrap()
    }

    #[test]
    fn annotation_deadline_mapping_uses_full_durations_and_caps_before_addition() {
        let now = Instant::now();
        let wall = UNIX_EPOCH + Duration::from_millis(10_000);
        for (deadline, ttl, expected) in [
            (10_001, 5000, Duration::from_millis(1)),
            (20_000, 5000, Duration::from_millis(5000)),
            (MAX_VALID_UNTIL_UNIX_MS, 300000, Duration::from_secs(300)),
        ] {
            assert_eq!(
                map_annotation_deadline(Duration::from_millis(ttl), deadline, now, wall),
                Ok(now + expected)
            );
        }
        for deadline in [0, 9999, 10_000] {
            assert_eq!(
                map_annotation_deadline(Duration::from_secs(5), deadline, now, wall),
                Err(Error::Expired)
            );
        }
        assert_eq!(
            map_annotation_deadline(
                Duration::from_secs(5),
                MAX_VALID_UNTIL_UNIX_MS + 1,
                now,
                wall
            ),
            Err(Error::InvalidDeadline)
        );
        assert_eq!(
            map_annotation_deadline(
                Duration::from_secs(5),
                10_001,
                now,
                UNIX_EPOCH - Duration::from_nanos(1)
            ),
            Err(Error::ClockUnavailable)
        );
        assert_eq!(
            map_annotation_deadline(
                Duration::from_secs(5),
                10_001,
                now,
                wall + Duration::from_nanos(999_500)
            ),
            Ok(now + Duration::from_nanos(500))
        );
    }

    #[test]
    fn annotation_deadline_queued_expired_writes_and_clears_are_atomic() {
        for clear in [false, true] {
            let (mut app, pane, terminal) = fixture();
            let now = Instant::now();
            let id = app
                .state
                .terminal_id_for_pane(0, app.state.workspaces[0].tabs[0].root_pane)
                .unwrap();
            let original = params(&app, &pane, &terminal, 1);
            assert_eq!(
                send_at(&mut app, original, now, UNIX_EPOCH, now)["result"]["type"],
                "ok"
            );
            let old_deadline = app.state.next_annotation_deadline;
            // Decode while still fresh; dispatch occurs at or beyond its source deadline.
            let mut value = serde_json::to_value(params(&app, &pane, &terminal, 2)).unwrap();
            value["valid_until_unix_ms"] = serde_json::json!(10_001);
            if clear {
                value["text"] = serde_json::Value::Null;
            }
            let queued: PaneAnnotateParams = serde_json::from_value(value).unwrap();
            for dispatched_ms in [10_001, 10_002] {
                let response = send_at(
                    &mut app,
                    queued.clone(),
                    now + Duration::from_millis(1),
                    UNIX_EPOCH + Duration::from_millis(dispatched_ms),
                    now + Duration::from_millis(1),
                );
                assert_eq!(response["error"]["code"], "annotation_expired");
                assert_eq!(
                    app.state
                        .annotations
                        .project(&id, now + Duration::from_millis(1)),
                    Some("passed")
                );
                assert_eq!(app.state.next_annotation_deadline, old_deadline);
            }
            // Same rejected revision can be used for a genuinely valid update/cleanup.
            let mut valid = queued;
            valid.valid_until_unix_ms = None;
            assert_eq!(
                send_at(
                    &mut app,
                    valid,
                    now + Duration::from_millis(1),
                    UNIX_EPOCH,
                    now + Duration::from_millis(1)
                )["result"]["type"],
                "ok"
            );
            assert_eq!(
                app.state
                    .annotations
                    .project(&id, now + Duration::from_millis(1)),
                if clear { None } else { Some("passed") }
            );
            app.state.assert_invariants_for_test();
        }
    }

    #[test]
    fn annotation_deadline_dispatch_and_insertion_delay_never_restart_validity() {
        let (mut app, pane, terminal) = fixture();
        let now = Instant::now();
        let id = app
            .state
            .terminal_id_for_pane(0, app.state.workspaces[0].tabs[0].root_pane)
            .unwrap();
        let mut request = params(&app, &pane, &terminal, 1);
        request.valid_until_unix_ms = Some(10_001);
        let wall = UNIX_EPOCH + Duration::from_millis(10_000) + Duration::from_nanos(999_500);
        // Positive 500ns validity is not rounded up to the 1ms minimum request TTL.
        assert_eq!(
            send_at(&mut app, request.clone(), now, wall, now)["result"]["type"],
            "ok"
        );
        assert_eq!(app.state.annotations.project(&id, now), Some("passed"));
        let expiry = now + Duration::from_nanos(500);
        assert_eq!(app.state.next_annotation_deadline, Some(expiry));
        assert_eq!(app.state.annotations.project(&id, expiry), None);
        request.revision = 2;
        // Pause between mapping and insertion reaches/passes the already mapped deadline.
        for delay in [500, 501] {
            assert_eq!(
                send_at(
                    &mut app,
                    request.clone(),
                    now,
                    wall,
                    now + Duration::from_nanos(delay)
                )["error"]["code"],
                "annotation_expired"
            );
            assert_eq!(app.state.next_annotation_deadline, Some(expiry));
        }
        // Dispatch 4 seconds after observation retains only the final 1 second.
        request.valid_until_unix_ms = Some(15_000);
        let dispatched = now + Duration::from_secs(4);
        assert_eq!(
            send_at(
                &mut app,
                request,
                dispatched,
                UNIX_EPOCH + Duration::from_millis(14_000),
                dispatched + Duration::from_millis(1)
            )["result"]["type"],
            "ok"
        );
        assert_eq!(
            app.state.next_annotation_deadline,
            Some(now + Duration::from_secs(5))
        );
        // Projection uses only the fixed monotonic cap; later wall rollback cannot renew it.
        assert!(app.expire_due_annotations(now + Duration::from_secs(5)));
        assert_eq!(
            app.state
                .annotations
                .project(&id, now + Duration::from_secs(5)),
            None
        );
        assert!(!app.expire_due_annotations(now + Duration::from_secs(5)));
        app.state.assert_invariants_for_test();
    }

    #[test]
    fn annotation_deadline_errors_and_legacy_clock_independence() {
        let (mut app, pane, terminal) = fixture();
        let now = Instant::now();
        for (deadline, wall, error) in [
            (
                MAX_VALID_UNTIL_UNIX_MS + 1,
                UNIX_EPOCH,
                "invalid_annotation_deadline",
            ),
            (
                1,
                UNIX_EPOCH - Duration::from_nanos(1),
                "annotation_clock_unavailable",
            ),
        ] {
            let mut request = params(&app, &pane, &terminal, 1);
            request.valid_until_unix_ms = Some(deadline);
            assert_eq!(
                send_at(&mut app, request, now, wall, now)["error"]["code"],
                error
            );
        }
        for ttl in [0, 300_001] {
            let mut request = params(&app, &pane, &terminal, 1);
            request.valid_until_unix_ms = Some(10_000);
            request.ttl_ms = ttl;
            assert_eq!(
                send_at(&mut app, request, now, UNIX_EPOCH, now)["error"]["code"],
                "invalid_annotation_ttl"
            );
        }
        assert_eq!(app.state.next_annotation_deadline, None);
        let request = params(&app, &pane, &terminal, 1);
        let response = app.handle_pane_annotate_with_clock(
            "legacy".into(),
            request,
            || now,
            || panic!("legacy writes must not sample the wall clock"),
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&response).unwrap()["result"]["type"],
            "ok"
        );
        assert_eq!(
            app.state.next_annotation_deadline,
            Some(now + Duration::from_secs(300))
        );
        app.state.assert_invariants_for_test();
    }

    #[test]
    fn annotation_deadline_clock_mapping_does_not_promise_preacceptance_skew_safety() {
        let now = Instant::now();
        let ttl = Duration::from_secs(300);
        let deadline = 15_000;
        let compatible =
            map_annotation_deadline(ttl, deadline, now, UNIX_EPOCH + Duration::from_secs(10))
                .unwrap();
        let server_behind =
            map_annotation_deadline(ttl, deadline, now, UNIX_EPOCH + Duration::from_secs(9))
                .unwrap();
        assert_eq!(compatible, now + Duration::from_secs(5));
        assert_eq!(server_behind, now + Duration::from_secs(6));
        // The mapping cannot establish clock quality; projection never re-reads W.
    }

    #[test]
    fn annotation_deadline_valid_clear_retains_priority_and_revision_fences() {
        let (mut app, pane, terminal) = fixture();
        app.state
            .annotations
            .reconfigure(vec!["build".into(), "tests".into()])
            .unwrap();
        let now = Instant::now();
        let wall = UNIX_EPOCH + Duration::from_secs(10);
        let id = app
            .state
            .terminal_id_for_pane(0, app.state.workspaces[0].tabs[0].root_pane)
            .unwrap();
        let mut lower = params(&app, &pane, &terminal, 1);
        lower.source = "tests".into();
        assert_eq!(
            send_at(&mut app, lower, now, wall, now)["result"]["type"],
            "ok"
        );
        let mut clear = params(&app, &pane, &terminal, 1);
        clear.text = None;
        clear.valid_until_unix_ms = Some(10_001);
        assert_eq!(
            send_at(&mut app, clear.clone(), now, wall, now)["result"]["type"],
            "ok"
        );
        assert_eq!(app.state.annotations.project(&id, now), None);
        assert_eq!(
            app.state
                .annotations
                .project(&id, now + Duration::from_secs(1)),
            None
        );
        let mut replay = clear;
        replay.text = Some("replay".into());
        replay.valid_until_unix_ms = Some(20_000);
        assert_eq!(
            send_at(
                &mut app,
                replay,
                now + Duration::from_secs(1),
                wall + Duration::from_secs(1),
                now + Duration::from_secs(1)
            )["error"]["code"],
            "annotation_stale_revision"
        );
        assert_eq!(app.state.next_annotation_deadline, None);
        app.state.assert_invariants_for_test();
    }

    #[test]
    fn annotation_deadline_preserves_handler_fences_before_wall_sampling() {
        let (mut app, pane, terminal) = fixture();
        let now = Instant::now();
        let original = params(&app, &pane, &terminal, 1);
        assert_eq!(
            send_at(&mut app, original, now, UNIX_EPOCH, now)["result"]["type"],
            "ok"
        );
        let old_deadline = app.state.next_annotation_deadline;
        for error in [
            "annotation_epoch_mismatch",
            "source_disabled",
            "pane_not_found",
            "terminal_mismatch",
            "annotation_stale_revision",
            "invalid_annotation_text",
            "invalid_annotation_ttl",
        ] {
            let mut request = params(&app, &pane, &terminal, 2);
            request.valid_until_unix_ms = Some(MAX_VALID_UNTIL_UNIX_MS);
            match error {
                "annotation_epoch_mismatch" => request.annotation_epoch = "old".into(),
                "source_disabled" => request.source = "other".into(),
                "pane_not_found" => request.pane_id = "shell".into(),
                "terminal_mismatch" => request.expected_terminal_id = "replacement".into(),
                "annotation_stale_revision" => request.revision = 1,
                "invalid_annotation_text" => request.text = Some(String::new()),
                "invalid_annotation_ttl" => request.ttl_ms = 0,
                _ => unreachable!(),
            }
            let response = app.handle_pane_annotate_with_clock(
                "fence".into(),
                request,
                || now,
                || panic!("wall clock sampled before {error} fence"),
            );
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&response).unwrap()["error"]["code"],
                error
            );
            assert_eq!(app.state.next_annotation_deadline, old_deadline);
            app.state.assert_invariants_for_test();
        }
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
