use super::*;
use crate::terminal::{TerminalId, TerminalRuntime, TerminalState, TerminalStateMutation};

fn apply_queued_detector(terminal: &mut TerminalState, event: AppEvent) -> TerminalStateMutation {
    match event {
        AppEvent::DetectorUpdate {
            epoch,
            foreground,
            fallback,
            observed_at,
            ..
        } => terminal.apply_detector_update(epoch, foreground, fallback, observed_at),
        event => panic!("unexpected event: {event:?}"),
    }
}

fn positive(producer: &mut DetectorProducer) -> Option<ForegroundObservation> {
    producer.observe_probe(
        Some(Agent::Pi),
        Some(123),
        ForegroundShellAgentAction::ObserveProbe,
        Some(Agent::Pi),
        Some(Agent::Pi),
    )
}

#[test]
fn every_actual_positive_including_unchanged_restored_presence_is_evidence() {
    let mut producer = DetectorProducer::default();
    let confirmed = ForegroundObservation::Confirmed {
        agent: Agent::Pi,
        process_group_id: Some(123),
    };
    assert_eq!(positive(&mut producer), Some(confirmed));
    assert_eq!(positive(&mut producer), Some(confirmed));
    assert_eq!(
        producer.observe(ForegroundObservation::Inconclusive),
        Some(ForegroundObservation::Inconclusive)
    );
    assert_eq!(positive(&mut producer), Some(confirmed));
    producer.observe(ForegroundObservation::Reset);
    assert_eq!(positive(&mut producer), Some(confirmed));
    producer.observe(ForegroundObservation::Exited { agent: Agent::Pi });
    assert_eq!(positive(&mut producer), Some(confirmed));
}

#[test]
fn raw_misses_retain_presence_but_only_threshold_clears_evidence() {
    let mut producer = DetectorProducer::default();
    let mut presence = AgentDetectionPresence::from_agent(Some(Agent::Pi));
    positive(&mut producer);
    for miss in 1..=AGENT_MISS_CONFIRMATION_ATTEMPTS {
        let previous = presence.current_agent();
        presence.observe_process_probe(None);
        let finding = producer.observe_probe(
            None,
            Some(123),
            ForegroundShellAgentAction::ObserveProbe,
            previous,
            presence.current_agent(),
        );
        if miss == 1 {
            assert_eq!(finding, Some(ForegroundObservation::Inconclusive));
        } else if miss == AGENT_MISS_CONFIRMATION_ATTEMPTS {
            assert_eq!(finding, Some(ForegroundObservation::Cleared));
        } else {
            assert_eq!(finding, None);
            assert_eq!(presence.current_agent(), Some(Agent::Pi));
        }
    }
}

#[test]
fn release_filtered_probe_cannot_confirm_and_exit_cleanup_emits_clear_not_exit() {
    let mut producer = DetectorProducer::default();
    positive(&mut producer);
    assert_eq!(
        producer.observe_probe(
            None,
            Some(123),
            ForegroundShellAgentAction::ObserveProbe,
            None,
            None
        ),
        Some(ForegroundObservation::Inconclusive)
    );
    assert_eq!(
        producer.observe_probe(
            None,
            Some(123),
            ForegroundShellAgentAction::ReportProcessExit,
            Some(Agent::Pi),
            Some(Agent::Pi)
        ),
        None
    );
    producer.observe(ForegroundObservation::Exited { agent: Agent::Pi });
    assert_eq!(
        producer.observe_probe(
            None,
            Some(123),
            ForegroundShellAgentAction::ClearAgent,
            Some(Agent::Pi),
            None
        ),
        Some(ForegroundObservation::Cleared)
    );
    assert_eq!(
        producer.observe_probe(
            None,
            Some(123),
            ForegroundShellAgentAction::ClearAgent,
            Some(Agent::Pi),
            None,
        ),
        Some(ForegroundObservation::Cleared)
    );
    assert!(positive(&mut producer).is_some());
}

