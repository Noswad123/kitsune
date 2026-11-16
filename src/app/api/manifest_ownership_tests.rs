use super::*;
use crate::api::schema::{EmptyParams, Method, Request};
use crate::detect::manifest_update::{ManifestUpdateCommit, ManifestUpdateStatus, ManifestVersion};

fn fixture() -> App {
    let mut state = crate::app::AppState::test_with_adversarial_identity_state();
    state.external_agent_management = true;
    state.agent_manifest_update_status.last_check_unix = Some(123);
    state.agent_manifest_update_status.last_result = Some("owned-status".into());
    state
        .agent_manifest_summaries
        .push(crate::detect::manifest::AgentManifestSummary {
            agent: crate::detect::Agent::Codex,
            active_source: crate::detect::manifest::ManifestSource::Bundled,
            active_version: Some("held-version".into()),
            cached_remote_version: Some("held-cache".into()),
            local_override_shadowing_remote: false,
            warning: Some("held-warning".into()),
        });
    for terminal in state.terminals.values_mut() {
        terminal.seed_restored_agent_identity(crate::detect::Agent::Codex);
        terminal.set_manual_label("held-label".into());
        let session = crate::agent_resume::PersistedAgentSession {
            source: "kitsune:codex".into(),
            agent: "codex".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("held-history").unwrap(),
        };
        terminal.set_persisted_agent_session(session.clone());
        terminal.pending_agent_resume_plan =
            crate::agent_resume::plan(&session.source, &session.agent, &session.session_ref);
        terminal.hook_authority = Some(crate::terminal::state::HookAuthority {
            source: "custom:codex".into(),
            agent_label: "codex".into(),
            state: crate::detect::AgentState::Idle,
            message: Some("held-native-status".into()),
            reported_at: Instant::now(),
            session_ref: Some(session.session_ref),
        });
    }
    let mut app = crate::app::test_support::app_from_state(state);
    for (id, terminal) in &mut app.state.terminals {
        let (runtime, _) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        let now = Instant::now();
        let epoch = runtime.detector_epoch();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        terminal.apply_detector_update(
            epoch,
            Some(
                crate::terminal::evidence::ForegroundObservation::Confirmed {
                    agent: crate::detect::Agent::Codex,
                    process_group_id: Some(123),
                },
            ),
            None,
            now,
        );
        app.terminal_runtimes.insert(id.clone(), runtime);
    }
    app.render_dirty.store(false, Ordering::Release);
    app
}

fn snapshot(app: &App) -> String {
    let mut terminals: Vec<_> = app
        .state
        .terminals
        .iter()
        .map(|(id, terminal)| {
            (
                id.to_string(),
                terminal.test_legacy_control_snapshot(),
                terminal.foreground_agent_evidence(),
                terminal.foreground_process_group_id(),
            )
        })
        .collect();
    terminals.sort_by(|left, right| left.0.cmp(&right.0));
    format!(
        "{terminals:?} {:?} {:?} {:?} {:?} {:?} {} {} {}",
        app.state.agent_manifest_summaries,
        app.state.agent_manifest_update_status,
        app.state.toast,
        app.toast_deadline,
        app.session_save_deadline,
        app.state.session_dirty,
        app.render_dirty.load(Ordering::Acquire),
        app.event_hub.events_after(0).len()
    )
}

fn update() -> AppEvent {
    AppEvent::AgentDetectionManifestsUpdated {
        updated: vec![ManifestUpdateCommit {
            agent: crate::detect::Agent::Codex,
            version: ManifestVersion::parse("9999.1").unwrap(),
        }],
        status: ManifestUpdateStatus::default(),
    }
}

#[tokio::test]
async fn external_manifest_api_refuses_before_status_cache_or_reset() {
    let mut app = fixture();
    let before = snapshot(&app);
    let epochs: Vec<_> = app
        .terminal_runtimes
        .values()
        .map(|r| r.detector_epoch())
        .collect();
    for method in [
        Method::ServerAgentManifests(EmptyParams::default()),
        Method::ServerReloadAgentManifests(EmptyParams::default()),
    ] {
        let response: serde_json::Value = serde_json::from_str(&app.handle_api_request(Request {
            id: "manifest-canary".into(),
            method,
        }))
        .unwrap();
        assert_eq!(response["id"], "manifest-canary");
        assert_eq!(response["error"]["code"], "external_agent_management");
        assert_eq!(snapshot(&app), before);
        assert_eq!(
            app.terminal_runtimes
                .values()
                .map(|r| r.detector_epoch())
                .collect::<Vec<_>>(),
            epochs
        );
    }
}

#[tokio::test]
async fn external_manifest_queued_event_and_direct_resets_are_inert() {
    let mut app = fixture();
    let before = snapshot(&app);
    let epochs: Vec<_> = app
        .terminal_runtimes
        .values()
        .map(|r| r.detector_epoch())
        .collect();
    let notifications: Vec<_> = app
        .terminal_runtimes
        .values()
        .map(|r| r.agent_detection_reset_notify_for_test())
        .collect();
    assert!(app.state.handle_app_event(update()).is_empty());
    assert!(!app.handle_internal_event_with_render_impact(update()));
    app.reset_agent_detection_for_agents(&[crate::detect::Agent::Codex]);
    app.reset_all_agent_detection_runtimes();
    assert_eq!(snapshot(&app), before);
    assert_eq!(
        app.terminal_runtimes
            .values()
            .map(|r| r.detector_epoch())
            .collect::<Vec<_>>(),
        epochs
    );
    for notify in notifications {
        assert!(
            tokio::time::timeout(Duration::from_millis(1), notify.notified())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn external_manifest_direct_scheduler_clears_expired_deadline() {
    let mut app = fixture();
    app.no_session = false;
    app.update_manifest_check_enabled = true;
    app.next_agent_manifest_update_check = Some(Instant::now());
    let before = snapshot(&app);
    app.run_agent_manifest_update_check();
    assert!(app.next_agent_manifest_update_check.is_none());
    assert_eq!(snapshot(&app), before);
}

#[test]
fn external_manifest_startup_eligibility_matrix_is_independent_of_build_mode() {
    for no_session in [false, true] {
        for external in [false, true] {
            for debug in [false, true] {
                for enabled in [false, true] {
                    assert_eq!(
                        crate::app::remote_manifest_updates_enabled(no_session, external, debug)
                            && enabled,
                        !no_session && !external && !debug && enabled,
                    );
                    assert_eq!(
                        crate::app::background_update_check_enabled(no_session, enabled, external),
                        !no_session && !external && !cfg!(debug_assertions) && enabled,
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn external_manifest_reload_cannot_reschedule_enabled_checks() {
    let mut app = fixture();
    app.no_session = false;
    let mut config = crate::config::Config::default();
    config.agent_detection.manifest_check = true;
    for previously_enabled in [false, true] {
        app.update_manifest_check_enabled = previously_enabled;
        app.next_agent_manifest_update_check = Some(Instant::now());
        app.apply_live_config(&config, &[], &[], false);
        assert!(app.state.external_agent_management);
        assert!(app.update_manifest_check_enabled);
        assert!(app.next_agent_manifest_update_check.is_none());
    }
}
