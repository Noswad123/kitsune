use super::*;
use crate::agent_resume::{AgentResumePlan, AgentSessionRef, PersistedAgentSession};
use crate::app::{test_support::app_from_state, AppState};
use crate::terminal::TerminalId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

struct OwnedDirectory(PathBuf);

impl OwnedDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "kitsune-resume-guards-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create owned guard-test directory: {error}"),
            }
        }
        panic!("could not reserve a unique guard-test directory");
    }
}

impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    app: App,
    terminal_id: TerminalId,
    session: PersistedAgentSession,
    plan: AgentResumePlan,
    _directory: OwnedDirectory,
}

fn fixture() -> Fixture {
    let directory = OwnedDirectory::new();
    let mut state = AppState::test_with_adversarial_identity_state();
    // An accidentally bypassed guard must not resolve the user's default shell
    // or launch a provider. The private directory stays owned for the test.
    state.default_shell = directory
        .0
        .join("shell-must-not-run")
        .to_str()
        .unwrap()
        .into();
    state.active = Some(0);
    state.ensure_test_terminals();
    state.host_terminal_theme = Default::default();
    let pane_id = state.workspaces[0].tabs[0].root_pane;
    let terminal_id = state.workspaces[0].terminal_id(pane_id).unwrap().clone();
    let session = PersistedAgentSession {
        source: "kitsune:codex".into(),
        agent: "codex".into(),
        session_ref: AgentSessionRef::id("guarded-session").unwrap(),
    };
    let plan = crate::agent_resume::plan(&session.source, &session.agent, &session.session_ref)
        .expect("fixture must have a real supported plan");
    let terminal = state.terminals.get_mut(&terminal_id).unwrap();
    terminal.cwd = directory.0.clone();
    terminal.set_persisted_agent_session(session.clone());
    terminal.seed_restored_agent_identity(crate::detect::Agent::Codex);
    terminal.pending_agent_resume_plan = Some(plan.clone());
    terminal.launch_argv = Some(vec!["saved-launch-marker".into()]);
    terminal.respawn_shell_on_exit = true;
    state.session_dirty = false;
    let mut app = app_from_state(state);
    // Enable scheduling so a mistakenly scheduled save would be observable.
    // No background-save executor is invoked by these guard tests.
    app.no_session = false;
    Fixture {
        app,
        terminal_id,
        session,
        plan,
        _directory: directory,
    }
}

fn assert_intent_preserved(
    app: &App,
    terminal_id: &TerminalId,
    session: &PersistedAgentSession,
    plan: &AgentResumePlan,
    revision: u64,
) {
    let terminal = &app.state.terminals[terminal_id];
    assert_eq!(terminal.persisted_agent_session.as_ref(), Some(session));
    assert_eq!(terminal.pending_agent_resume_plan.as_ref(), Some(plan));
    assert_eq!(
        terminal.launch_argv,
        Some(vec!["saved-launch-marker".into()])
    );
    assert!(terminal.respawn_shell_on_exit);
    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.detected_agent(), Some(crate::detect::Agent::Codex));
    assert_eq!(
        terminal.historical_agent_identity(),
        Some(crate::detect::Agent::Codex)
    );
    assert_eq!(terminal.foreground_agent_evidence(), None);
    assert_eq!(terminal.state, crate::detect::AgentState::Idle);
    assert_eq!(terminal.revision, revision);
    assert_eq!(app.terminal_runtimes.len(), 0);
    assert!(!app.state.session_dirty);
    assert!(app.session_save_deadline.is_none());
    assert!(app.session_save_thread.is_none());
    app.state.assert_invariants_for_test();
}

#[test]
fn external_agent_management_suppresses_even_queued_resume_execution() {
    let Fixture {
        mut app,
        terminal_id,
        session,
        plan,
        ..
    } = fixture();
    app.state.external_agent_management = true;
    let revision = app.state.terminals[&terminal_id].revision;
    app.state.view.terminal_area = Rect::new(0, 0, 80, 24);
    app.pending_agent_resume_deadline = Some(Instant::now());
    app.sync_pending_agent_resume_deadline(Instant::now());
    assert!(app.pending_agent_resume_deadline.is_none());
    assert!(!app.has_pending_agent_resumes());
    assert!(app.pending_agent_resume_candidates().is_empty());
    assert!(!app.start_pending_agent_resumes(true));
    assert!(!app.start_pending_agent_resume_for_terminal(&terminal_id, 24, 80, true));
    assert_intent_preserved(&app, &terminal_id, &session, &plan, revision);
}