#[tokio::test]
async fn both_loop_producer_emits_foreground_before_hook_or_screen_skip() {
    let (tx, mut rx) = mpsc::channel(4);
    let epoch = DetectorEpoch::new();
    let pane_id = PaneId::from_raw(42);
    let now = std::time::Instant::now();
    let mut producer = DetectorProducer::default();
    producer
        .publish_probe(
            &tx,
            pane_id,
            epoch,
            Some(Agent::Pi),
            Some(123),
            ForegroundShellAgentAction::ObserveProbe,
            Some(Agent::Pi),
            Some(Agent::Pi),
            now,
        )
        .await;
    assert!(
        matches!(rx.recv().await, Some(AppEvent::DetectorUpdate { epoch: delivered, foreground: Some(ForegroundObservation::Confirmed { agent: Agent::Pi, .. }), fallback: None, observed_at, .. }) if delivered == epoch && observed_at == now)
    );
    producer.reset(&tx, pane_id, epoch).await;
    assert!(matches!(
        rx.recv().await,
        Some(AppEvent::DetectorUpdate {
            foreground: Some(ForegroundObservation::Reset),
            fallback: None,
            ..
        })
    ));
    assert!(positive(&mut producer).is_some());
}

#[tokio::test]
async fn screen_publication_never_confirms_and_exit_is_combined_with_idle() {
    let (tx, mut rx) = mpsc::channel(4);
    let epoch = DetectorEpoch::new();
    let pane_id = PaneId::from_raw(42);
    for exited in [false, true] {
        publish_state_changed_event(
            tx.clone(),
            pane_id,
            epoch,
            Some(Agent::Pi),
            AgentState::Working,
            false,
            false,
            exited,
            std::time::Instant::now(),
        )
        .await;
        match rx.recv().await.unwrap() {
            AppEvent::DetectorUpdate {
                foreground,
                fallback: Some(fallback),
                ..
            } => {
                assert_eq!(
                    foreground,
                    exited.then_some(ForegroundObservation::Exited { agent: Agent::Pi })
                );
                assert_eq!(
                    fallback.state,
                    if exited {
                        AgentState::Idle
                    } else {
                        AgentState::Working
                    }
                );
            }
            event => panic!("unexpected event: {event:?}"),
        }
    }
}

#[tokio::test]
async fn exit_publish_updates_cache_and_reports_only_combined_idle() {
    let (tx, mut rx) = mpsc::channel(4);
    let mut producer = DetectorProducer::default();
    positive(&mut producer);
    let mut state = AgentState::Working;
    let mut idle = false;
    let mut blocker = false;
    let mut working = true;
    let mut refreshed = None;
    let mut exited = false;
    apply_agent_detection_publish_update(
        tx,
        PaneId::from_raw(42),
        DetectorEpoch::new(),
        &mut producer,
        Some(Agent::Pi),
        AgentDetectionPublishUpdate {
            state: AgentState::Idle,
            visible_idle: true,
            visible_blocker: false,
            visible_working: false,
            process_exited: true,
        },
        std::time::Instant::now(),
        &mut state,
        &mut idle,
        &mut blocker,
        &mut working,
        &mut refreshed,
        &mut exited,
    )
    .await;
    assert!(exited);
    assert_eq!(state, AgentState::Idle);
    assert!(matches!(
        rx.recv().await,
        Some(AppEvent::DetectorUpdate {
            foreground: Some(ForegroundObservation::Exited { agent: Agent::Pi }),
            fallback: Some(FallbackObservation {
                state: AgentState::Idle,
                ..
            }),
            ..
        })
    ));
    assert!(rx.try_recv().is_err());
    assert!(positive(&mut producer).is_some());
}

