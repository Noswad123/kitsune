use serde::{Deserialize, Serialize};

pub(crate) const DETECTOR_MANIFEST_RETIREMENT_MESSAGE: &str = "Built-in detector-manifest management has been retired; Kitsune no longer downloads or reloads agent-detection manifests. Generic pane/process operations remain available.";

pub(crate) fn legacy_agent_retirement_error(method: &'static str) -> ErrorBody {
    ErrorBody {
        code: "feature_retired".into(),
        message: format!("Legacy agent operation {method} is retired; Kitsune no longer provides built-in agent lifecycle, readiness, or status. Use generic terminal/pane operations, or an external agent manager."),
    }
}

pub(crate) fn legacy_agent_retired_method(method: &Method) -> Option<&'static str> {
    Some(match method {
        Method::AgentList(_) => "agent.list",
        Method::AgentGet(_) => "agent.get",
        Method::AgentRead(_) => "agent.read",
        Method::AgentExplain(_) => "agent.explain",
        Method::AgentSendKeys(_) => "agent.send_keys",
        Method::AgentRename(_) => "agent.rename",
        Method::AgentFocus(_) => "agent.focus",
        Method::AgentStart(_) => "agent.start",
        Method::AgentPrompt(_) => "agent.prompt",
        Method::AgentWait(_) => "agent.wait",
        Method::AgentViewSet(_) => "agent.view.set",
        Method::AgentViewClear(_) => "agent.view.clear",
        Method::IntegrationInstall(_) => "integration.install",
        Method::EventsSubscribe(params)
            if params.subscriptions.iter().any(|subscription| {
                matches!(subscription, Subscription::PaneAgentStatusChanged { .. })
            }) =>
        {
            "events.subscribe"
        }
        Method::EventsWait(params)
            if matches!(
                params.match_event,
                EventMatch::PaneAgentStatusChanged { .. }
            ) =>
        {
            "events.wait"
        }
        _ => return None,
    })
}

/// Compatibility tombstones are independent of runtime ownership and target identity.
pub(crate) fn retired_request_response(request: &Request) -> Option<serde_json::Value> {
    let error = if let Some(method) = legacy_agent_retired_method(&request.method) {
        legacy_agent_retirement_error(method)
    } else if matches!(
        request.method,
        Method::ServerAgentManifests(_) | Method::ServerReloadAgentManifests(_)
    ) {
        ErrorBody {
            code: "feature_retired".into(),
            message: DETECTOR_MANIFEST_RETIREMENT_MESSAGE.into(),
        }
    } else {
        return None;
    };
    Some(serde_json::json!({"id": request.id, "error": error}))
}

pub mod agents;
pub mod common;
pub mod events;
pub mod integration_views;
pub mod integrations;
pub mod panes;
pub mod response;
pub mod server;
pub mod session;
pub mod tabs;
pub mod terminal_navigation;
pub mod workspaces;
pub mod worktrees;

pub use agents::*;
pub use common::*;
pub use events::*;
pub use integration_views::*;
pub use integrations::*;
pub use panes::*;
pub use response::*;
pub use server::*;
pub use session::{AnnotationInfo, CommandSubmissionInfo, ProcessObservationInfo, SessionSnapshot};
pub use tabs::*;
pub use terminal_navigation::*;
pub use workspaces::*;
pub use worktrees::*;

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Request {
    pub id: String,
    #[serde(flatten)]
    pub method: Method,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "method", content = "params")]
