use std::time::{Duration, Instant};

use super::test_support::{legacy_control_fixture, legacy_control_snapshot};

#[tokio::test]
async fn held_resume_intent_is_data_in_both_ownership_modes() {
    for external in [false, true] {
        let (mut app, _, terminal_id) = legacy_control_fixture();
        app.state.external_agent_management = external;
        let (runtime, mut input) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        app.terminal_runtimes.insert(terminal_id.clone(), runtime);
        let intent = app.state.terminals[&terminal_id]
            .pending_agent_resume_plan
            .clone();
        let session = app.state.terminals[&terminal_id]
            .persisted_agent_session
            .clone();
        app.state.session_dirty = false;
        app.session_save_deadline = None;
        for geometry_dirty in [false, true] {
            app.handle_scheduled_tasks(Instant::now() + Duration::from_secs(3600), geometry_dirty);
            assert_eq!(
                app.state.terminals[&terminal_id].pending_agent_resume_plan,
                intent
            );
            assert_eq!(
                app.state.terminals[&terminal_id].persisted_agent_session,
                session
            );
            assert!(input.try_recv().is_err());
            assert!(app.session_save_deadline.is_none());
            assert!(!app.state.session_dirty);
        }
        let before = legacy_control_snapshot(&app);
        app.state
            .terminals
            .get_mut(&terminal_id)
            .unwrap()
            .clear_agent_runtime_identity_after_respawn();
        assert_eq!(
            app.state.terminals[&terminal_id].pending_agent_resume_plan,
            intent
        );
        assert_eq!(
            app.state.terminals[&terminal_id].persisted_agent_session,
            session
        );
        assert!(!before.is_empty());
    }
}

#[tokio::test]
async fn actual_child_exit_cleans_up_without_legacy_completion_or_notifications() {
    for external in [false, true] {
        let (mut app, _, terminal_id) = legacy_control_fixture();
        app.state.external_agent_management = external;
        app.state.toast = None;
        app.toast_deadline = None;
        let pane_id = app.state.workspaces[0].tabs.last().unwrap().root_pane;
        assert_eq!(
            app.state.terminal_id_for_pane(0, pane_id).as_ref(),
            Some(&terminal_id)
        );
        let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
        terminal.respawn_shell_on_exit = false;
        terminal.state = crate::detect::AgentState::Working;
        let (runtime, mut input) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
        app.terminal_runtimes.insert(terminal_id.clone(), runtime);
        app.handle_internal_event(crate::events::AppEvent::PaneDied { pane_id });
        assert!(app.find_pane(pane_id).is_none());
        assert!(!app.state.terminals.contains_key(&terminal_id));
        assert!(app.terminal_runtimes.get(&terminal_id).is_none());
        assert!(input.try_recv().is_err());
        assert!(app.state.toast.is_none());
        assert!(app.toast_deadline.is_none());
        assert!(app
            .event_hub
            .events_after(0)
            .iter()
            .all(|(_, event)| !matches!(
                event.event,
                crate::api::schema::EventKind::PaneAgentDetected
                    | crate::api::schema::EventKind::PaneAgentStatusChanged
            )));
        app.state.assert_invariants_for_test();
    }
}
