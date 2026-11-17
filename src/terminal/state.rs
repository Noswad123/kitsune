use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::detect::{Agent, AgentState};
use crate::terminal::TerminalId;

#[path = "metadata.rs"]
mod metadata;

/// Historical hook data is never foreground or lifecycle authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookAuthority {
    pub source: String,
    pub agent_label: String,
    pub state: AgentState,
    pub message: Option<String>,
    pub reported_at: Instant,
    pub session_ref: Option<crate::agent_resume::AgentSessionRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct TerminalTitleChange {
    pub(crate) raw_changed: bool,
    pub(crate) stripped_changed: bool,
}

/// Pure terminal data. Legacy agent fields are retained only as history, not
/// aliases, readiness, process evidence, or instructions to execute.
pub struct TerminalState {
    pub id: TerminalId,
    pub cwd: PathBuf,
    #[cfg(test)]
    historical_agent: Option<Agent>,
    pub hook_authority: Option<HookAuthority>,
    pub metadata_tokens: crate::metadata_tokens::MetadataTokens,
    pub persisted_agent_session: Option<crate::agent_resume::PersistedAgentSession>,
    pub terminal_title: Option<String>,
    pub manual_label: Option<String>,
    pub agent_name: Option<String>,
    pub(crate) historical_managed_agent: Option<Agent>,
    metadata_report_sequences: HashMap<String, u64>,
    #[cfg(test)]
    pub state: AgentState,
    pub revision: u64,
    pub launch_argv: Option<Vec<String>>,
    pub respawn_shell_on_exit: bool,
    // Kept as inert in-memory compatibility data; no runtime consumes this intent.
    #[allow(dead_code)]
    pub pending_agent_resume_plan: Option<crate::agent_resume::AgentResumePlan>,
}

impl TerminalState {
    pub fn new(id: TerminalId, cwd: PathBuf) -> Self {
        Self {
            id,
            cwd,
            #[cfg(test)]
            historical_agent: None,
            hook_authority: None,
            metadata_tokens: Default::default(),
            persisted_agent_session: None,
            terminal_title: None,
            manual_label: None,
            agent_name: None,
            historical_managed_agent: None,
            metadata_report_sequences: HashMap::new(),
            #[cfg(test)]
            state: AgentState::Unknown,
            revision: 0,
            launch_argv: None,
            respawn_shell_on_exit: false,
            pending_agent_resume_plan: None,
        }
    }

    pub(crate) fn terminal_title_stripped(&self) -> Option<String> {
        self.terminal_title
            .as_deref()
            .and_then(super::stripped_terminal_title)
    }

    pub(crate) fn set_terminal_title(&mut self, title: Option<String>) -> TerminalTitleChange {
        let stripped_before = self.terminal_title_stripped();
        let raw_changed = self.terminal_title != title;
        self.terminal_title = title;
        let stripped_changed = stripped_before != self.terminal_title_stripped();
        if stripped_changed {
            self.revision = self.revision.saturating_add(1);
        }
        TerminalTitleChange {
            raw_changed,
            stripped_changed,
        }
    }

    pub fn with_launch_argv(mut self, argv: Vec<String>) -> Self {
        self.launch_argv = Some(argv);
        self
    }

    pub fn set_persisted_agent_session(
        &mut self,
        session: crate::agent_resume::PersistedAgentSession,
    ) {
        self.persisted_agent_session = Some(session);
    }

    #[cfg(test)]
    pub(crate) fn seed_restored_agent_identity(&mut self, agent: Agent) {
        self.historical_agent = Some(agent);
    }

    pub fn effective_agent_label(&self) -> Option<&str> {
        None
    }
    #[cfg(test)]
    pub(crate) fn detected_agent(&self) -> Option<Agent> {
        None
    }
    #[cfg(test)]
    pub fn managed_agent_launch_pending(&self) -> bool {
        false
    }
    #[cfg(test)]
    pub fn managed_agent_interactive_ready(&self) -> bool {
        false
    }
    pub fn managed_agent_kind(&self) -> Option<Agent> {
        self.historical_managed_agent
    }
    #[cfg(test)]
    pub fn is_agent_terminal(&self) -> bool {
        false
    }

    pub fn set_manual_label(&mut self, label: String) {
        let label = label.trim().to_string();
        self.manual_label = (!label.is_empty()).then_some(label);
    }

    pub fn clear_manual_label(&mut self) {
        self.manual_label = None;
    }

    pub fn border_label(&self, _: bool) -> Option<String> {
        self.manual_label.clone()
    }

    pub fn clear_agent_runtime_identity_after_respawn(&mut self) {
        // Respawning a generic shell does not destroy historical session refs or
        // already-held historical intent, and never makes that intent executable.
        self.launch_argv = None;
        self.respawn_shell_on_exit = false;
    }

    #[cfg(test)]
    pub(crate) fn test_legacy_control_snapshot(&self) -> String {
        format!(
            "{:?}",
            (
                &self.agent_name,
                &self.hook_authority,
                &self.persisted_agent_session,
                &self.pending_agent_resume_plan,
                &self.launch_argv,
                &self.manual_label,
                self.historical_managed_agent,
                self.state,
                self.revision
            )
        )
    }

    #[cfg(test)]
    pub fn set_detected_state(&mut self, agent: Option<Agent>, state: AgentState) {
        self.historical_agent = agent;
        self.state = state;
    }

    #[cfg(test)]
    pub(crate) fn set_fallback_agent_for_test(&mut self, agent: Option<Agent>) {
        self.historical_agent = agent;
    }

    #[cfg(test)]
    pub fn set_agent_name(&mut self, name: String) {
        self.agent_name = Some(name);
    }

    #[cfg(test)]
    pub fn restore_managed_agent(&mut self, name: String, kind: Agent) {
        self.agent_name = Some(name);
        self.historical_managed_agent = Some(kind);
    }

    #[cfg(test)]
    pub fn begin_managed_agent(
        &mut self,
        name: String,
        kind: Agent,
        _: Instant,
        _: std::time::Duration,
        _: std::time::Duration,
    ) {
        self.agent_name = Some(name);
        self.historical_managed_agent = Some(kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_identity_is_not_live_authority() {
        let mut terminal = TerminalState::new(TerminalId::alloc(), PathBuf::from("/"));
        terminal.seed_restored_agent_identity(Agent::Codex);
        terminal.begin_managed_agent(
            "old".into(),
            Agent::Codex,
            Instant::now(),
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
        );
        assert_eq!(terminal.effective_agent_label(), None);
        assert!(!terminal.is_agent_terminal());
        assert!(!terminal.managed_agent_interactive_ready());
        assert!(!terminal.managed_agent_launch_pending());
        assert_eq!(terminal.border_label(true), None);
    }
}
