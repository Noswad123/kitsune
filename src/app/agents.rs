use super::App;

impl App {
    // Historical pane session references are not a live agent inventory.
    pub(super) fn collect_agent_infos(&self) -> Vec<crate::api::schema::AgentInfo> {
        Vec::new()
    }
}
