use super::*;

/// Cache inconclusive findings, never retained presence or screen-derived state.
/// Every actual positive probe and policy clear must cross the queue: an earlier
/// observation may have been rejected behind a consumer-side lifecycle barrier.
#[derive(Default)]
pub(super) struct DetectorProducer {
    last: Option<ForegroundObservation>,
}

impl DetectorProducer {
    pub(super) async fn reset_presence(
        &mut self,
        events: &mpsc::Sender<AppEvent>,
        pane_id: PaneId,
        epoch: DetectorEpoch,
        presence: &mut AgentDetectionPresence,
        suppressed_agent: Option<Agent>,
    ) {
        // Invalidate proof, but retain probe-policy context so a later shell-only
        // probe can retire the old session. A released kind must not exit twice.
        *presence = AgentDetectionPresence::from_agent(
            presence
                .current_agent()
                .filter(|agent| Some(*agent) != suppressed_agent),
        );
        self.reset(events, pane_id, epoch).await;
    }

    pub(super) fn observe(
        &mut self,
        finding: ForegroundObservation,
    ) -> Option<ForegroundObservation> {
        if self.last == Some(finding)
            && !matches!(
                finding,
                ForegroundObservation::Confirmed { .. } | ForegroundObservation::Cleared
            )
        {
            return None;
        }
        self.last = Some(finding);
        Some(finding)
    }

    pub(super) async fn reset(
        &mut self,
        events: &mpsc::Sender<AppEvent>,
        pane_id: PaneId,
        epoch: DetectorEpoch,
    ) {
        self.last = Some(ForegroundObservation::Reset);
        self.publish(
            events,
            pane_id,
            epoch,
            ForegroundObservation::Reset,
            std::time::Instant::now(),
        )
        .await;
    }

    // Both normal and handoff loops call this with the filtered actual probe.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn publish_probe(
        &mut self,
        events: &mpsc::Sender<AppEvent>,
        pane_id: PaneId,
        epoch: DetectorEpoch,
        agent: Option<Agent>,
        process_group_id: Option<u32>,
        action: ForegroundShellAgentAction,
        previous_agent: Option<Agent>,
        retained_agent: Option<Agent>,
        now: std::time::Instant,
    ) {
        if let Some(finding) = self.observe_probe(
            agent,
            process_group_id,
            action,
            previous_agent,
            retained_agent,
        ) {
            self.publish(events, pane_id, epoch, finding, now).await;
        }
    }

    pub(super) fn observe_probe(
        &mut self,
        agent: Option<Agent>,
        process_group_id: Option<u32>,
        action: ForegroundShellAgentAction,
        previous_agent: Option<Agent>,
        retained_agent: Option<Agent>,
    ) -> Option<ForegroundObservation> {
        let finding = if let Some(agent) = agent {
            ForegroundObservation::Confirmed {
                agent,
                process_group_id,
            }
        } else if action == ForegroundShellAgentAction::ReportProcessExit {
            // Exit belongs to the deferred Idle publication.
            return None;
        } else if action == ForegroundShellAgentAction::ClearAgent
            || (previous_agent.is_some() && retained_agent.is_none())
        {
            ForegroundObservation::Cleared
        } else {
            ForegroundObservation::Inconclusive
        };
        self.observe(finding)
    }

    async fn publish(
        &self,
        events: &mpsc::Sender<AppEvent>,
        pane_id: PaneId,
        epoch: DetectorEpoch,
        finding: ForegroundObservation,
        observed_at: std::time::Instant,
    ) {
        if let Err(err) = events
            .send(AppEvent::DetectorUpdate {
                pane_id,
                epoch,
                foreground: Some(finding),
                fallback: None,
                observed_at,
            })
            .await
        {
            warn!(pane = pane_id.raw(), %err, "failed to deliver detector observation");
        }
    }
}