// Request enums are short-lived wire values; keeping variants direct preserves
// the simple serde shape and avoids boxing churn across every caller.
#[allow(clippy::large_enum_variant)]
pub enum Method {
    #[serde(rename = "ping")]
    Ping(PingParams),
    #[serde(rename = "server.stop")]
    ServerStop(EmptyParams),
    #[serde(rename = "server.live_handoff")]
    ServerLiveHandoff(ServerLiveHandoffParams),
    #[serde(rename = "server.reload_config")]
    ServerReloadConfig(EmptyParams),
    // Compatibility-only methods. Both unconditionally return feature_retired.
    #[serde(rename = "server.agent_manifests")]
    ServerAgentManifests(EmptyParams),
    #[serde(rename = "server.reload_agent_manifests")]
    ServerReloadAgentManifests(EmptyParams),
    #[serde(rename = "notification.show")]
    NotificationShow(NotificationShowParams),
    #[serde(rename = "client.window_title.set")]
    ClientWindowTitleSet(ClientWindowTitleSetParams),
    #[serde(rename = "client.window_title.clear")]
    ClientWindowTitleClear(EmptyParams),
    #[serde(rename = "session.snapshot")]
    SessionSnapshot(EmptyParams),
    #[serde(rename = "terminal.inspect")]
    TerminalInspect(TerminalInspectParams),
    #[serde(rename = "terminal.focus")]
    TerminalFocus(TerminalFocusParams),
    #[serde(rename = "integration.view.open")]
    ViewOpen(ViewOpenParams),
    #[serde(rename = "integration.view.replace")]
    ViewReplace(ViewReplaceParams),
    #[serde(rename = "integration.view.poll")]
    ViewPoll(ViewOwnerParams),
    #[serde(rename = "integration.view.close")]
    ViewClose(ViewOwnerParams),
    #[serde(rename = "workspace.create")]
    WorkspaceCreate(WorkspaceCreateParams),
    #[serde(rename = "workspace.list")]
    WorkspaceList(EmptyParams),
    #[serde(rename = "workspace.get")]
    WorkspaceGet(WorkspaceTarget),
    #[serde(rename = "workspace.focus")]
    WorkspaceFocus(WorkspaceTarget),
    #[serde(rename = "workspace.rename")]
    WorkspaceRename(WorkspaceRenameParams),
    #[serde(rename = "workspace.move")]
    WorkspaceMove(WorkspaceMoveParams),
    #[serde(rename = "workspace.report_metadata")]
    WorkspaceReportMetadata(WorkspaceReportMetadataParams),
    #[serde(rename = "workspace.close")]
    WorkspaceClose(WorkspaceTarget),
    #[serde(rename = "worktree.list")]
    WorktreeList(WorktreeListParams),
    #[serde(rename = "worktree.create")]
    WorktreeCreate(WorktreeCreateParams),
    #[serde(rename = "worktree.open")]
    WorktreeOpen(WorktreeOpenParams),
    #[serde(rename = "worktree.remove")]
    WorktreeRemove(WorktreeRemoveParams),
    #[serde(rename = "tab.create")]
    TabCreate(TabCreateParams),
    #[serde(rename = "tab.list")]
    TabList(TabListParams),
    #[serde(rename = "tab.get")]
    TabGet(TabTarget),
    #[serde(rename = "tab.focus")]
    TabFocus(TabTarget),
    #[serde(rename = "tab.rename")]
    TabRename(TabRenameParams),
    #[serde(rename = "tab.move")]
    TabMove(TabMoveParams),
    #[serde(rename = "tab.close")]
    TabClose(TabTarget),
    #[serde(rename = "agent.list")]
    /// Compatibility-only agent operations always return feature_retired.
    AgentList(EmptyParams),
    #[serde(rename = "agent.get")]
    /// Compatibility-only; live requests always return feature_retired.
    AgentGet(AgentTarget),
    #[serde(rename = "agent.read")]
    /// Compatibility-only; live requests always return feature_retired.
    AgentRead(AgentReadParams),
    #[serde(rename = "agent.explain")]
    /// Compatibility-only; live requests always return feature_retired. Offline CLI file diagnostics remain available.
    AgentExplain(AgentTarget),
    #[serde(rename = "agent.send_keys")]
    /// Compatibility-only; live requests always return feature_retired.
    AgentSendKeys(AgentSendKeysParams),
    #[serde(rename = "agent.rename")]
    /// Compatibility-only; live requests always return feature_retired.
    AgentRename(AgentRenameParams),
    #[serde(rename = "agent.view.set")]
    /// Compatibility-only; live requests always return feature_retired.
    AgentViewSet(AgentViewSetParams),
    #[serde(rename = "agent.view.clear")]
    /// Compatibility-only; live requests always return feature_retired.
    AgentViewClear(AgentViewClearParams),
    #[serde(rename = "agent.focus")]
    /// Compatibility-only; live requests always return feature_retired.
    AgentFocus(AgentTarget),
    #[serde(rename = "agent.start")]
    /// Compatibility-only; live requests always return feature_retired without launching or checking readiness.
    AgentStart(AgentStartParams),
    #[serde(rename = "agent.prompt")]
    /// Compatibility-only; live requests always return feature_retired, including prompts with wait options.
    AgentPrompt(AgentPromptParams),
    #[serde(rename = "agent.wait")]
    /// Compatibility-only; live requests always return feature_retired without polling, including without a timeout.
    AgentWait(AgentWaitParams),
    #[serde(rename = "pane.split")]
    PaneSplit(PaneSplitParams),
    #[serde(rename = "pane.swap")]
    PaneSwap(PaneSwapParams),
    #[serde(rename = "pane.move")]
    PaneMove(PaneMoveParams),
    #[serde(rename = "pane.zoom")]
    PaneZoom(PaneZoomParams),
    #[serde(rename = "pane.layout")]
    PaneLayout(PaneLayoutParams),
    #[serde(rename = "pane.process_info")]
    PaneProcessInfo(PaneProcessInfoParams),
    #[serde(rename = "layout.export")]
    LayoutExport(LayoutExportParams),
    #[serde(rename = "layout.apply")]
    LayoutApply(LayoutApplyParams),
    #[serde(rename = "layout.set_split_ratio")]
    LayoutSetSplitRatio(LayoutSetSplitRatioParams),
    #[serde(rename = "pane.neighbor")]
    PaneNeighbor(PaneNeighborParams),
    #[serde(rename = "pane.edges")]
    PaneEdges(PaneEdgesParams),
    #[serde(rename = "pane.focus_direction")]
    PaneFocusDirection(PaneFocusDirectionParams),
    #[serde(rename = "pane.resize")]
    PaneResize(PaneResizeParams),
    #[serde(rename = "pane.list")]
    PaneList(PaneListParams),
    #[serde(rename = "pane.current")]
    PaneCurrent(PaneCurrentParams),
    #[serde(rename = "pane.get")]
    PaneGet(PaneTarget),
    #[serde(rename = "pane.focus")]
    PaneFocus(PaneTarget),
    #[serde(rename = "pane.rename")]
    PaneRename(PaneRenameParams),
    #[serde(rename = "pane.annotate")]
    PaneAnnotate(PaneAnnotateParams),
    #[serde(rename = "pane.send_text")]
    PaneSendText(PaneSendTextParams),
    #[serde(rename = "pane.send_keys")]
    PaneSendKeys(PaneSendKeysParams),
    #[serde(rename = "pane.send_input")]
    PaneSendInput(PaneSendInputParams),
    #[serde(rename = "pane.submit_command")]
    PaneSubmitCommand(PaneSubmitCommandParams),
    #[serde(rename = "pane.read")]
    PaneRead(PaneReadParams),
    #[serde(rename = "pane.graphics.set")]
    PaneGraphicsSet(PaneGraphicsSetParams),
    #[serde(rename = "pane.graphics.clear")]
    PaneGraphicsClear(PaneGraphicsClearParams),
    #[serde(rename = "pane.graphics.info")]
    PaneGraphicsInfo(PaneTarget),
    #[serde(rename = "pane.graphics.stream")]
    #[schemars(skip)]
    PaneGraphicsStream(PaneGraphicsStreamParams),
    #[serde(skip)]
    #[schemars(skip)]
    PaneGraphicsStreamSet(PaneGraphicsSetParams),
    #[serde(skip)]
    #[schemars(skip)]
    PaneGraphicsStreamOpen(PaneGraphicsStreamParams),
    #[serde(skip)]
    #[schemars(skip)]
    PaneGraphicsStreamClose(PaneGraphicsStreamParams),
    #[serde(rename = "pane.report_agent")]
    PaneReportAgent(PaneReportAgentParams),
    #[serde(rename = "pane.report_agent_session")]
    PaneReportAgentSession(PaneReportAgentSessionParams),
    #[serde(rename = "pane.report_metadata")]
    PaneReportMetadata(PaneReportMetadataParams),
    #[serde(rename = "pane.clear_agent_authority")]
    PaneClearAgentAuthority(PaneClearAgentAuthorityParams),
    #[serde(rename = "pane.release_agent")]
    PaneReleaseAgent(PaneReleaseAgentParams),
    #[serde(rename = "pane.close")]
    PaneClose(PaneTarget),
    #[serde(rename = "popup.close")]
    PopupClose(EmptyParams),
    #[serde(rename = "events.subscribe")]
    EventsSubscribe(EventsSubscribeParams),
    #[serde(rename = "events.wait")]
    EventsWait(EventsWaitParams),
    #[serde(rename = "pane.wait_for_output")]
    PaneWaitForOutput(PaneWaitForOutputParams),
    #[serde(rename = "integration.install")]
    IntegrationInstall(IntegrationInstallParams),
    #[serde(rename = "integration.uninstall")]
    IntegrationUninstall(IntegrationUninstallParams),
}

#[cfg(test)]
mod tests;
