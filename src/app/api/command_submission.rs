//! Generic interactive submission only. Native agent policy/identity stays outside Kitsune.

use bytes::Bytes;

use crate::api::schema::{PaneSubmitCommandParams, ResponseResult};
use crate::app::App;
use crate::terminal::TerminalRuntime;

use super::responses::{encode_error, encode_success};

const MAX_ARGUMENTS: usize = 64;
const MAX_ARGUMENT_BYTES: usize = 32768;

fn valid_arguments(argv: &[String]) -> bool {
    if !(1..=MAX_ARGUMENTS).contains(&argv.len())
        || !std::path::Path::new(&argv[0]).is_absolute()
        || argv.iter().any(|arg| arg.chars().any(char::is_control))
    {
        return false;
    }
    argv.iter()
        .try_fold(0usize, |total, arg| total.checked_add(arg.len()))
        .is_some_and(|total| total <= MAX_ARGUMENT_BYTES)
}

impl App {
    pub(super) fn handle_pane_submit_command(
        &mut self,
        id: String,
        params: PaneSubmitCommandParams,
    ) -> String {
        self.submit_command_with_shell(id, params, |runtime| {
            crate::platform::available_pane_shell(runtime.child_pid()?)
        })
    }

    fn submit_command_with_shell(
        &self,
        id: String,
        params: PaneSubmitCommandParams,
        available_shell: impl FnOnce(&TerminalRuntime) -> Option<String>,
    ) -> String {
        if !valid_arguments(&params.argv) {
            return encode_error(id, "invalid_command_argument", "Expected 1..64 arguments, an absolute executable, no control characters and at most 32768 UTF-8 bytes");
        }
        let Some((ws_idx, pane_id)) = self.parse_current_public_pane_id(&params.pane_id) else {
            return encode_error(id, "pane_not_found", "Current pane not found");
        };
        let Some(terminal_id) = self.state.workspaces[ws_idx].terminal_id(pane_id) else {
            return encode_error(id, "pane_not_found", "Current pane terminal not found");
        };
        if terminal_id.to_string() != params.expected_terminal_id {
            return encode_error(id, "terminal_mismatch", "Pane terminal does not match");
        }
        if !self.state.terminals.contains_key(terminal_id) {
            return encode_error(id, "pane_command_unavailable", "Terminal is unavailable");
        }
        let Some(runtime) = self.lookup_runtime_sender(ws_idx, pane_id) else {
            return encode_error(
                id,
                "pane_command_unavailable",
                "Terminal has no live runtime",
            );
        };
        // This is an observed foreground-shell check, not a prompt/OS process lease.
        let Some(shell) = available_shell(runtime) else {
            return encode_error(id, "pane_command_busy", "No available foreground shell");
        };
        let Some(command) = crate::platform::interactive_shell_command(&params.argv, &shell) else {
            return encode_error(
                id,
                "unsupported_shell",
                "Arguments cannot be encoded for this shell",
            );
        };
        let bytes = crate::app::api_helpers::encode_api_submission(runtime, &command);
        if runtime.try_send_bytes(Bytes::from(bytes)).is_err() {
            return encode_error(
                id,
                "pane_command_input_failed",
                "Command input could not be queued",
            );
        }
        encode_success(
            id,
            ResponseResult::PaneCommandSubmitted {
                pane_id: params.pane_id,
                terminal_id: params.expected_terminal_id,
                argv: params.argv,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, workspace::Workspace};

    fn shell() -> Option<String> {
        Some(if cfg!(windows) { "pwsh" } else { "sh" }.into())
    }

    fn fixture() -> (
        App,
        PaneSubmitCommandParams,
        tokio::sync::mpsc::Receiver<Bytes>,
    ) {
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &Config::default(),
            true,
            None,
            rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![Workspace::test_new("submission")];
        app.state.ensure_test_terminals();
        let pane = app.state.workspaces[0].tabs[0].root_pane;
        let params = PaneSubmitCommandParams {
            pane_id: app.public_pane_id(0, pane).unwrap(),
            expected_terminal_id: app.state.terminal_id_for_pane(0, pane).unwrap().to_string(),
            argv: vec![
                std::env::current_exe()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                "literal".into(),
            ],
        };
        let (runtime, rx) = TerminalRuntime::test_with_channel_capacity(80, 24, 1);
        let terminal = app.state.terminal_id_for_pane(0, pane).unwrap();
        app.terminal_runtimes.insert(terminal, runtime);
        (app, params, rx)
    }

    fn response(
        app: &App,
        params: PaneSubmitCommandParams,
        available: Option<String>,
    ) -> serde_json::Value {
        serde_json::from_str(&app.submit_command_with_shell("submit".into(), params, |_| available))
            .unwrap()
    }

    #[tokio::test]
    async fn command_submission_preserves_literal_arguments_and_one_bracketed_frame_without_agent_state(
    ) {
        let (app, mut params, mut rx) = fixture();
        params
            .argv
            .extend(["a b", "'", "$(not a command)", "", "λ"].map(str::to_owned));
        let pane = app.state.workspaces[0].tabs[0].root_pane;
        app.lookup_runtime_sender(0, pane)
            .unwrap()
            .test_process_pty_bytes(b"\x1b[?2004h");
        let command =
            crate::platform::interactive_shell_command(&params.argv, &shell().unwrap()).unwrap();
        let result = response(&app, params.clone(), shell());
        assert_eq!(result["id"], "submit");
        assert_eq!(result["result"]["type"], "pane_command_submitted");
        assert_eq!(result["result"]["terminal_id"], params.expected_terminal_id);
        assert_eq!(result["result"]["argv"], serde_json::json!(params.argv));
        assert_eq!(
            rx.try_recv().unwrap(),
            Bytes::from(format!("\x1b[200~{command}\x1b[201~\r"))
        );
        assert!(rx.try_recv().is_err());
        assert_eq!(app.state.workspaces.len(), 1);
        let terminal = &app.state.terminals[&app.state.terminal_id_for_pane(0, pane).unwrap()];
        assert!(terminal.agent_name.is_none());
        assert!(terminal.managed_agent_kind().is_none());
        assert!(terminal.effective_agent_label().is_none());
    }

    #[tokio::test]
    async fn command_submission_rejects_invalid_bounds_before_shell_observation() {
        let (app, params, mut rx) = fixture();
        let mut bad = vec![
            vec![],
            vec!["relative".into()],
            vec![String::new()],
            vec![params.argv[0].clone(), "bad\ninput".into()],
            vec![params.argv[0].clone(), "\0".into()],
            vec![params.argv[0].clone(); MAX_ARGUMENTS + 1],
            vec![params.argv[0].clone(), "λ".repeat(MAX_ARGUMENT_BYTES)],
        ];
        for argv in bad.drain(..) {
            let result: serde_json::Value = serde_json::from_str(&app.submit_command_with_shell(
                "bad".into(),
                PaneSubmitCommandParams {
                    argv,
                    ..params.clone()
                },
                |_| panic!("invalid input must not observe shell"),
            ))
            .unwrap();
            assert_eq!(result["error"]["code"], "invalid_command_argument");
            assert!(rx.try_recv().is_err());
        }
        let remaining = MAX_ARGUMENT_BYTES - params.argv[0].len();
        let exact = vec![params.argv[0].clone(), "x".repeat(remaining)];
        assert!(valid_arguments(&exact));
        let mut over = exact;
        over[1].push('x');
        assert!(!valid_arguments(&over));
        assert!(valid_arguments(std::slice::from_ref(&params.argv[0])));
        let mut max_count = vec![String::new(); MAX_ARGUMENTS];
        max_count[0] = params.argv[0].clone();
        assert!(valid_arguments(&max_count));
    }

    #[tokio::test]
    async fn command_submission_target_mismatch_and_busy_shell_write_nothing() {
        let (app, params, mut rx) = fixture();
        let mut changed = params.clone();
        changed.expected_terminal_id.push_str("-replaced");
        assert_eq!(
            response(&app, changed, shell())["error"]["code"],
            "terminal_mismatch"
        );
        let mut absent = params.clone();
        absent.pane_id = "w999:p999".into();
        assert_eq!(
            response(&app, absent, shell())["error"]["code"],
            "pane_not_found"
        );
        assert_eq!(
            response(&app, params, None)["error"]["code"],
            "pane_command_busy"
        );
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn command_submission_missing_runtime_and_backpressure_are_not_success() {
        let (mut app, params, mut rx) = fixture();
        assert_eq!(
            response(&app, params.clone(), shell())["result"]["type"],
            "pane_command_submitted"
        );
        assert_eq!(
            response(&app, params.clone(), shell())["error"]["code"],
            "pane_command_input_failed"
        );
        rx.try_recv().unwrap();
        assert!(rx.try_recv().is_err());
        let pane = app.state.workspaces[0].tabs[0].root_pane;
        let terminal = app.state.terminal_id_for_pane(0, pane).unwrap();
        app.terminal_runtimes.remove(&terminal);
        assert_eq!(
            response(&app, params, shell())["error"]["code"],
            "pane_command_unavailable"
        );
    }

    #[tokio::test]
    async fn command_submission_dispatch_does_not_guess_shell_readiness() {
        let (mut app, params, mut rx) = fixture();
        // The channel fixture has no actual shell PID. The real public handler
        // must fail closed rather than inheriting agent.start's test shortcut.
        let result: serde_json::Value =
            serde_json::from_str(&app.handle_api_request(crate::api::schema::Request {
                id: "dispatch".into(),
                method: crate::api::schema::Method::PaneSubmitCommand(params),
            }))
            .unwrap();
        assert_eq!(result["error"]["code"], "pane_command_busy");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn command_submission_rechecks_terminal_attachment_at_dispatch() {
        let (mut app, params, mut rx) = fixture();
        let pane = app.state.workspaces[0].tabs[0].root_pane;
        let replacement = crate::terminal::TerminalId::alloc();
        app.state.workspaces[0].tabs[0]
            .panes
            .get_mut(&pane)
            .unwrap()
            .attached_terminal_id = replacement;
        assert_eq!(
            response(&app, params, shell())["error"]["code"],
            "terminal_mismatch"
        );
        assert!(rx.try_recv().is_err());
    }
}
