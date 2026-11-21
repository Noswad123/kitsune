use crate::api::schema::{
    ResponseResult, TerminalAvailability, TerminalFocusParams, TerminalInspectParams,
};
use crate::app::App;
use crate::layout::PaneId;
use crate::terminal::TerminalId;

use super::responses::{encode_error, encode_success};

struct NavigationTarget {
    terminal_id: TerminalId,
    attachment: Option<(usize, usize, PaneId)>,
    workspace_id: Option<String>,
    tab_id: Option<String>,
    pane_id: Option<String>,
}

impl App {
    // Deliberately does not use resolve_terminal_target: that accepts pane/name aliases.
    fn navigation_target(&self, exact_id: &str) -> Result<NavigationTarget, &'static str> {
        let terminal_id = self
            .state
            .terminals
            .keys()
            .find(|id| id.to_string() == exact_id)
            .ok_or("terminal_not_found")?
            .clone();
        let mut attachment = None;
        for (ws_idx, ws) in self.state.workspaces.iter().enumerate() {
            for (tab_idx, tab) in ws.tabs.iter().enumerate() {
                for pane_id in tab.layout.pane_ids() {
                    if tab.terminal_id(pane_id) == Some(&terminal_id)
                        && attachment.replace((ws_idx, tab_idx, pane_id)).is_some()
                    {
                        return Err("terminal_ambiguous");
                    }
                }
            }
        }
        let (workspace_id, tab_id, pane_id) = match attachment {
            Some((ws, tab, pane)) => (
                Some(self.public_workspace_id(ws)),
                Some(self.public_tab_id(ws, tab).ok_or("terminal_unavailable")?),
                Some(
                    self.public_pane_id(ws, pane)
                        .ok_or("terminal_unavailable")?,
                ),
            ),
            None => (None, None, None),
        };
        Ok(NavigationTarget {
            terminal_id,
            attachment,
            workspace_id,
            tab_id,
            pane_id,
        })
    }

    pub(super) fn handle_terminal_inspect(
        &self,
        id: String,
        params: TerminalInspectParams,
    ) -> String {
        let target = match self.navigation_target(&params.terminal_id) {
            Ok(target) => target,
            Err(code) => {
                return encode_error(id, code, "exact terminal target could not be resolved")
            }
        };
        let runtime = self.terminal_runtimes.get(&target.terminal_id);
        let availability = if target.attachment.is_none() {
            TerminalAvailability::Detached
        } else {
            match runtime {
                None => TerminalAvailability::Unavailable,
                Some(runtime) if runtime.process_exited() => TerminalAvailability::Exited,
                Some(_) => TerminalAvailability::Live,
            }
        };
        encode_success(
            id,
            ResponseResult::TerminalInspected {
                terminal_id: target.terminal_id.to_string(),
                runtime_generation: runtime.map(|runtime| runtime.runtime_generation().to_owned()),
                availability,
                workspace_id: target.workspace_id,
                tab_id: target.tab_id,
                pane_id: target.pane_id,
            },
        )
    }

    pub(super) fn handle_terminal_focus(
        &mut self,
        id: String,
        params: TerminalFocusParams,
    ) -> String {
        let target = match self.navigation_target(&params.terminal_id) {
            Ok(target) => target,
            Err(code) => {
                return encode_error(id, code, "exact terminal target could not be resolved")
            }
        };
        let Some((ws_idx, tab_idx, pane)) = target.attachment else {
            return encode_error(id, "terminal_detached", "terminal is not attached");
        };
        let Some(runtime) = self.terminal_runtimes.get(&target.terminal_id) else {
            return encode_error(
                id,
                "terminal_unavailable",
                "terminal runtime is unavailable",
            );
        };
        if runtime.runtime_generation() != params.expected_runtime_generation {
            return encode_error(
                id,
                "runtime_generation_mismatch",
                "terminal runtime has changed",
            );
        }
        if runtime.process_exited() {
            return encode_error(id, "terminal_exited", "terminal PTY process has exited");
        }
        let (Some(workspace_id), Some(tab_id), Some(pane_id)) =
            (target.workspace_id, target.tab_id, target.pane_id)
        else {
            return encode_error(
                id,
                "terminal_unavailable",
                "terminal locator is unavailable",
            );
        };
        let runtime_generation = runtime.runtime_generation().to_owned();

        // Legacy switch_tab/focus helpers mark panes seen. Generic focus must not.
        // All admission checks precede these synchronous focus mutations.
        let previous = self.state.current_pane_focus_target();
        self.state.active = Some(ws_idx);
        self.state.selected = ws_idx;
        let workspace = &mut self.state.workspaces[ws_idx];
        workspace.active_tab = tab_idx;
        workspace.tabs[tab_idx].layout.focus_pane(pane);
        self.state.record_pane_focus_change(previous, ws_idx, pane);
        self.state.mark_session_dirty();
        self.state.settle_terminal_mode_after_focus();
        encode_success(
            id,
            ResponseResult::TerminalFocused {
                terminal_id: target.terminal_id.to_string(),
                runtime_generation,
                workspace_id,
                tab_id,
                pane_id,
            },
        )
    }
}