#[tokio::test]
async fn foreground_queue_backpressure_preserves_original_epoch() {
    let (tx, mut rx) = mpsc::channel(1);
    tx.send(AppEvent::ClipboardWrite { content: vec![] })
        .await
        .unwrap();
    let old_epoch = DetectorEpoch::new();
    let replacement_epoch = DetectorEpoch::new();
    let mut producer = DetectorProducer::default();
    let publish = producer.publish_probe(
        &tx,
        PaneId::from_raw(42),
        old_epoch,
        Some(Agent::Pi),
        Some(123),
        ForegroundShellAgentAction::ObserveProbe,
        Some(Agent::Pi),
        Some(Agent::Pi),
        std::time::Instant::now(),
    );
    tokio::pin!(publish);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), &mut publish)
            .await
            .is_err()
    );
    rx.recv().await.unwrap();
    publish.await;
    assert!(matches!(rx.recv().await, Some(AppEvent::DetectorUpdate {
        epoch, foreground: Some(ForegroundObservation::Confirmed { .. }), fallback: None, ..
    }) if epoch == old_epoch && epoch != replacement_epoch));
}

#[tokio::test]
async fn same_positive_after_rejected_queued_probe_promotes_pending_full_hook() {
    let (runtime, _) = TerminalRuntime::test_with_channel(20, 4);
    let mut terminal = TerminalState::new(TerminalId::alloc(), "/tmp".into());
    terminal.seed_restored_agent_identity(Agent::Djinn);
    let epoch = runtime.detector_epoch();
    terminal.synchronize_detector_epoch(Some(epoch), epoch.started_at());
    let (tx, mut rx) = mpsc::channel(4);
    let mut producer = DetectorProducer::default();
    let t0 = std::time::Instant::now();
    producer
        .publish_probe(
            &tx,
            PaneId::from_raw(42),
            epoch,
            Some(Agent::Djinn),
            Some(123),
            ForegroundShellAgentAction::ObserveProbe,
            Some(Agent::Djinn),
            Some(Agent::Djinn),
            t0,
        )
        .await;

    let session = crate::agent_resume::AgentSessionRef::id("new-session");
    // A recognized start is buffered until a post-barrier probe confirms it.
    terminal.set_agent_session_ref_for_session_start(
        "kitsune:djinn".into(),
        "djinn".into(),
        session.clone(),
        Some(1),
        Some("new".into()),
    );
    terminal.set_hook_authority_with_session_ref(
        "kitsune:djinn".into(),
        "djinn".into(),
        AgentState::Working,
        None,
        session,
        Some(2),
    );
    apply_queued_detector(&mut terminal, rx.recv().await.unwrap());
    assert!(terminal.foreground_agent_evidence().is_none());
    assert!(!terminal.full_lifecycle_hook_authority_active());
    assert_eq!(terminal.state, AgentState::Idle);

    let t2 = std::time::Instant::now();
    producer
        .publish_probe(
            &tx,
            PaneId::from_raw(42),
            epoch,
            Some(Agent::Djinn),
            Some(123),
            ForegroundShellAgentAction::ObserveProbe,
            Some(Agent::Djinn),
            Some(Agent::Djinn),
            t2,
        )
        .await;
    let event = rx
        .try_recv()
        .expect("same-kind actual probe must be emitted again");
    let mutation = apply_queued_detector(&mut terminal, event);
    assert!(mutation.effective_state_change.is_some());
    assert_eq!(
        terminal.foreground_agent_evidence(),
        Some((Agent::Djinn, t2))
    );
    assert!(terminal.full_lifecycle_hook_authority_active());
    assert_eq!(terminal.state, AgentState::Working);

    let wake = runtime.agent_detection_wake_notify_for_test();
    let reset = runtime.agent_detection_reset_notify_for_test();
    runtime.set_full_lifecycle_authority_active(terminal.full_lifecycle_hook_authority_active());
    tokio::time::timeout(std::time::Duration::from_millis(50), wake.notified())
        .await
        .expect("activation should wake the detector");
    producer
        .publish_probe(
            &tx,
            PaneId::from_raw(42),
            epoch,
            Some(Agent::Djinn),
            Some(123),
            ForegroundShellAgentAction::ObserveProbe,
            Some(Agent::Djinn),
            Some(Agent::Djinn),
            std::time::Instant::now(),
        )
        .await;
    apply_queued_detector(&mut terminal, rx.recv().await.unwrap());
    runtime.set_full_lifecycle_authority_active(terminal.full_lifecycle_hook_authority_active());
    assert!(terminal.full_lifecycle_hook_authority_active());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), reset.notified())
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), wake.notified())
            .await
            .is_err()
    );
    assert!(rx.try_recv().is_err());
    let last_confirmation = terminal.foreground_agent_evidence();

    let inconclusive = producer.observe_probe(
        None,
        Some(123),
        ForegroundShellAgentAction::ObserveProbe,
        Some(Agent::Djinn),
        Some(Agent::Djinn),
    );
    terminal.apply_detector_update(epoch, inconclusive, None, std::time::Instant::now());
    assert_eq!(terminal.foreground_agent_evidence(), last_confirmation);
}