#[test]
fn pending_agent_resume_deadline_tracks_geometry_without_extending_wait() {
    let Fixture {
        mut app,
        terminal_id,
        session,
        plan,
        _directory,
    } = fixture();
    let revision = app.state.terminals[&terminal_id].revision;
    let now = Instant::now();
    for area in [Rect::new(0, 0, 0, 30), Rect::new(0, 0, 100, 0)] {
        app.state.view.terminal_area = area;
        app.pending_agent_resume_deadline = Some(now);
        assert!(app.pending_agent_resume_candidates().is_empty());
        app.sync_pending_agent_resume_deadline(now);
        assert!(app.pending_agent_resume_deadline.is_none());
        assert!(!app.pending_agent_resume_due(now));
        assert!(!app.start_pending_agent_resumes(false));
        assert_intent_preserved(&app, &terminal_id, &session, &plan, revision);
    }

    app.state.view.terminal_area = Rect::new(0, 0, 100, 30);
    let candidates = app.pending_agent_resume_candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].terminal_id, terminal_id);
    assert!(candidates[0].rows > 0 && candidates[0].cols > 0);
    app.sync_pending_agent_resume_deadline(now);
    let deadline = now + crate::app::PENDING_AGENT_RESUME_THEME_WAIT;
    assert_eq!(app.pending_agent_resume_deadline, Some(deadline));
    for later in [now + Duration::from_millis(1), now + Duration::from_secs(2)] {
        app.sync_pending_agent_resume_deadline(later);
        assert_eq!(app.pending_agent_resume_deadline, Some(deadline));
    }
    assert!(!app.pending_agent_resume_due(deadline - Duration::from_nanos(1)));
    assert!(app.pending_agent_resume_due(deadline));
    assert!(app.pending_agent_resume_due(deadline + Duration::from_nanos(1)));
    assert_intent_preserved(&app, &terminal_id, &session, &plan, revision);

    app.state.view.terminal_area = Rect::default();
    app.sync_pending_agent_resume_deadline(now + Duration::from_secs(3));
    assert!(app.pending_agent_resume_deadline.is_none());
    assert_intent_preserved(&app, &terminal_id, &session, &plan, revision);
    app.state.view.terminal_area = Rect::new(0, 0, 100, 30);
    let reacquired = now + Duration::from_secs(4);
    app.sync_pending_agent_resume_deadline(reacquired);
    assert_eq!(
        app.pending_agent_resume_deadline,
        Some(reacquired + crate::app::PENDING_AGENT_RESUME_THEME_WAIT)
    );

    app.state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .pending_agent_resume_plan = None;
    assert!(!app.has_pending_agent_resumes());
    app.sync_pending_agent_resume_deadline(reacquired);
    assert!(app.pending_agent_resume_deadline.is_none());
    assert_eq!(app.terminal_runtimes.len(), 0);
    assert_eq!(
        app.state.terminals[&terminal_id]
            .persisted_agent_session
            .as_ref(),
        Some(&session)
    );
    app.state.assert_invariants_for_test();
}

#[test]
fn pending_agent_resume_theme_and_empty_argv_guards_preserve_intent() {
    for empty_argv in [false, true] {
        let Fixture {
            mut app,
            terminal_id,
            session,
            mut plan,
            _directory,
        } = fixture();
        if empty_argv {
            // Defensive input: the real planner never generates an empty argv.
            plan.argv.clear();
            app.state
                .terminals
                .get_mut(&terminal_id)
                .unwrap()
                .pending_agent_resume_plan = Some(plan.clone());
        }
        let revision = app.state.terminals[&terminal_id].revision;
        app.state.view.terminal_area = Rect::new(0, 0, 100, 30);
        assert!(app.state.host_terminal_theme.is_empty());
        assert_eq!(app.pending_agent_resume_candidates().len(), 1);
        app.sync_pending_agent_resume_deadline(Instant::now());
        let deadline = app.pending_agent_resume_deadline;
        assert!(deadline.is_some());

        // A valid plan is blocked by missing theme; an empty argv remains blocked
        // even when empty theme is explicitly allowed. Both stop before spawning.
        assert!(!app.start_pending_agent_resumes(empty_argv));
        assert_intent_preserved(&app, &terminal_id, &session, &plan, revision);
        assert_eq!(app.pending_agent_resume_deadline, deadline);
        assert!(!app.start_pending_agent_resume_for_terminal(&terminal_id, 24, 80, empty_argv));
        assert_intent_preserved(&app, &terminal_id, &session, &plan, revision);
        assert_eq!(app.pending_agent_resume_deadline, deadline);
    }
}

#[tokio::test]
async fn inert_runtime_and_resume_command_submission_do_not_confirm_historical_identity() {
    let Fixture {
        mut app,
        terminal_id,
        session,
        plan,
        _directory,
    } = fixture();
    let revision = app.state.terminals[&terminal_id].revision;
    let (runtime, mut input) = crate::terminal::TerminalRuntime::test_with_channel(80, 24);
    app.terminal_runtimes.insert(terminal_id.clone(), runtime);
    app.state.view.terminal_area = Rect::new(0, 0, 100, 30);
    // An existing runtime wins over deferred resume intent, without a spawn.
    assert!(app.pending_agent_resume_candidates().is_empty());
    assert!(!app.start_pending_agent_resumes(true));
    assert!(!app.start_pending_agent_resume_for_terminal(&terminal_id, 24, 80, true));
    let mut command = shell_command_from_argv(&plan.argv).unwrap();
    command.push('\r');
    app.terminal_runtimes
        .get(&terminal_id)
        .unwrap()
        .try_send_bytes(bytes::Bytes::from(command.clone()))
        .unwrap();
    assert_eq!(input.recv().await.unwrap().as_ref(), command.as_bytes());
    // Transport acceptance is not a provider process probe.
    let terminal = &app.state.terminals[&terminal_id];
    assert_eq!(
        terminal.historical_agent_identity(),
        Some(crate::detect::Agent::Codex)
    );
    assert_eq!(terminal.foreground_agent_evidence(), None);
    assert_eq!(terminal.state, crate::detect::AgentState::Idle);
    assert_eq!(terminal.revision, revision);
    assert_eq!(terminal.persisted_agent_session.as_ref(), Some(&session));
    assert_eq!(terminal.pending_agent_resume_plan.as_ref(), Some(&plan));
    assert!(terminal.hook_authority.is_none());
    assert!(!app.state.session_dirty);
    assert!(app.session_save_deadline.is_none());
    app.state.assert_invariants_for_test();
}
