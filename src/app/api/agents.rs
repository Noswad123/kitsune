use crate::api::schema::{
    legacy_agent_retirement_error, AgentPromptParams, AgentReadParams, AgentRenameParams,
    AgentSendKeysParams, AgentStartParams, AgentTarget,
};
use crate::app::App;

use super::responses::encode_error_body;

impl App {
    pub(super) fn handle_agent_list(&mut self, id: String) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.list"))
    }

    pub(super) fn handle_agent_get(&mut self, id: String, _: AgentTarget) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.get"))
    }

    pub(super) fn handle_agent_focus(&mut self, id: String, _: AgentTarget) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.focus"))
    }

    pub(super) fn handle_agent_rename(&mut self, id: String, _: AgentRenameParams) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.rename"))
    }

    pub(super) fn handle_agent_start(&mut self, id: String, _: AgentStartParams) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.start"))
    }

    pub(super) fn handle_agent_prompt(&mut self, id: String, _: AgentPromptParams) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.prompt"))
    }

    pub(super) fn handle_agent_read(&mut self, id: String, _: AgentReadParams) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.read"))
    }

    pub(super) fn handle_agent_explain(&mut self, id: String, _: AgentTarget) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.explain"))
    }

    pub(super) fn handle_agent_send_keys(&mut self, id: String, _: AgentSendKeysParams) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.send_keys"))
    }
}