#[tokio::test]
async fn explicit_reset_retains_probe_context_to_retire_old_session_on_shell_exit() {
    let mut terminal = TerminalState::new(TerminalId::alloc(), "/tmp".into());
    let epoch = DetectorEpoch::new();
    terminal.synchronize_detector_epoch(Some(epoch), epoch.started_at());
    let session = crate::agent_resume::AgentSessionRef::id("old-session");
    terminal.set_agent_session_ref_for_session_start(
        "kitsune:djinn".into(),
        "djinn".into(),
        session.clone(),
        Some(1),
        Some("new".into()),
    );
    terminal.set_hook_authority_with_session_ref(
        "kitsune:djinn".into(),
        "djinn".into(),
        AgentState::Working,
        None,
        session.clone(),
        Some(2),
    );
    let (tx, mut rx) = mpsc::channel(4);
    let pane_id = PaneId::from_raw(42);
    let mut producer = DetectorProducer::default();
    let mut presence = AgentDetectionPresence::from_agent(Some(Agent::Djinn));
    producer
        .publish_probe(
            &tx,
            pane_id,
            epoch,
            Some(Agent::Djinn),
            Some(123),
            ForegroundShellAgentAction::ObserveProbe,
            Some(Agent::Djinn),
            Some(Agent::Djinn),
            std::time::Instant::now(),
        )
        .await;
    apply_queued_detector(&mut terminal, rx.recv().await.unwrap());
    assert!(terminal.full_lifecycle_hook_authority_active());
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|s| &s.session_ref),
        session.as_ref()
    );

    producer
        .reset_presence(&tx, pane_id, epoch, &mut presence, None)
        .await;
    apply_queued_detector(&mut terminal, rx.recv().await.unwrap());
    assert!(terminal.foreground_agent_evidence().is_none());
    assert_eq!(presence.current_agent(), Some(Agent::Djinn));
    let mut pending_clear = false;
    let mut exit_reported = false;
    let previous = presence.current_agent();
    let action = foreground_shell_agent_action(previous, None, true, exit_reported);
    assert_eq!(action, ForegroundShellAgentAction::ReportProcessExit);
    apply_foreground_shell_agent_action(
        &mut presence,
        action,
        previous,
        None,
        &mut pending_clear,
        &mut exit_reported,
    );
    producer
        .publish_probe(
            &tx,
            pane_id,
            epoch,
            None,
            Some(7),
            action,
            previous,
            presence.current_agent(),
            std::time::Instant::now(),
        )
        .await;
    assert!(
        rx.try_recv().is_err(),
        "exit must await combined Idle publication"
    );
    let mut state = AgentState::Working;
    let mut idle = false;
    let mut blocker = false;
    let mut working = true;
    let mut refreshed = None;
    apply_agent_detection_publish_update(
        tx.clone(),
        pane_id,
        epoch,
        &mut producer,
        presence.current_agent(),
        AgentDetectionPublishUpdate {
            state: AgentState::Idle,
            visible_idle: true,
            visible_blocker: false,
            visible_working: false,
            process_exited: pending_clear && !exit_reported,
        },
        std::time::Instant::now(),
        &mut state,
        &mut idle,
        &mut blocker,
        &mut working,
        &mut refreshed,
        &mut exit_reported,
    )
    .await;
    let exit = rx.recv().await.unwrap();
    assert!(matches!(
        exit,
        AppEvent::DetectorUpdate {
            foreground: Some(ForegroundObservation::Exited {
                agent: Agent::Djinn
            }),
            fallback: Some(FallbackObservation {
                state: AgentState::Idle,
                ..
            }),
            ..
        }
    ));
    apply_queued_detector(&mut terminal, exit);
    assert!(terminal.persisted_agent_session.is_none());

    let previous = presence.current_agent();
    let action = foreground_shell_agent_action(previous, None, true, exit_reported);
    assert_eq!(action, ForegroundShellAgentAction::ClearAgent);
    apply_foreground_shell_agent_action(
        &mut presence,
        action,
        previous,
        None,
        &mut pending_clear,
        &mut exit_reported,
    );
    producer
        .publish_probe(
            &tx,
            pane_id,
            epoch,
            None,
            Some(7),
            action,
            previous,
            presence.current_agent(),
            std::time::Instant::now(),
        )
        .await;
    let cleared = apply_queued_detector(&mut terminal, rx.recv().await.unwrap());
    assert!(
        !cleared.agent_released,
        "cleanup must not repeat completion"
    );
    assert!(terminal.foreground_agent_evidence().is_none());

    producer
        .publish_probe(
            &tx,
            pane_id,
            epoch,
            Some(Agent::Djinn),
            Some(123),
            ForegroundShellAgentAction::ObserveProbe,
            None,
            Some(Agent::Djinn),
            std::time::Instant::now(),
        )
        .await;
    apply_queued_detector(&mut terminal, rx.recv().await.unwrap());
    terminal.set_hook_authority_with_session_ref(
        "kitsune:djinn".into(),
        "djinn".into(),
        AgentState::Working,
        None,
        session,
        Some(3),
    );
    assert!(!terminal.full_lifecycle_hook_authority_active());
    assert!(terminal.persisted_agent_session.is_none());
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn reset_presence_restarts_six_miss_policy_without_forgetting_agent() {
    let (tx, mut rx) = mpsc::channel(4);
    let mut producer = DetectorProducer::default();
    let mut presence = AgentDetectionPresence::from_agent(Some(Agent::Pi));
    for _ in 1..AGENT_MISS_CONFIRMATION_ATTEMPTS {
        presence.observe_process_probe(None);
    }
    producer
        .reset_presence(
            &tx,
            PaneId::from_raw(42),
            DetectorEpoch::new(),
            &mut presence,
            None,
        )
        .await;
    assert!(matches!(
        rx.recv().await,
        Some(AppEvent::DetectorUpdate {
            foreground: Some(ForegroundObservation::Reset),
            fallback: None,
            ..
        })
    ));
    for miss in 1..=AGENT_MISS_CONFIRMATION_ATTEMPTS {
        let previous = presence.current_agent();
        let mut pending_clear = false;
        let mut exit_reported = false;
        let action = foreground_shell_agent_action(previous, None, false, exit_reported);
        apply_foreground_shell_agent_action(
            &mut presence,
            action,
            previous,
            None,
            &mut pending_clear,
            &mut exit_reported,
        );
        let finding =
            producer.observe_probe(None, Some(123), action, previous, presence.current_agent());
        if miss == AGENT_MISS_CONFIRMATION_ATTEMPTS {
            assert_eq!(finding, Some(ForegroundObservation::Cleared));
            assert_eq!(presence.current_agent(), None);
        } else {
            assert_eq!(presence.current_agent(), Some(Agent::Pi));
            assert_eq!(
                finding,
                (miss == 1).then_some(ForegroundObservation::Inconclusive)
            );
        }
    }
}

