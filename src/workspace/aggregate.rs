use std::collections::HashMap;

use crate::detect::AgentState;
use crate::terminal::{TerminalId, TerminalState};

use super::Workspace;

impl Workspace {
    pub fn aggregate_state(&self, _: &HashMap<TerminalId, TerminalState>) -> (AgentState, bool) {
        (AgentState::Unknown, true)
    }

    pub fn has_working_pane(&self, _: &HashMap<TerminalId, TerminalState>) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_agent_state_does_not_create_workspace_attention() {
        let workspace = Workspace::test_adversarial_identity_state();
        for state in [AgentState::Working, AgentState::Blocked, AgentState::Idle] {
            let mut terminals = HashMap::new();
            for tab in &workspace.tabs {
                for pane in tab.panes.values() {
                    let mut terminal =
                        TerminalState::new(pane.attached_terminal_id.clone(), "/".into());
                    terminal.state = state;
                    terminal.agent_name = Some("historical".into());
                    terminals.insert(terminal.id.clone(), terminal);
                }
            }
            assert_eq!(
                workspace.aggregate_state(&terminals),
                (AgentState::Unknown, true)
            );
            assert!(!workspace.has_working_pane(&terminals));
        }
    }
}
