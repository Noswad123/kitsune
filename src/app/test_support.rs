use std::sync::{atomic::AtomicBool, Arc};
use std::time::Instant;

use super::{App, AppState};

// Avoid App::new: these tests need no application config loading or runtimes.
pub(crate) fn app_from_state(state: AppState) -> App {
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
    let (_, api_rx) = tokio::sync::mpsc::unbounded_channel();
    let now = Instant::now();
    App {
        state,
        terminal_runtimes: crate::terminal::TerminalRuntimeRegistry::new(),
        event_tx,
        event_rx,
        api_rx,
        event_hub: crate::api::EventHub::default(),
        last_focus: None,
        no_session: true,
        input_rx: None,
        last_terminal_size: None,
        config_diagnostic_deadline: None,
        toast_deadline: None,
        copy_feedback_deadline: None,
        last_api_notification_at: None,
        last_git_remote_status_refresh: now,
        last_git_repo_discovery_refresh: now,
        git_refresh_in_flight: false,
        git_refresh_due_after_in_flight: false,
        git_identity_refresh_requested: false,
        git_status_cache: Default::default(),
        pending_api_worktree_creates: Default::default(),
        pending_api_worktree_removes: Default::default(),
        pending_api_worktree_remove_paths: Default::default(),
        next_api_worktree_operation_id: 1,
        last_sidebar_divider_click: None,
        last_pane_click: None,
        pending_url_click_sources: Default::default(),
        next_resize_poll: now,
        next_animation_tick: None,
        next_agent_manifest_update_check: None,
        update_manifest_check_enabled: false,
        loaded_host_cursor: Default::default(),
        agent_metadata_deadline: None,
        pending_agent_resume_deadline: None,
        selection_autoscroll_deadline: None,
        selection_highlight_clear_deadline: None,
        session_save_deadline: None,
        session_save_thread: None,
        detached_custom_command_children: Vec::new(),
        persist_pane_history: false,
        last_render_at: None,
        pressed_terminal_keys: Default::default(),
        suppressed_repeat_keys: Default::default(),
        render_notify: Arc::new(tokio::sync::Notify::new()),
        render_dirty: Arc::new(AtomicBool::new(false)),
        full_redraw_pending: false,
        overlay_panes: Default::default(),
        local_terminal_notifications: false,
        local_input_source_switch: false,
        config_reloaded_from_disk: false,
        prefix_input_source: Box::new(crate::platform::RealPrefixInputSource::default()),
    }
}