#[tokio::test]
async fn release_reset_drops_suppressed_presence_without_duplicate_exit() {
    let (runtime, _) = TerminalRuntime::test_with_channel(20, 4);
    let epoch = runtime.detector_epoch();
    let reset = runtime.agent_detection_reset_notify_for_test();
    runtime.begin_graceful_release(Agent::Pi);
    tokio::time::timeout(std::time::Duration::from_millis(50), reset.notified())
        .await
        .expect("explicit graceful release must still reset");
    let (tx, mut rx) = mpsc::channel(4);
    let mut producer = DetectorProducer::default();
    let mut presence = AgentDetectionPresence::from_agent(Some(Agent::Pi));
    producer
        .reset_presence(
            &tx,
            PaneId::from_raw(42),
            epoch,
            &mut presence,
            Some(Agent::Pi),
        )
        .await;
    assert!(matches!(
        rx.recv().await,
        Some(AppEvent::DetectorUpdate {
            foreground: Some(ForegroundObservation::Reset),
            fallback: None,
            ..
        })
    ));
    assert_eq!(presence.current_agent(), None);
    let action = foreground_shell_agent_action(presence.current_agent(), None, true, false);
    assert_eq!(action, ForegroundShellAgentAction::ObserveProbe);
    assert_eq!(
        producer.observe_probe(None, Some(123), action, None, None),
        Some(ForegroundObservation::Inconclusive)
    );
}

