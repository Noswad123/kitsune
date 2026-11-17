use crate::api::schema::{legacy_agent_retirement_error, AgentViewClearParams, AgentViewSetParams};
use crate::app::App;

use super::responses::encode_error_body;

impl App {
    pub(super) fn handle_agent_view_set(&mut self, id: String, _: AgentViewSetParams) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.view.set"))
    }

    pub(super) fn handle_agent_view_clear(
        &mut self,
        id: String,
        _: AgentViewClearParams,
    ) -> String {
        encode_error_body(id, legacy_agent_retirement_error("agent.view.clear"))
    }
}
