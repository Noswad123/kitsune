use super::*;
use crate::app::state::AppState;
use crate::detect::{Agent, AgentState};
use crate::layout::PaneId;
use crate::terminal::evidence::{DetectorEpoch, FallbackObservation, ForegroundObservation};
use crate::terminal::{TerminalId, TerminalRuntime};

fn fixture() -> (App, PaneId, TerminalId) {
    let (_, api_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(
        &crate::config::Config::default(),
        true,
        None,
        api_rx,
        crate::api::EventHub::default(),
    );
    app.state = AppState::test_with_adversarial_identity_state();
    app.state.local_sound_playback = false;
    let pane_id = app.state.workspaces[0].tabs[0].root_pane;
    let terminal_id = app
        .find_pane(pane_id)
        .unwrap()
        .1
        .attached_terminal_id
        .clone();
    let runtime = TerminalRuntime::test_with_screen_bytes(80, 24, b"");
    app.terminal_runtimes.insert(terminal_id.clone(), runtime);
    // Set up the session-start protocol without binding any runtime evidence.
    // The first owner-sensitive App event must still bind the actual runtime.
    app.state.handle_app_event(AppEvent::AgentSessionReported {
        pane_id,
        source: "kitsune:djinn".into(),
        agent_label: "djinn".into(),
        seq: Some(0),
        session_ref: crate::agent_resume::AgentSessionRef::id("djinn-session"),
        session_start_source: Some("new".into()),
    });
    app.state.assert_invariants_for_test();
    (app, pane_id, terminal_id)
}

fn confirm(pane_id: PaneId, epoch: DetectorEpoch) -> AppEvent {
    AppEvent::DetectorUpdate {
        pane_id,
        epoch,
        foreground: Some(ForegroundObservation::Confirmed {
            agent: Agent::Djinn,
            process_group_id: Some(42),
        }),
        fallback: None,
        observed_at: Instant::now(),
    }
}

fn report(pane_id: PaneId, seq: u64) -> AppEvent {
    AppEvent::HookStateReported {
        pane_id,
        source: "kitsune:djinn".into(),
        agent_label: "djinn".into(),
        state: AgentState::Working,
        message: None,
        seq: Some(seq),
        session_ref: crate::agent_resume::AgentSessionRef::id("djinn-session"),
    }
}

#[tokio::test]
async fn detector_update_rejects_stale_epoch_and_missing_runtime_before_mutation() {
    let (mut app, pane, terminal) = fixture();
    let revision = app.state.terminals[&terminal].revision;
    assert!(!app.handle_internal_event_with_render_impact(confirm(pane, DetectorEpoch::new())));
    assert_eq!(app.state.terminals[&terminal].revision, revision);
    let runtime = app.terminal_runtimes.remove(&terminal).unwrap();
    assert!(!app.handle_internal_event_with_render_impact(confirm(pane, runtime.detector_epoch())));
    assert_eq!(app.state.terminals[&terminal].revision, revision);
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn detector_update_rejects_old_runtime_and_cross_pane_binding() {
    let (mut app, pane, terminal) = fixture();
    let old_epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    app.terminal_runtimes.insert(
        terminal.clone(),
        TerminalRuntime::test_with_screen_bytes(80, 24, b""),
    );
    let revision = app.state.terminals[&terminal].revision;
    app.handle_internal_event(confirm(pane, old_epoch));
    assert_eq!(app.state.terminals[&terminal].revision, revision);
    let other_pane = app.state.workspaces[0]
        .tabs
        .iter()
        .flat_map(|tab| tab.panes.keys())
        .copied()
        .find(|id| *id != pane)
        .unwrap();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    let other_terminal = app
        .find_pane(other_pane)
        .unwrap()
        .1
        .attached_terminal_id
        .clone();
    let other_revision = app.state.terminals[&other_terminal].revision;
    app.handle_internal_event(confirm(other_pane, epoch));
    assert_eq!(
        app.state.terminals[&other_terminal].revision,
        other_revision
    );
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn hook_delivery_before_queued_current_runtime_probe_preserves_admission() {
    let (mut app, pane, terminal) = fixture();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    let observed_at = Instant::now();
    assert!(observed_at >= epoch.started_at());
    app.handle_internal_event(report(pane, 1));
    assert!(!app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    assert!(
        app.handle_internal_event_with_render_impact(AppEvent::DetectorUpdate {
            pane_id: pane,
            epoch,
            foreground: Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            fallback: None,
            observed_at,
        })
    );
    assert_eq!(
        app.state.terminals[&terminal].foreground_agent_evidence(),
        Some((Agent::Djinn, observed_at))
    );
    assert!(app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    assert_eq!(app.state.terminals[&terminal].state, AgentState::Working);
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn detector_update_same_kind_restored_confirmation_promotes_pending_hook() {
    let (mut app, pane, terminal) = fixture();
    app.state
        .terminals
        .get_mut(&terminal)
        .unwrap()
        .seed_restored_agent_identity(Agent::Djinn);
    app.handle_internal_event(report(pane, 1));
    assert!(!app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    assert!(app.handle_internal_event_with_render_impact(confirm(pane, epoch)));
    assert!(app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    assert_eq!(app.state.terminals[&terminal].state, AgentState::Working);
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn detector_update_evidence_only_is_presentation_quiet() {
    let (mut app, pane, terminal) = fixture();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    app.handle_internal_event(AppEvent::DetectorUpdate {
        pane_id: pane,
        epoch,
        foreground: None,
        fallback: Some(FallbackObservation {
            agent: Some(Agent::Djinn),
            state: AgentState::Working,
            visible_blocker: false,
            visible_working: true,
        }),
        observed_at: Instant::now(),
    });
    let before = app.detector_presentation_snapshot(pane);
    let toast = app.state.toast.clone();
    let seq = app.state.terminals[&terminal].last_agent_state_change_seq;
    assert!(!app.handle_internal_event_with_render_impact(confirm(pane, epoch)));
    assert_eq!(app.detector_presentation_snapshot(pane), before);
    assert_eq!(app.state.toast, toast);
    assert_eq!(
        app.state.terminals[&terminal].last_agent_state_change_seq,
        seq
    );
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn legacy_screen_update_cannot_authorize_hook() {
    let (mut app, pane, terminal) = fixture();
    app.handle_internal_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Djinn),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: true,
        process_exited: false,
        observed_at: Instant::now(),
    });
    app.handle_internal_event(report(pane, 1));
    assert!(!app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn owner_report_after_runtime_replacement_cannot_reuse_prior_confirmation() {
    let (mut app, pane, terminal) = fixture();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    app.handle_internal_event(confirm(pane, epoch));
    app.handle_internal_event(report(pane, 1));
    assert!(app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    app.terminal_runtimes.insert(
        terminal.clone(),
        TerminalRuntime::test_with_screen_bytes(80, 24, b""),
    );
    app.handle_internal_event(report(pane, 2));
    assert!(!app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn owner_sensitive_events_synchronize_missing_runtime_and_publish_changes() {
    for kind in 0..4 {
        let (mut app, pane, terminal) = fixture();
        let epoch = app
            .terminal_runtimes
            .get(&terminal)
            .unwrap()
            .detector_epoch();
        app.handle_internal_event(confirm(pane, epoch));
        app.handle_internal_event(report(pane, 1));
        assert!(app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
        app.terminal_runtimes.remove(&terminal);
        let event_count = app.event_hub.events_after(0).len();
        let event = match kind {
            0 => report(pane, 2),
            1 => AppEvent::AgentSessionReported {
                pane_id: pane,
                source: "kitsune:djinn".into(),
                agent_label: "djinn".into(),
                seq: Some(2),
                session_ref: None,
                session_start_source: None,
            },
            2 => AppEvent::HookAuthorityCleared {
                pane_id: pane,
                source: Some("kitsune:djinn".into()),
                seq: Some(2),
            },
            _ => AppEvent::HookAgentReleased {
                pane_id: pane,
                source: "kitsune:djinn".into(),
                agent_label: "djinn".into(),
                known_agent: Some(Agent::Djinn),
                seq: Some(2),
                session_ref: None,
            },
        };
        app.handle_internal_event(event);
        assert!(app.state.terminals[&terminal]
            .foreground_agent_evidence()
            .is_none());
        assert!(!app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
        assert!(
            app.event_hub.events_after(0).len() > event_count,
            "epoch invalidation must use the status pipeline"
        );
        app.state.assert_invariants_for_test();
    }
}

#[tokio::test]
async fn detector_update_pane_rebind_rejects_previous_terminal_epoch() {
    let (mut app, pane, terminal) = fixture();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    let other_pane = app.state.workspaces[0]
        .tabs
        .iter()
        .flat_map(|tab| tab.panes.keys())
        .copied()
        .find(|id| *id != pane)
        .unwrap();
    let other_terminal = app
        .find_pane(other_pane)
        .unwrap()
        .1
        .attached_terminal_id
        .clone();
    for tab in &mut app.state.workspaces[0].tabs {
        if let Some(pane) = tab.panes.get_mut(&pane) {
            pane.attached_terminal_id = other_terminal.clone();
        }
        if let Some(pane) = tab.panes.get_mut(&other_pane) {
            pane.attached_terminal_id = terminal.clone();
        }
    }
    app.handle_internal_event(confirm(pane, epoch));
    assert!(app.state.terminals[&other_terminal]
        .foreground_agent_evidence()
        .is_none());
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn legacy_screen_update_cannot_refresh_positive_probe() {
    let (mut app, pane, terminal) = fixture();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    app.handle_internal_event(confirm(pane, epoch));
    let evidence = app.state.terminals[&terminal].foreground_agent_evidence();
    assert!(evidence.is_some());
    app.handle_internal_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Djinn),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: true,
        process_exited: false,
        observed_at: Instant::now(),
    });
    assert_eq!(
        app.state.terminals[&terminal].foreground_agent_evidence(),
        evidence
    );
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn detector_update_release_protocol_keeps_process_identity_until_exit() {
    let (mut app, pane, terminal) = fixture();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    app.handle_internal_event(AppEvent::DetectorUpdate {
        pane_id: pane,
        epoch,
        foreground: None,
        fallback: Some(FallbackObservation {
            agent: Some(Agent::Djinn),
            state: AgentState::Idle,
            visible_blocker: false,
            visible_working: false,
        }),
        observed_at: Instant::now(),
    });
    app.handle_internal_event(confirm(pane, epoch));
    app.handle_internal_event(report(pane, 1));
    assert!(app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    let reset_notify = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .agent_detection_reset_notify_for_test();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), reset_notify.notified())
            .await
            .is_err(),
        "authority activation must wake detection without resetting evidence"
    );
    let before_release = app.event_hub.events_after(0).len();
    app.handle_internal_event(AppEvent::HookAgentReleased {
        pane_id: pane,
        source: "kitsune:djinn".into(),
        agent_label: "djinn".into(),
        known_agent: Some(Agent::Djinn),
        seq: Some(2),
        session_ref: None,
    });
    assert!(app.state.terminals[&terminal]
        .foreground_agent_evidence()
        .is_none());
    assert_eq!(
        app.state.terminals[&terminal].detected_agent(),
        Some(Agent::Djinn)
    );
    assert!(!app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    tokio::time::timeout(Duration::from_millis(50), reset_notify.notified())
        .await
        .expect("accepted process-owned release must start runtime suppression");
    assert!(!app
        .event_hub
        .events_after(0)
        .iter()
        .skip(before_release)
        .any(|(_, event)| matches!(
            event.data,
            crate::api::schema::EventData::PaneAgentDetected { released: true, .. }
        )));
    let before_exit = app.event_hub.events_after(0).len();
    app.handle_internal_event(AppEvent::DetectorUpdate {
        pane_id: pane,
        epoch,
        foreground: Some(ForegroundObservation::Exited {
            agent: Agent::Djinn,
        }),
        fallback: None,
        observed_at: Instant::now(),
    });
    assert!(app.state.terminals[&terminal]
        .foreground_agent_evidence()
        .is_none());
    assert!(!app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    let events = app.event_hub.events_after(0);
    assert_eq!(events.iter().skip(before_exit).filter(|(_, event)| matches!(
        &event.data,
        crate::api::schema::EventData::PaneAgentDetected {
            agent: Some(agent), released: true, final_status: Some(crate::api::schema::AgentStatus::Idle), ..
        } if agent == "djinn"
    )).count(), 1);
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn accepted_idle_release_without_visible_update_starts_runtime_suppression() {
    let (mut app, pane, terminal) = fixture();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    app.handle_internal_event(AppEvent::DetectorUpdate {
        pane_id: pane,
        epoch,
        foreground: None,
        fallback: Some(FallbackObservation {
            agent: Some(Agent::Djinn),
            state: AgentState::Idle,
            visible_blocker: false,
            visible_working: false,
        }),
        observed_at: Instant::now(),
    });
    app.handle_internal_event(confirm(pane, epoch));
    let mut idle_report = report(pane, 1);
    if let AppEvent::HookStateReported { state, .. } = &mut idle_report {
        *state = AgentState::Idle;
    }
    app.handle_internal_event(idle_report);
    assert!(app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    assert_eq!(app.state.terminals[&terminal].state, AgentState::Idle);
    let reset_notify = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .agent_detection_reset_notify_for_test();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), reset_notify.notified())
            .await
            .is_err()
    );
    app.state.session_dirty = false;
    let before = app.detector_presentation_snapshot(pane);
    let event_count = app.event_hub.events_after(0).len();
    app.handle_internal_event(AppEvent::HookAgentReleased {
        pane_id: pane,
        source: "kitsune:djinn".into(),
        agent_label: "djinn".into(),
        known_agent: Some(Agent::Djinn),
        seq: Some(2),
        session_ref: None,
    });
    assert_eq!(app.detector_presentation_snapshot(pane), before);
    assert_eq!(
        app.event_hub.events_after(0).len(),
        event_count,
        "acceptance must not fabricate a pane update"
    );
    assert!(app.state.session_dirty);
    assert!(app.state.terminals[&terminal]
        .foreground_agent_evidence()
        .is_none());
    assert_eq!(
        app.state.terminals[&terminal].detected_agent(),
        Some(Agent::Djinn)
    );
    assert!(!app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    tokio::time::timeout(Duration::from_millis(50), reset_notify.notified())
        .await
        .expect("accepted invisible release must start suppression");
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn unchanged_idle_projection_reconfirmation_does_not_create_toast_or_unseen() {
    let (mut app, pane, terminal) = fixture();
    app.state.outer_terminal_focus = Some(false);
    app.state.toast_config.delivery = crate::config::ToastDelivery::Kitsune;
    app.state.toast_config.delay_seconds = 0;
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    app.handle_internal_event(AppEvent::DetectorUpdate {
        pane_id: pane,
        epoch,
        foreground: None,
        fallback: Some(FallbackObservation {
            agent: Some(Agent::Djinn),
            state: AgentState::Idle,
            visible_blocker: false,
            visible_working: false,
        }),
        observed_at: Instant::now(),
    });
    app.handle_internal_event(confirm(pane, epoch));
    let mut idle_report = report(pane, 1);
    if let AppEvent::HookStateReported { state, .. } = &mut idle_report {
        *state = AgentState::Idle;
    }
    app.handle_internal_event(idle_report);
    assert!(app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    assert!(app.state.toast.is_none());
    app.handle_internal_event(AppEvent::DetectorUpdate {
        pane_id: pane,
        epoch,
        foreground: Some(ForegroundObservation::Reset),
        fallback: None,
        observed_at: Instant::now(),
    });
    assert_eq!(app.state.terminals[&terminal].state, AgentState::Unknown);
    app.handle_internal_event(confirm(pane, epoch));
    assert_eq!(app.state.terminals[&terminal].state, AgentState::Idle);
    assert!(app.find_pane(pane).unwrap().1.seen);
    assert!(app.state.toast.is_none());
    assert!(app.state.pending_agent_notifications.is_empty());
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn rejected_release_with_epoch_sync_change_does_not_start_runtime_suppression() {
    let (mut app, pane, terminal) = fixture();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    app.handle_internal_event(AppEvent::DetectorUpdate {
        pane_id: pane,
        epoch,
        foreground: None,
        fallback: Some(FallbackObservation {
            agent: Some(Agent::Djinn),
            state: AgentState::Idle,
            visible_blocker: false,
            visible_working: false,
        }),
        observed_at: Instant::now(),
    });
    app.handle_internal_event(confirm(pane, epoch));
    app.handle_internal_event(report(pane, 1));
    assert_eq!(app.state.terminals[&terminal].state, AgentState::Working);
    assert!(app.state.terminals[&terminal].full_lifecycle_hook_authority_active());
    let replacement = TerminalRuntime::test_with_screen_bytes(80, 24, b"");
    let reset_notify = replacement.agent_detection_reset_notify_for_test();
    app.terminal_runtimes.insert(terminal.clone(), replacement);
    let event_count = app.event_hub.events_after(0).len();
    app.handle_internal_event(AppEvent::HookAgentReleased {
        pane_id: pane,
        source: "custom:unrelated".into(),
        agent_label: "djinn".into(),
        known_agent: Some(Agent::Djinn),
        seq: Some(2),
        session_ref: None,
    });
    assert_eq!(app.state.terminals[&terminal].state, AgentState::Unknown);
    assert_eq!(
        app.state.terminals[&terminal]
            .hook_authority
            .as_ref()
            .unwrap()
            .source,
        "kitsune:djinn"
    );
    assert!(
        app.event_hub.events_after(0).len() > event_count,
        "epoch invalidation must still publish its legitimate state change"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(10), reset_notify.notified())
            .await
            .is_err(),
        "a rejected release must not start replacement-runtime suppression"
    );
    app.state.assert_invariants_for_test();
}

#[tokio::test]
async fn pane_died_clears_foreground_before_pane_removal() {
    let (mut app, pane, terminal) = fixture();
    let epoch = app
        .terminal_runtimes
        .get(&terminal)
        .unwrap()
        .detector_epoch();
    app.handle_internal_event(confirm(pane, epoch));
    app.handle_internal_event(report(pane, 1));
    app.handle_internal_event(AppEvent::PaneDied { pane_id: pane });
    assert!(app.find_pane(pane).is_none());
    assert!(app.terminal_runtimes.get(&terminal).is_none());
    assert!(app
        .state
        .terminals
        .get(&terminal)
        .is_none_or(|terminal| terminal.foreground_agent_evidence().is_none()));
    app.state.assert_invariants_for_test();
}