#[tokio::test]
async fn clear_agent_after_rejected_exit_removes_proof_without_completion() {
    let mut terminal = TerminalState::new(TerminalId::alloc(), "/tmp".into());
    let epoch = DetectorEpoch::new();
    terminal.synchronize_detector_epoch(Some(epoch), epoch.started_at());
    let (tx, mut rx) = mpsc::channel(4);
    let mut producer = DetectorProducer::default();
    let confirmed = positive(&mut producer);
    terminal.apply_detector_update(
        epoch,
        confirmed,
        Some(FallbackObservation {
            agent: Some(Agent::Pi),
            state: AgentState::Working,
            visible_blocker: false,
            visible_working: true,
        }),
        std::time::Instant::now(),
    );
    assert!(terminal.foreground_agent_evidence().is_some());

    // Mirror the detector's deferred exit publication, including its cache update.
    producer.observe(ForegroundObservation::Exited { agent: Agent::Pi });
    publish_state_changed_event(
        tx.clone(),
        PaneId::from_raw(42),
        epoch,
        Some(Agent::Pi),
        AgentState::Idle,
        false,
        false,
        true,
        std::time::Instant::now(),
    )
    .await;
    terminal
        .release_agent_with_mutation("custom:pi", "pi", Some(1), None)
        .expect("release should establish the newer detector barrier");
    let before_exit_delivery = terminal.state;
    let rejected = apply_queued_detector(&mut terminal, rx.recv().await.unwrap());
    assert!(!rejected.agent_released);
    assert!(rejected.effective_state_change.is_none());
    assert_eq!(terminal.state, before_exit_delivery);

    let mut presence = AgentDetectionPresence::from_agent(Some(Agent::Pi));
    let previous = presence.current_agent();
    let action = foreground_shell_agent_action(previous, None, true, true);
    assert_eq!(action, ForegroundShellAgentAction::ClearAgent);
    let mut pending_clear = true;
    let mut exit_reported = true;
    apply_foreground_shell_agent_action(
        &mut presence,
        action,
        previous,
        None,
        &mut pending_clear,
        &mut exit_reported,
    );
    producer
        .publish_probe(
            &tx,
            PaneId::from_raw(42),
            epoch,
            None,
            Some(123),
            action,
            previous,
            presence.current_agent(),
            std::time::Instant::now(),
        )
        .await;
    let event = rx
        .try_recv()
        .expect("cleanup must clear even if the queued exit was rejected");
    assert!(matches!(
        event,
        AppEvent::DetectorUpdate {
            foreground: Some(ForegroundObservation::Cleared),
            fallback: None,
            ..
        }
    ));
    let cleared = apply_queued_detector(&mut terminal, event);
    assert!(terminal.foreground_agent_evidence().is_none());
    assert!(!cleared.agent_released);
    assert_eq!(terminal.state, before_exit_delivery);
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn runtime_epochs_differ_even_with_reused_pane_id_and_reset_keeps_epoch() {
    let (first, _) = PaneRuntime::test_with_channel(20, 4);
    let (second, _) = PaneRuntime::test_with_channel(20, 4);
    assert_eq!(first.pane_id, second.pane_id);
    assert_ne!(first.detector_epoch(), second.detector_epoch());
    let epoch = first.detector_epoch();
    first.reset_agent_detection();
    assert_eq!(first.detector_epoch(), epoch);
}
