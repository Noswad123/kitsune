use super::*;

impl TerminalState {
    /// True only for the most recent detector application restoring an unchanged
    /// suspended native projection with no accepted state changes in its window.
    /// Consumers must match the actual event's epoch
    /// and observation time; this is internal provenance, never wire state.
    pub(crate) fn detector_projection_reconfirmed(
        &self,
        epoch: DetectorEpoch,
        observed_at: Instant,
    ) -> bool {
        self.detector_epoch == Some(epoch)
            && self.last_detector_projection_reconfirmation == Some((epoch, observed_at))
    }

    fn suspend_native_projection(&mut self) {
        if self.suspended_native_projection.is_none() && self.live_full_lifecycle_hook_authority() {
            self.suspended_native_projection =
                self.hook_authority
                    .clone()
                    .map(|authority| SuspendedNativeProjection {
                        authority,
                        state_changed: false,
                    });
        }
    }

    /// Legacy presentation fixtures may set a label without fabricating a probe.
    #[cfg(test)]
    pub(crate) fn set_fallback_agent_for_test(&mut self, agent: Option<Agent>) {
        self.detected_agent = agent;
    }

    /// Compatibility presentation identity, never evidence of a live process.
    pub(crate) fn detected_agent(&self) -> Option<Agent> {
        self.detected_agent.or_else(|| {
            self.fallback_observed_at
                .is_none()
                .then_some(self.historical_agent)
                .flatten()
        })
    }

    #[cfg(test)]
    pub(crate) fn historical_agent_identity(&self) -> Option<Agent> {
        self.historical_agent
    }

    /// Last genuine positive probe, retained until the detector's clearing policy
    /// or an explicit lifecycle boundary invalidates it. There is no extra TTL.
    pub(crate) fn foreground_agent_evidence(&self) -> Option<(Agent, Instant)> {
        self.foreground_evidence
            .filter(|evidence| Some(evidence.epoch) == self.detector_epoch)
            .map(|evidence| (evidence.agent, evidence.observed_at))
    }

    /// Group context of the last positive probe, not a native generation ID.
    #[cfg(test)]
    pub(crate) fn foreground_process_group_id(&self) -> Option<u32> {
        self.foreground_evidence
            .and_then(|evidence| evidence.process_group_id)
    }

    pub(super) fn foreground_agent(&self) -> Option<Agent> {
        self.foreground_agent_evidence().map(|(agent, _)| agent)
    }

    pub(crate) fn seed_restored_agent_identity(&mut self, agent: Agent) -> TerminalStateMutation {
        let now = Instant::now();
        let previous = self.unchanged_effective_state_change_at(now);
        self.historical_agent = Some(agent);
        if self.fallback_observed_at.is_none() {
            self.fallback_state = AgentState::Idle;
        }
        TerminalStateMutation {
            effective_state_change: self.recompute_effective_state(
                previous.agent_label,
                previous.known_agent,
                previous.state,
                previous.presentation,
                now,
            ),
            ..TerminalStateMutation::default()
        }
    }

    /// The App must synchronize the active runtime's epoch before delivering any
    /// detector events. Updates never implicitly bind or replace an epoch. `now`
    /// is the lower bound for this incarnation's observation timestamps, not a
    /// later delivery time for an already queued event.
    pub(crate) fn synchronize_detector_epoch(
        &mut self,
        epoch: Option<DetectorEpoch>,
        now: Instant,
    ) -> TerminalStateMutation {
        if self.detector_epoch == epoch {
            return TerminalStateMutation::default();
        }
        self.last_detector_projection_reconfirmation = None;
        self.suspend_native_projection();
        let previous = self.unchanged_effective_state_change_at(now);
        self.detector_epoch = epoch;
        self.detector_observed_at = Some(self.detector_observed_at.map_or(now, |at| at.max(now)));
        self.foreground_evidence = None;
        TerminalStateMutation {
            effective_state_change: self.recompute_effective_state(
                previous.agent_label,
                previous.known_agent,
                previous.state,
                previous.presentation,
                now,
            ),
            ..TerminalStateMutation::default()
        }
    }

    pub(crate) fn apply_detector_update(
        &mut self,
        epoch: DetectorEpoch,
        foreground: Option<ForegroundObservation>,
        fallback: Option<FallbackObservation>,
        observed_at: Instant,
    ) -> TerminalStateMutation {
        self.last_detector_projection_reconfirmation = None;
        if self.detector_epoch != Some(epoch)
            || self.detector_observed_at.is_some_and(|at| observed_at < at)
            || self
                .recent_agent_process_exit
                .is_some_and(|exit| observed_at < exit.observed_at)
        {
            return TerminalStateMutation::default();
        }
        // A later screen sample must not prevent a genuine probe from being
        // accepted, but its presentation must not be overwritten by an older one.
        let fallback =
            fallback.filter(|_| self.fallback_observed_at.is_none_or(|at| observed_at >= at));
        let previous = self.unchanged_effective_state_change_at(observed_at);
        let previous_session = self.current_session_identity_for_persistence();
        let previous_foreground = self.foreground_agent().or_else(|| {
            // Policy-confirmed absence also retires an owner suspended by Reset.
            (foreground == Some(ForegroundObservation::Cleared))
                .then(|| {
                    self.hook_authority.as_ref().and_then(|authority| {
                        crate::detect::parse_agent_label(&authority.agent_label)
                    })
                })
                .flatten()
        });
        let mut process_exited = false;
        let mut exit_agent = None;
        let foreground_update = matches!(
            foreground,
            Some(
                ForegroundObservation::Confirmed { .. }
                    | ForegroundObservation::Cleared
                    | ForegroundObservation::Exited { .. }
            )
        );
        if let Some(observation) = foreground {
            match observation {
                ForegroundObservation::Confirmed {
                    agent,
                    process_group_id,
                } => {
                    if self.detected_state_observed_before_release_suppression(
                        Some(agent),
                        observed_at,
                    ) {
                        return TerminalStateMutation::default();
                    }
                    self.foreground_evidence = Some(ForegroundAgentEvidence {
                        agent,
                        process_group_id,
                        epoch,
                        observed_at,
                    });
                    self.recent_agent_process_exit = None;
                    let replacement_source = self
                        .suppressed_full_lifecycle_hook_reports
                        .iter()
                        .find(|(_, suppressed)| {
                            crate::detect::parse_agent_label(&suppressed.agent_label) == Some(agent)
                                && suppressed.reason
                                    == FullLifecycleHookSuppressionReason::ProcessExit
                                && suppressed.replacement_session_ref.is_some()
                        })
                        .map(|(source, _)| source.clone());
                    if replacement_source.as_ref().is_some_and(|source| {
                        self.hook_authority
                            .as_ref()
                            .is_some_and(|authority| &authority.source != source)
                    }) {
                        self.suppress_current_full_lifecycle_hook_authority(
                            FullLifecycleHookSuppressionReason::HookClear,
                        );
                        self.hook_authority = None;
                    }
                    let reconfirming_retained_authority =
                        self.hook_authority.as_ref().is_some_and(|authority| {
                            crate::detect::full_lifecycle_hook_authority(
                                &authority.source,
                                &authority.agent_label,
                            ) && crate::detect::parse_agent_label(&authority.agent_label)
                                == Some(agent)
                                && replacement_source.is_none()
                        });
                    if !reconfirming_retained_authority {
                        self.clear_full_lifecycle_hook_suppression_for_detected_agent(
                            previous_foreground,
                            Some(agent),
                        );
                    }
                }
                ForegroundObservation::Inconclusive => {}
                ForegroundObservation::Reset => {
                    self.suspend_native_projection();
                    self.foreground_evidence = None;
                }
                ForegroundObservation::Cleared => {
                    self.suspended_native_projection = None;
                    self.foreground_evidence = None;
                }
                ForegroundObservation::Exited { agent } => {
                    self.suspended_native_projection = None;
                    self.foreground_evidence = None;
                    process_exited = true;
                    exit_agent = Some(agent);
                }
            }
            self.detector_observed_at = Some(observed_at);
        }
        let presentation_agent = if process_exited {
            exit_agent
        } else {
            fallback
                .map(|fallback| fallback.agent)
                .unwrap_or(self.detected_agent())
        };
        let mut mutation = self.update_detection_presentation(
            presentation_agent,
            fallback,
            process_exited,
            foreground_update,
            previous_foreground,
            observed_at,
        );
        mutation.effective_state_change = self.recompute_effective_state(
            previous.agent_label,
            previous.known_agent,
            previous.state,
            previous.presentation,
            observed_at,
        );
        mutation.session_ref_changed =
            previous_session != self.current_session_identity_for_persistence();
        if matches!(foreground, Some(ForegroundObservation::Confirmed { .. })) {
            let reconfirmed = self
                .suspended_native_projection
                .take()
                .is_some_and(|suspended| {
                    previous.state == AgentState::Unknown
                        && !suspended.state_changed
                        && self.live_full_lifecycle_hook_authority()
                        && self.hook_authority.as_ref().is_some_and(|authority| {
                            authority.source == suspended.authority.source
                                && authority.agent_label == suspended.authority.agent_label
                                && authority.session_ref == suspended.authority.session_ref
                                && authority.state == suspended.authority.state
                        })
                });
            if reconfirmed {
                self.last_detector_projection_reconfirmation = Some((epoch, observed_at));
                if let Some(change) = mutation.effective_state_change.as_mut() {
                    change.projection_reconfirmed = true;
                }
            }
        }
        mutation
    }

    pub(crate) fn publish_fallback_observation(
        &mut self,
        fallback: FallbackObservation,
        observed_at: Instant,
    ) -> TerminalStateMutation {
        self.last_detector_projection_reconfirmation = None;
        if self.fallback_observed_at.is_some_and(|at| observed_at < at)
            || self.detector_observed_at.is_some_and(|at| observed_at < at)
            || self
                .recent_agent_process_exit
                .is_some_and(|exit| observed_at <= exit.observed_at)
        {
            return TerminalStateMutation::default();
        }
        self.update_detection_presentation(
            fallback.agent,
            Some(fallback),
            false,
            false,
            self.foreground_agent(),
            observed_at,
        )
    }

    pub(crate) fn record_process_exit(
        &mut self,
        agent: Option<Agent>,
        observed_at: Instant,
    ) -> TerminalStateMutation {
        self.last_detector_projection_reconfirmation = None;
        if self.detector_observed_at.is_some_and(|at| observed_at < at)
            || self
                .recent_agent_process_exit
                .is_some_and(|exit| observed_at < exit.observed_at)
        {
            return TerminalStateMutation::default();
        }
        let previous = self.unchanged_effective_state_change_at(observed_at);
        let previous_foreground = self.foreground_agent();
        let agent = agent.or(previous_foreground).or(self.detected_agent());
        self.foreground_evidence = None;
        self.suspended_native_projection = None;
        self.detector_observed_at = Some(observed_at);
        let mut mutation = self.update_detection_presentation(
            agent,
            Some(FallbackObservation {
                agent,
                state: AgentState::Idle,
                visible_blocker: false,
                visible_working: false,
            }),
            true,
            true,
            previous_foreground,
            observed_at,
        );
        mutation.effective_state_change = self.recompute_effective_state(
            previous.agent_label,
            previous.known_agent,
            previous.state,
            previous.presentation,
            observed_at,
        );
        mutation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_native_terminal() -> (TerminalState, DetectorEpoch, Instant) {
        let mut terminal = terminal();
        let epoch = DetectorEpoch::new();
        let now = Instant::now();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            Some(fallback(Agent::Djinn, AgentState::Idle)),
            now,
        );
        let session = crate::agent_resume::AgentSessionRef::id("session");
        terminal.set_agent_session_ref_for_session_start(
            "kitsune:djinn".into(),
            "djinn".into(),
            session.clone(),
            Some(19),
            Some("new".into()),
        );
        terminal.set_hook_authority_at(
            "kitsune:djinn".into(),
            "djinn".into(),
            AgentState::Working,
            None,
            session,
            Some(20),
            now,
        );
        assert_eq!(terminal.state, AgentState::Working);
        (terminal, epoch, now)
    }

    #[test]
    fn buffered_idle_working_idle_activity_is_completion_eligible_once() {
        let (mut terminal, epoch, now) = active_native_terminal();
        terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Idle,
                None,
                crate::agent_resume::AgentSessionRef::id("session"),
                Some(21),
                now,
            )
            .unwrap();
        terminal.apply_detector_update(epoch, Some(ForegroundObservation::Reset), None, now);
        terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Working,
                None,
                crate::agent_resume::AgentSessionRef::id("session"),
                Some(22),
                now + Duration::from_secs(1),
            )
            .unwrap();
        // A repeated invalidation in this window must not erase accepted activity.
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Reset),
            None,
            now + Duration::from_secs(1),
        );
        terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Idle,
                None,
                crate::agent_resume::AgentSessionRef::id("session"),
                Some(23),
                now + Duration::from_secs(2),
            )
            .unwrap();
        assert_eq!(terminal.state, AgentState::Unknown);
        let observed_at = now + Duration::from_secs(3);
        let mutation = terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            observed_at,
        );
        let change = mutation.effective_state_change.unwrap();
        assert_eq!(change.previous_state, AgentState::Unknown);
        assert_eq!(change.state, AgentState::Idle);
        assert!(!change.projection_reconfirmed);
        assert!(!terminal.detector_projection_reconfirmed(epoch, observed_at));
        let repeated = terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            now + Duration::from_secs(4),
        );
        assert!(repeated.effective_state_change.is_none());
        // The next suspension has its own clean activity window.
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Reset),
            None,
            now + Duration::from_secs(5),
        );
        let mutation = terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            now + Duration::from_secs(6),
        );
        assert!(
            mutation
                .effective_state_change
                .unwrap()
                .projection_reconfirmed
        );
    }

    #[test]
    fn repeated_buffered_idle_report_remains_quiet_reconfirmation() {
        let (mut terminal, epoch, now) = active_native_terminal();
        terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Idle,
                None,
                crate::agent_resume::AgentSessionRef::id("session"),
                Some(21),
                now,
            )
            .unwrap();
        terminal.apply_detector_update(epoch, Some(ForegroundObservation::Reset), None, now);
        terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Idle,
                None,
                crate::agent_resume::AgentSessionRef::id("session"),
                Some(22),
                now + Duration::from_secs(1),
            )
            .unwrap();
        let observed_at = now + Duration::from_secs(2);
        let mutation = terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            observed_at,
        );
        assert_eq!(terminal.state, AgentState::Idle);
        assert!(
            mutation
                .effective_state_change
                .unwrap()
                .projection_reconfirmed
        );
        assert!(terminal.detector_projection_reconfirmed(epoch, observed_at));
    }

    #[test]
    fn rejected_stale_foreign_and_unvalidated_reports_do_not_dirty_native_window() {
        for (source, agent_label, reference, seq) in [
            ("kitsune:djinn", "djinn", "session", 21),
            ("kitsune:djinn", "djinn", "unvalidated", 22),
            ("kitsune:opencode", "opencode", "foreign", 22),
        ] {
            let (mut terminal, epoch, now) = active_native_terminal();
            terminal
                .set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    AgentState::Idle,
                    None,
                    crate::agent_resume::AgentSessionRef::id("session"),
                    Some(21),
                    now,
                )
                .unwrap();
            terminal.apply_detector_update(epoch, Some(ForegroundObservation::Reset), None, now);
            assert!(terminal
                .set_hook_authority_at(
                    source.into(),
                    agent_label.into(),
                    AgentState::Working,
                    None,
                    crate::agent_resume::AgentSessionRef::id(reference),
                    Some(seq),
                    now + Duration::from_secs(1)
                )
                .is_none());
            assert_eq!(terminal.state, AgentState::Unknown);
            terminal
                .set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    AgentState::Idle,
                    None,
                    crate::agent_resume::AgentSessionRef::id("session"),
                    Some(23),
                    now + Duration::from_secs(2),
                )
                .unwrap();
            let observed_at = now + Duration::from_secs(3);
            let mutation = terminal.apply_detector_update(
                epoch,
                Some(ForegroundObservation::Confirmed {
                    agent: Agent::Djinn,
                    process_group_id: Some(42),
                }),
                None,
                observed_at,
            );
            assert_eq!(terminal.state, AgentState::Idle);
            assert!(
                mutation
                    .effective_state_change
                    .unwrap()
                    .projection_reconfirmed
            );
            assert!(terminal.detector_projection_reconfirmed(epoch, observed_at));
            assert_eq!(
                terminal
                    .current_session_identity_for_persistence()
                    .unwrap()
                    .3,
                "session"
            );
        }
    }

    #[test]
    fn admitted_native_clear_conflict_and_exit_override_newer_unverified_reports() {
        for late_state in [AgentState::Idle, AgentState::Working, AgentState::Blocked] {
            for observation in [
                ForegroundObservation::Cleared,
                ForegroundObservation::Confirmed {
                    agent: Agent::OpenCode,
                    process_group_id: Some(43),
                },
                ForegroundObservation::Exited {
                    agent: Agent::Djinn,
                },
            ] {
                let (mut terminal, epoch, now) = active_native_terminal();
                terminal.apply_detector_update(
                    epoch,
                    Some(ForegroundObservation::Reset),
                    None,
                    now,
                );
                let sampled_at = Instant::now();
                terminal
                    .set_hook_authority_at(
                        "kitsune:djinn".into(),
                        "djinn".into(),
                        late_state,
                        None,
                        crate::agent_resume::AgentSessionRef::id("session"),
                        Some(21),
                        sampled_at + Duration::from_secs(1),
                    )
                    .unwrap();
                assert_eq!(terminal.state, AgentState::Unknown);
                terminal.apply_detector_update(epoch, Some(observation), None, sampled_at);
                assert!(
                    terminal.hook_authority.is_none(),
                    "{observation:?} after {late_state:?}"
                );
                assert!(!terminal.full_lifecycle_hook_authority_active());
                terminal.apply_detector_update(
                    epoch,
                    Some(ForegroundObservation::Confirmed {
                        agent: Agent::Djinn,
                        process_group_id: Some(42),
                    }),
                    None,
                    sampled_at + Duration::from_secs(2),
                );
                assert!(terminal.hook_authority.is_none());
                assert!(terminal
                    .set_hook_authority_at(
                        "kitsune:djinn".into(),
                        "djinn".into(),
                        AgentState::Working,
                        None,
                        crate::agent_resume::AgentSessionRef::id("session"),
                        Some(22),
                        sampled_at + Duration::from_secs(3)
                    )
                    .is_none());
                assert!(terminal.hook_authority.is_none());
            }
        }
    }

    #[test]
    fn newer_genuine_foreground_confirmation_rejects_older_policy_clear() {
        let (mut terminal, epoch, now) = active_native_terminal();
        terminal.apply_detector_update(epoch, Some(ForegroundObservation::Reset), None, now);
        let sampled_at = Instant::now();
        terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Blocked,
                None,
                crate::agent_resume::AgentSessionRef::id("session"),
                Some(21),
                sampled_at + Duration::from_secs(1),
            )
            .unwrap();
        let positive_at = sampled_at + Duration::from_secs(2);
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            positive_at,
        );
        let authority = terminal.hook_authority.clone();
        let mutation = terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Cleared),
            None,
            sampled_at,
        );
        assert_eq!(mutation, TerminalStateMutation::default());
        assert_eq!(terminal.hook_authority, authority);
        assert_eq!(
            terminal.foreground_agent_evidence(),
            Some((Agent::Djinn, positive_at))
        );
        assert_eq!(terminal.state, AgentState::Blocked);
        assert!(terminal.full_lifecycle_hook_authority_active());
    }

    #[test]
    fn admitted_old_probe_preserves_newer_custom_authority_exception() {
        for observation in [
            ForegroundObservation::Cleared,
            ForegroundObservation::Confirmed {
                agent: Agent::OpenCode,
                process_group_id: Some(43),
            },
            ForegroundObservation::Exited {
                agent: Agent::Djinn,
            },
        ] {
            let mut terminal = terminal();
            let epoch = DetectorEpoch::new();
            let now = Instant::now();
            terminal.synchronize_detector_epoch(Some(epoch), now);
            terminal.apply_detector_update(
                epoch,
                Some(ForegroundObservation::Confirmed {
                    agent: Agent::Djinn,
                    process_group_id: Some(42),
                }),
                Some(fallback(Agent::Djinn, AgentState::Idle)),
                now,
            );
            terminal
                .set_hook_authority_at(
                    "custom:djinn".into(),
                    "djinn".into(),
                    AgentState::Working,
                    None,
                    None,
                    Some(20),
                    now,
                )
                .unwrap();
            terminal.apply_detector_update(epoch, Some(ForegroundObservation::Reset), None, now);
            let sampled_at = Instant::now();
            terminal
                .set_hook_authority_at(
                    "custom:djinn".into(),
                    "djinn".into(),
                    AgentState::Blocked,
                    None,
                    None,
                    Some(21),
                    sampled_at + Duration::from_secs(1),
                )
                .unwrap();
            let authority = terminal.hook_authority.clone();
            terminal.apply_detector_update(epoch, Some(observation), None, sampled_at);
            assert_eq!(terminal.hook_authority, authority);
            assert_eq!(terminal.state, AgentState::Blocked);
        }
    }

    #[test]
    fn old_native_clear_or_exit_preserves_replacement_start_received_after_sample() {
        for observation in [
            ForegroundObservation::Cleared,
            ForegroundObservation::Exited {
                agent: Agent::Djinn,
            },
        ] {
            let (mut terminal, epoch, now) = active_native_terminal();
            terminal.apply_detector_update(epoch, Some(ForegroundObservation::Reset), None, now);
            let sampled_at = Instant::now();
            terminal
                .set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    AgentState::Idle,
                    None,
                    crate::agent_resume::AgentSessionRef::id("session"),
                    Some(21),
                    sampled_at + Duration::from_secs(1),
                )
                .unwrap();
            assert!(terminal
                .set_agent_session_ref_for_session_start(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    crate::agent_resume::AgentSessionRef::id("replacement"),
                    Some(22),
                    Some("new".into())
                )
                .is_none());
            terminal.set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Working,
                None,
                crate::agent_resume::AgentSessionRef::id("replacement"),
                Some(23),
                sampled_at + Duration::from_secs(2),
            );
            terminal.apply_detector_update(epoch, Some(observation), None, sampled_at);
            assert!(terminal.hook_authority.is_none());
            terminal.apply_detector_update(
                epoch,
                Some(ForegroundObservation::Confirmed {
                    agent: Agent::Djinn,
                    process_group_id: Some(42),
                }),
                None,
                sampled_at + Duration::from_secs(3),
            );
            assert_eq!(
                terminal
                    .current_session_identity_for_persistence()
                    .unwrap()
                    .3,
                "replacement"
            );
            assert_eq!(terminal.state, AgentState::Working);
            assert!(terminal.full_lifecycle_hook_authority_active());
            assert_eq!(
                terminal.hook_report_sequences.get("kitsune:djinn"),
                Some(&23)
            );
            assert!(terminal
                .set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    AgentState::Idle,
                    None,
                    crate::agent_resume::AgentSessionRef::id("session"),
                    Some(24),
                    sampled_at + Duration::from_secs(4)
                )
                .is_none());
        }
    }

    #[test]
    fn suspended_native_reports_recover_once_without_resend_and_tag_only_unchanged_projection() {
        for replace_epoch in [false, true] {
            for (initial, report) in [
                (AgentState::Idle, None),
                (AgentState::Working, None),
                (AgentState::Idle, Some(AgentState::Idle)),
                (AgentState::Working, Some(AgentState::Working)),
                (AgentState::Working, Some(AgentState::Idle)),
                (AgentState::Idle, Some(AgentState::Working)),
            ] {
                let (mut terminal, mut epoch, now) = active_native_terminal();
                let session_ref = crate::agent_resume::AgentSessionRef::id("session");
                terminal.set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    initial,
                    None,
                    session_ref.clone(),
                    Some(21),
                    now,
                );
                let identity = terminal.current_session_identity_for_persistence();
                if replace_epoch {
                    epoch = DetectorEpoch::new();
                    terminal.synchronize_detector_epoch(Some(epoch), now + Duration::from_secs(1));
                } else {
                    terminal.apply_detector_update(
                        epoch,
                        Some(ForegroundObservation::Reset),
                        None,
                        now + Duration::from_secs(1),
                    );
                }
                if let Some(state) = report {
                    let mutation = terminal
                        .set_hook_authority_at(
                            "kitsune:djinn".into(),
                            "djinn".into(),
                            state,
                            None,
                            session_ref.clone(),
                            Some(22),
                            now + Duration::from_secs(2),
                        )
                        .unwrap();
                    assert!(mutation.effective_state_change.is_none());
                    assert!(!mutation.session_ref_changed);
                    assert_eq!(
                        terminal.hook_report_sequences.get("kitsune:djinn"),
                        Some(&22)
                    );
                    assert!(terminal
                        .set_hook_authority_at(
                            "kitsune:djinn".into(),
                            "djinn".into(),
                            initial,
                            None,
                            session_ref.clone(),
                            Some(21),
                            now + Duration::from_secs(2)
                        )
                        .is_none());
                }
                assert_eq!(terminal.state, AgentState::Unknown);
                assert!(!terminal.full_lifecycle_hook_authority_active());
                assert_eq!(
                    terminal.current_session_identity_for_persistence(),
                    identity
                );
                let observed_at = now + Duration::from_secs(3);
                let mutation = terminal.apply_detector_update(
                    epoch,
                    Some(ForegroundObservation::Confirmed {
                        agent: Agent::Djinn,
                        process_group_id: Some(42),
                    }),
                    None,
                    observed_at,
                );
                let target = report.unwrap_or(initial);
                let change = mutation.effective_state_change.unwrap();
                assert_eq!(change.previous_state, AgentState::Unknown);
                assert_eq!(change.state, target);
                assert_eq!(change.projection_reconfirmed, initial == target);
                assert_eq!(
                    terminal.detector_projection_reconfirmed(epoch, observed_at),
                    initial == target
                );
                assert!(
                    !terminal.detector_projection_reconfirmed(DetectorEpoch::new(), observed_at)
                );
                assert!(!terminal.detector_projection_reconfirmed(epoch, now));
                assert_eq!(terminal.state, target);
                assert!(terminal.full_lifecycle_hook_authority_active());
                assert!(terminal.stale_full_lifecycle_hook_sessions.is_empty());
                assert!(terminal.suppressed_full_lifecycle_hook_reports.is_empty());
                terminal.apply_detector_update(
                    epoch,
                    Some(ForegroundObservation::Inconclusive),
                    None,
                    now + Duration::from_secs(4),
                );
                assert!(!terminal.detector_projection_reconfirmed(epoch, observed_at));
            }
        }
    }

    #[test]
    fn suspended_different_native_owner_start_and_state_promote_without_resend() {
        for report_first in [false, true] {
            let (mut terminal, epoch, now) = active_native_terminal();
            terminal.apply_detector_update(
                epoch,
                Some(ForegroundObservation::Reset),
                None,
                now + Duration::from_secs(1),
            );
            let session = crate::agent_resume::AgentSessionRef::id("opencode-new");
            if report_first {
                terminal.set_hook_authority_at(
                    "kitsune:opencode".into(),
                    "opencode".into(),
                    AgentState::Idle,
                    None,
                    session.clone(),
                    Some(2),
                    now + Duration::from_secs(2),
                );
            }
            assert!(terminal
                .set_agent_session_ref_for_session_start(
                    "kitsune:opencode".into(),
                    "opencode".into(),
                    session.clone(),
                    Some(1),
                    Some("new".into())
                )
                .is_none());
            if !report_first {
                terminal.set_hook_authority_at(
                    "kitsune:opencode".into(),
                    "opencode".into(),
                    AgentState::Idle,
                    None,
                    session.clone(),
                    Some(2),
                    now + Duration::from_secs(2),
                );
            }
            assert_eq!(terminal.state, AgentState::Unknown);
            assert_eq!(
                terminal
                    .current_session_identity_for_persistence()
                    .unwrap()
                    .3,
                "session"
            );
            let mutation = terminal.apply_detector_update(
                epoch,
                Some(ForegroundObservation::Confirmed {
                    agent: Agent::OpenCode,
                    process_group_id: Some(43),
                }),
                None,
                now + Duration::from_secs(3),
            );
            assert_eq!(terminal.state, AgentState::Idle);
            assert_eq!(terminal.effective_agent_label(), Some("opencode"));
            assert!(
                !mutation
                    .effective_state_change
                    .unwrap()
                    .projection_reconfirmed
            );
            assert_eq!(
                terminal
                    .current_session_identity_for_persistence()
                    .unwrap()
                    .3,
                "opencode-new"
            );
            assert_eq!(
                terminal.hook_report_sequences.get("kitsune:opencode"),
                Some(&2)
            );
            assert!(terminal
                .set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    AgentState::Working,
                    None,
                    crate::agent_resume::AgentSessionRef::id("session"),
                    Some(21),
                    now + Duration::from_secs(4)
                )
                .is_none());
        }
    }

    #[test]
    fn suspended_new_same_kind_session_preserves_generation_barriers() {
        let (mut terminal, epoch, now) = active_native_terminal();
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Reset),
            None,
            now + Duration::from_secs(1),
        );
        terminal.set_agent_session_ref_for_session_start(
            "kitsune:djinn".into(),
            "djinn".into(),
            crate::agent_resume::AgentSessionRef::id("replacement"),
            Some(21),
            Some("new".into()),
        );
        terminal.set_hook_authority_at(
            "kitsune:djinn".into(),
            "djinn".into(),
            AgentState::Idle,
            None,
            crate::agent_resume::AgentSessionRef::id("replacement"),
            Some(22),
            now + Duration::from_secs(2),
        );
        assert_eq!(terminal.state, AgentState::Unknown);
        let mutation = terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            now + Duration::from_secs(3),
        );
        assert!(
            !mutation
                .effective_state_change
                .unwrap()
                .projection_reconfirmed
        );
        assert_eq!(terminal.state, AgentState::Idle);
        assert_eq!(
            terminal
                .current_session_identity_for_persistence()
                .unwrap()
                .3,
            "replacement"
        );
        assert!(terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Working,
                None,
                crate::agent_resume::AgentSessionRef::id("session"),
                Some(23),
                now + Duration::from_secs(4)
            )
            .is_none());
    }

    #[test]
    fn unrecognized_reference_candidate_does_not_poison_retained_session() {
        let (mut terminal, epoch, now) = active_native_terminal();
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Reset),
            None,
            now + Duration::from_secs(1),
        );
        assert!(terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Idle,
                None,
                crate::agent_resume::AgentSessionRef::id("unvalidated"),
                Some(21),
                now + Duration::from_secs(2)
            )
            .is_none());
        assert_eq!(
            terminal
                .current_session_identity_for_persistence()
                .unwrap()
                .3,
            "session"
        );
        assert_eq!(terminal.state, AgentState::Unknown);
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            now + Duration::from_secs(3),
        );
        assert!(terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Idle,
                None,
                crate::agent_resume::AgentSessionRef::id("session"),
                Some(22),
                now + Duration::from_secs(4)
            )
            .is_some());
        assert_eq!(terminal.state, AgentState::Idle);
        assert_eq!(
            terminal
                .current_session_identity_for_persistence()
                .unwrap()
                .3,
            "session"
        );
        assert!(terminal.stale_full_lifecycle_hook_sessions.is_empty());
    }

    #[test]
    fn suspended_state_only_report_retains_anchored_reference() {
        let (mut terminal, epoch, now) = active_native_terminal();
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Reset),
            None,
            now + Duration::from_secs(1),
        );
        terminal
            .set_hook_authority_at(
                "kitsune:djinn".into(),
                "djinn".into(),
                AgentState::Idle,
                None,
                None,
                Some(21),
                now + Duration::from_secs(2),
            )
            .unwrap();
        assert_eq!(terminal.state, AgentState::Unknown);
        assert_eq!(
            terminal
                .current_session_identity_for_persistence()
                .unwrap()
                .3,
            "session"
        );
        let mutation = terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            now + Duration::from_secs(3),
        );
        assert_eq!(terminal.state, AgentState::Idle);
        assert!(
            !mutation
                .effective_state_change
                .unwrap()
                .projection_reconfirmed
        );
        assert_eq!(
            terminal
                .current_session_identity_for_persistence()
                .unwrap()
                .3,
            "session"
        );
    }

    #[test]
    fn reset_and_epoch_replacement_suspend_without_retiring_native_session() {
        for replace_epoch in [false, true] {
            let (mut terminal, mut epoch, now) = active_native_terminal();
            let authority = terminal.hook_authority.clone();
            let session = terminal.current_session_identity_for_persistence();
            let sequences = terminal.hook_report_sequences.clone();
            let reset_at = now + Duration::from_secs(1);
            let mutation = if replace_epoch {
                epoch = DetectorEpoch::new();
                terminal.synchronize_detector_epoch(Some(epoch), reset_at)
            } else {
                terminal.apply_detector_update(
                    epoch,
                    Some(ForegroundObservation::Reset),
                    None,
                    reset_at,
                )
            };
            let change = mutation.effective_state_change.unwrap();
            assert_eq!(change.previous_state, AgentState::Working);
            assert_eq!(change.state, AgentState::Unknown);
            assert!(!mutation.agent_released);
            assert!(!mutation.session_ref_changed);
            assert!(!terminal.full_lifecycle_hook_authority_active());
            terminal
                .publish_fallback_observation(fallback(Agent::Djinn, AgentState::Idle), reset_at);
            assert_eq!(terminal.state, AgentState::Unknown);
            assert!(terminal
                .set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    AgentState::Idle,
                    None,
                    crate::agent_resume::AgentSessionRef::id("session"),
                    Some(20),
                    reset_at
                )
                .is_none());
            assert!(terminal
                .set_agent_session_ref_for_session_start(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    crate::agent_resume::AgentSessionRef::id("session"),
                    Some(20),
                    Some("new".into())
                )
                .is_none());
            assert_eq!(terminal.hook_authority, authority);
            assert_eq!(terminal.current_session_identity_for_persistence(), session);
            assert_eq!(terminal.hook_report_sequences, sequences);
            assert!(terminal.suppressed_full_lifecycle_hook_reports.is_empty());
            assert!(terminal.stale_full_lifecycle_hook_sessions.is_empty());
            terminal.apply_detector_update(
                epoch,
                Some(ForegroundObservation::Confirmed {
                    agent: Agent::Djinn,
                    process_group_id: Some(42),
                }),
                None,
                now + Duration::from_secs(2),
            );
            assert_eq!(terminal.state, AgentState::Working);
            assert_eq!(terminal.hook_report_sequences, sequences);
            assert!(terminal
                .set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    AgentState::Blocked,
                    None,
                    crate::agent_resume::AgentSessionRef::id("session"),
                    Some(21),
                    now + Duration::from_secs(3)
                )
                .is_some());
            assert_eq!(terminal.state, AgentState::Blocked);
            assert_eq!(terminal.current_session_identity_for_persistence(), session);
            assert!(terminal.stale_full_lifecycle_hook_sessions.is_empty());
        }
    }

    #[test]
    fn policy_clear_after_reset_still_retires_native_owner() {
        let (mut terminal, epoch, now) = active_native_terminal();
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Reset),
            None,
            now + Duration::from_secs(1),
        );
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Cleared),
            None,
            now + Duration::from_secs(2),
        );
        assert!(terminal.hook_authority.is_none());
        assert!(terminal
            .suppressed_full_lifecycle_hook_reports
            .contains_key("kitsune:djinn"));
        assert_eq!(
            terminal
                .current_session_identity_for_persistence()
                .unwrap()
                .3,
            "session"
        );
    }

    #[test]
    fn release_invalidates_eligibility_even_when_queued_exit_is_older() {
        let mut terminal = terminal();
        let epoch = DetectorEpoch::new();
        let now = Instant::now();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            Some(fallback(Agent::Djinn, AgentState::Idle)),
            now,
        );
        let old_owner = crate::agent_resume::PersistedAgentSession {
            source: "kitsune:claude".into(),
            agent: "claude".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("old-owner").unwrap(),
        };
        terminal.set_persisted_agent_session(old_owner.clone());
        let mutation = terminal
            .release_agent_with_mutation("kitsune:djinn", "djinn", Some(21), None)
            .unwrap();
        assert!(!mutation.agent_released);
        assert_eq!(terminal.detected_agent(), Some(Agent::Djinn));
        assert!(terminal.foreground_agent_evidence().is_none());
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Exited {
                agent: Agent::Djinn,
            }),
            None,
            now,
        );
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            now,
        );
        let incoming = crate::agent_resume::AgentSessionRef::id("new-owner").unwrap();
        assert!(!terminal.foreground_agent_confirms_session_owner(
            "kitsune:djinn",
            "djinn",
            &incoming
        ));
        terminal.publish_fallback_observation(
            FallbackObservation {
                agent: None,
                state: AgentState::Unknown,
                visible_blocker: false,
                visible_working: false,
            },
            now + Duration::from_secs(1),
        );
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Cleared),
            None,
            now + Duration::from_secs(1),
        );
        assert!(terminal
            .set_agent_session_ref_for_session_start(
                "kitsune:djinn".into(),
                "djinn".into(),
                Some(incoming.clone()),
                Some(22),
                Some("new".into())
            )
            .is_none());
        assert_eq!(terminal.persisted_agent_session, Some(old_owner));
        assert!(terminal.foreground_agent_evidence().is_none());
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            None,
            now + Duration::from_secs(2),
        );
        assert!(terminal.foreground_agent_confirms_session_owner(
            "kitsune:djinn",
            "djinn",
            &incoming
        ));
        assert!(terminal
            .set_agent_session_ref_for_session_start(
                "kitsune:djinn".into(),
                "djinn".into(),
                Some(incoming.clone()),
                Some(23),
                Some("new".into())
            )
            .is_some());
        assert_eq!(
            terminal
                .current_session_identity_for_persistence()
                .unwrap()
                .3,
            incoming.value
        );
    }

    #[test]
    fn real_process_exit_publishes_idle_completion_without_positive_proof() {
        let (mut terminal, _, now) = active_native_terminal();
        let mutation =
            terminal.record_process_exit(Some(Agent::Djinn), now + Duration::from_secs(1));
        let change = mutation.effective_state_change.unwrap();
        assert_eq!(change.previous_state, AgentState::Working);
        assert_eq!(change.state, AgentState::Idle);
        assert!(mutation.agent_released);
        assert!(terminal.foreground_agent_evidence().is_none());
        assert!(terminal.hook_authority.is_none());
        assert!(terminal.recent_agent_process_exit.is_some());
    }

    fn terminal() -> TerminalState {
        TerminalState::new(TerminalId::alloc(), "/tmp".into())
    }

    fn fallback(agent: Agent, state: AgentState) -> FallbackObservation {
        FallbackObservation {
            agent: Some(agent),
            state,
            visible_blocker: false,
            visible_working: false,
        }
    }

    fn confirm(terminal: &mut TerminalState, epoch: DetectorEpoch, now: Instant) {
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Claude,
                process_group_id: Some(42),
            }),
            None,
            now,
        );
    }

    #[test]
    fn restored_identity_is_idle_history_without_observation_or_readiness() {
        let mut terminal = terminal();
        terminal.restore_managed_agent("reviewer".into(), Agent::Claude);
        let managed = terminal.managed_agent;
        terminal.seed_restored_agent_identity(Agent::Claude);
        assert_eq!(terminal.historical_agent_identity(), Some(Agent::Claude));
        assert_eq!(terminal.detected_agent(), Some(Agent::Claude));
        assert_eq!(terminal.state, AgentState::Idle);
        assert_eq!(terminal.agent_name.as_deref(), Some("reviewer"));
        assert_eq!(terminal.managed_agent, managed);
        assert_eq!(terminal.fallback_observed_at, None);
        assert_eq!(terminal.foreground_agent_evidence(), None);
        assert!(!terminal.managed_agent_interactive_ready());
    }

    #[test]
    fn restored_identity_waits_for_probe_in_both_report_orders() {
        for (process_first, report_first) in [(false, false), (false, true), (true, false)] {
            let mut terminal = terminal();
            terminal.seed_restored_agent_identity(Agent::Djinn);
            let now = Instant::now();
            let epoch = DetectorEpoch::new();
            terminal.synchronize_detector_epoch(Some(epoch), now);
            let session = crate::agent_resume::AgentSessionRef::id("new-session");
            if process_first {
                terminal.apply_detector_update(
                    epoch,
                    Some(ForegroundObservation::Confirmed {
                        agent: Agent::Djinn,
                        process_group_id: Some(42),
                    }),
                    None,
                    now + Duration::from_secs(1),
                );
            }
            if report_first {
                terminal.set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    AgentState::Working,
                    None,
                    session.clone(),
                    Some(2),
                    now + Duration::from_millis(10),
                );
            }
            terminal.set_agent_session_ref_for_session_start(
                "kitsune:djinn".into(),
                "djinn".into(),
                session.clone(),
                Some(1),
                Some("new".into()),
            );
            if !report_first {
                terminal.set_hook_authority_at(
                    "kitsune:djinn".into(),
                    "djinn".into(),
                    AgentState::Working,
                    None,
                    session,
                    Some(2),
                    now + Duration::from_millis(10),
                );
            }
            if !process_first {
                assert!(!terminal.full_lifecycle_hook_authority_active());
                assert_eq!(terminal.state, AgentState::Idle);
                terminal.apply_detector_update(
                    epoch,
                    Some(ForegroundObservation::Confirmed {
                        agent: Agent::Djinn,
                        process_group_id: Some(42),
                    }),
                    None,
                    now + Duration::from_secs(1),
                );
            }
            assert!(terminal.full_lifecycle_hook_authority_active());
            assert_eq!(terminal.state, AgentState::Working);
            assert_eq!(terminal.foreground_process_group_id(), Some(42));
        }
    }

    #[test]
    fn fallback_only_never_confirms_owner_or_refreshes_probe() {
        let mut terminal = terminal();
        let now = Instant::now();
        let epoch = DetectorEpoch::new();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        terminal.publish_fallback_observation(fallback(Agent::Claude, AgentState::Idle), now);
        let session = crate::agent_resume::AgentSessionRef::id("session").unwrap();
        assert!(!terminal.foreground_agent_confirms_session_owner(
            "kitsune:claude",
            "claude",
            &session
        ));
        confirm(&mut terminal, epoch, now);
        terminal.publish_fallback_observation(
            fallback(Agent::Claude, AgentState::Working),
            now + Duration::from_secs(1),
        );
        assert_eq!(
            terminal.foreground_agent_evidence(),
            Some((Agent::Claude, now))
        );
    }

    #[test]
    fn inconclusive_misses_retain_original_positive_timestamp() {
        let mut terminal = terminal();
        let now = Instant::now();
        let epoch = DetectorEpoch::new();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        confirm(&mut terminal, epoch, now);
        for miss in 1..=6 {
            terminal.apply_detector_update(
                epoch,
                Some(ForegroundObservation::Inconclusive),
                None,
                now + Duration::from_secs(miss),
            );
            assert_eq!(
                terminal.foreground_agent_evidence(),
                Some((Agent::Claude, now))
            );
        }
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Cleared),
            None,
            now + Duration::from_secs(7),
        );
        assert_eq!(terminal.foreground_agent_evidence(), None);
    }

    #[test]
    fn stale_probe_and_exit_cannot_replace_newer_evidence() {
        let mut terminal = terminal();
        let now = Instant::now();
        let epoch = DetectorEpoch::new();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        confirm(&mut terminal, epoch, now + Duration::from_secs(2));
        terminal.record_process_exit(Some(Agent::Claude), now + Duration::from_secs(1));
        assert!(terminal.foreground_agent_evidence().is_some());
        terminal.record_process_exit(Some(Agent::Claude), now + Duration::from_secs(3));
        confirm(&mut terminal, epoch, now + Duration::from_secs(2));
        assert_eq!(terminal.foreground_agent_evidence(), None);
        terminal.publish_fallback_observation(
            fallback(Agent::Claude, AgentState::Idle),
            now + Duration::from_secs(4),
        );
        assert!(terminal.recent_agent_process_exit.is_some());
        assert_eq!(terminal.effective_known_agent(), None);
        confirm(&mut terminal, epoch, now + Duration::from_secs(5));
        assert!(terminal.recent_agent_process_exit.is_none());
    }

    #[test]
    fn reset_and_epoch_replacement_invalidate_without_claiming_exit() {
        let mut terminal = terminal();
        let now = Instant::now();
        let old = DetectorEpoch::new();
        let new = DetectorEpoch::new();
        confirm(&mut terminal, old, now);
        assert_eq!(terminal.foreground_agent_evidence(), None);
        terminal.synchronize_detector_epoch(Some(old), now);
        confirm(&mut terminal, old, now);
        terminal.apply_detector_update(old, Some(ForegroundObservation::Reset), None, now);
        assert_eq!(terminal.foreground_agent_evidence(), None);
        assert!(terminal.recent_agent_process_exit.is_none());
        confirm(&mut terminal, old, now);
        terminal.synchronize_detector_epoch(Some(new), now + Duration::from_secs(1));
        confirm(&mut terminal, old, now + Duration::from_secs(2));
        assert_eq!(terminal.foreground_agent_evidence(), None);
        confirm(&mut terminal, new, now + Duration::from_secs(2));
        assert!(terminal.foreground_agent_evidence().is_some());
        terminal.synchronize_detector_epoch(None, now + Duration::from_secs(3));
        confirm(&mut terminal, new, now + Duration::from_secs(4));
        assert_eq!(terminal.foreground_agent_evidence(), None);
    }

    #[test]
    fn foreground_accepted_while_lifecycle_fallback_is_suppressed() {
        let mut terminal = terminal();
        let now = Instant::now();
        let epoch = DetectorEpoch::new();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(42),
            }),
            Some(fallback(Agent::Djinn, AgentState::Idle)),
            now,
        );
        terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
            source: "kitsune:djinn".into(),
            agent: "djinn".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("session").unwrap(),
        });
        terminal.set_hook_authority_at(
            "kitsune:djinn".into(),
            "djinn".into(),
            AgentState::Working,
            None,
            None,
            None,
            now,
        );
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: Some(99),
            }),
            Some(fallback(Agent::Djinn, AgentState::Blocked)),
            now + Duration::from_secs(1),
        );
        assert_eq!(
            terminal.foreground_agent_evidence(),
            Some((Agent::Djinn, now + Duration::from_secs(1)))
        );
        assert_eq!(terminal.foreground_process_group_id(), Some(99));
        assert_eq!(terminal.fallback_state, AgentState::Idle);
        assert_eq!(terminal.fallback_observed_at, Some(now));
        assert_eq!(terminal.state, AgentState::Working);
    }

    #[test]
    fn restore_and_screen_only_leave_buffered_generation_barriers_unchanged() {
        let mut terminal = terminal();
        let session = crate::agent_resume::AgentSessionRef::id("session");
        terminal.set_agent_session_ref_for_session_start(
            "kitsune:djinn".into(),
            "djinn".into(),
            session.clone(),
            Some(1),
            Some("new".into()),
        );
        let now = Instant::now();
        terminal.set_hook_authority_at(
            "kitsune:djinn".into(),
            "djinn".into(),
            AgentState::Working,
            None,
            session,
            Some(2),
            now,
        );
        let suppressed = terminal.suppressed_full_lifecycle_hook_reports.clone();
        let sequences = terminal.hook_report_sequences.clone();
        terminal.seed_restored_agent_identity(Agent::Djinn);
        assert_eq!(terminal.suppressed_full_lifecycle_hook_reports, suppressed);
        assert_eq!(terminal.hook_report_sequences, sequences);
        terminal.publish_fallback_observation(fallback(Agent::Djinn, AgentState::Idle), now);
        assert_eq!(terminal.suppressed_full_lifecycle_hook_reports, suppressed);
        assert_eq!(terminal.hook_report_sequences, sequences);
        assert!(terminal.hook_authority.is_none());
        assert!(terminal.foreground_agent_evidence().is_none());
    }

    #[test]
    fn restored_identity_does_not_preserve_process_owned_release() {
        let mut terminal = terminal();
        terminal.seed_restored_agent_identity(Agent::Claude);
        terminal.set_agent_name("reviewer".into());
        let session = crate::agent_resume::AgentSessionRef::id("session").unwrap();
        terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
            source: "kitsune:claude".into(),
            agent: "claude".into(),
            session_ref: session.clone(),
        });
        let mutation = terminal
            .release_agent_with_mutation("kitsune:claude", "claude", Some(1), Some(&session))
            .unwrap();
        assert!(mutation.agent_released);
        assert!(terminal.detected_agent().is_none());
        assert!(terminal.agent_name.is_none());
        assert!(terminal.persisted_agent_session.is_none());
    }

    #[test]
    fn stale_probe_cannot_cross_release_barrier() {
        let mut terminal = terminal();
        terminal.seed_restored_agent_identity(Agent::Djinn);
        let epoch = DetectorEpoch::new();
        let now = Instant::now();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        terminal
            .release_agent_with_mutation("kitsune:djinn", "djinn", Some(1), None)
            .unwrap();
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: None,
            }),
            None,
            now,
        );
        assert!(terminal.foreground_agent_evidence().is_none());
        assert!(terminal.detected_agent().is_none());
    }

    #[test]
    fn seed_preserves_persisted_reference_and_presentation_metadata() {
        let mut terminal = terminal();
        let session = crate::agent_resume::PersistedAgentSession {
            source: "kitsune:claude".into(),
            agent: "claude".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("session").unwrap(),
        };
        terminal.set_persisted_agent_session(session.clone());
        terminal.set_manual_label("custom".into());
        terminal.set_agent_name("reviewer".into());
        let identity = terminal.current_session_identity_for_persistence();
        let owner = terminal.agent_name_owner.clone();
        terminal.seed_restored_agent_identity(Agent::Claude);
        assert_eq!(
            terminal.current_session_identity_for_persistence(),
            identity
        );
        assert_eq!(terminal.persisted_agent_session, Some(session));
        assert_eq!(terminal.agent_name_owner, owner);
        assert_eq!(terminal.manual_label.as_deref(), Some("custom"));
        assert!(terminal.fallback_observed_at.is_none());
    }

    #[test]
    fn historical_idle_cannot_bypass_managed_settle_or_effective_state_rules() {
        for state in [AgentState::Idle, AgentState::Blocked, AgentState::Working] {
            let mut terminal = terminal();
            let now = Instant::now();
            terminal.begin_managed_agent(
                "reviewer".into(),
                Agent::Claude,
                now,
                Duration::from_millis(100),
                Duration::from_secs(1),
            );
            terminal.seed_restored_agent_identity(Agent::Claude);
            terminal.reconcile_managed_agent_at(now + Duration::from_millis(100), false);
            assert!(terminal.managed_agent_launch_pending());
            let epoch = DetectorEpoch::new();
            terminal.synchronize_detector_epoch(Some(epoch), now);
            terminal.apply_detector_update(
                epoch,
                Some(ForegroundObservation::Confirmed {
                    agent: Agent::Claude,
                    process_group_id: None,
                }),
                Some(fallback(Agent::Claude, state)),
                now + Duration::from_millis(101),
            );
            terminal.reconcile_managed_agent_at(now + Duration::from_millis(101), false);
            assert_eq!(
                terminal.managed_agent_interactive_ready(),
                state != AgentState::Working
            );
        }
    }

    #[test]
    fn historical_identity_cannot_take_over_another_native_owner() {
        let mut terminal = terminal();
        terminal.seed_restored_agent_identity(Agent::Djinn);
        let old = crate::agent_resume::PersistedAgentSession {
            source: "kitsune:claude".into(),
            agent: "claude".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("old").unwrap(),
        };
        terminal.set_persisted_agent_session(old.clone());
        let incoming = crate::agent_resume::AgentSessionRef::id("new");
        terminal.set_agent_session_ref_for_session_start(
            "kitsune:djinn".into(),
            "djinn".into(),
            incoming.clone(),
            Some(1),
            Some("new".into()),
        );
        terminal.set_hook_authority_with_session_ref(
            "kitsune:djinn".into(),
            "djinn".into(),
            AgentState::Working,
            None,
            incoming,
            Some(2),
        );
        assert_eq!(terminal.persisted_agent_session, Some(old));
        assert!(terminal.hook_authority.is_none());
        assert!(!terminal.foreground_agent_confirms_session_owner(
            "kitsune:djinn",
            "djinn",
            &crate::agent_resume::AgentSessionRef::id("new").unwrap()
        ));
        let now = Instant::now();
        let epoch = DetectorEpoch::new();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Djinn,
                process_group_id: None,
            }),
            None,
            now,
        );
        assert_eq!(terminal.effective_agent_label(), Some("djinn"));
        assert_eq!(terminal.state, AgentState::Working);
    }

    #[test]
    fn older_screen_sample_does_not_overwrite_newer_presentation() {
        let mut terminal = terminal();
        let now = Instant::now();
        let epoch = DetectorEpoch::new();
        terminal.synchronize_detector_epoch(Some(epoch), now);
        terminal.publish_fallback_observation(
            fallback(Agent::Claude, AgentState::Blocked),
            now + Duration::from_secs(2),
        );
        terminal.apply_detector_update(
            epoch,
            Some(ForegroundObservation::Confirmed {
                agent: Agent::Claude,
                process_group_id: None,
            }),
            Some(fallback(Agent::Claude, AgentState::Idle)),
            now + Duration::from_secs(1),
        );
        assert_eq!(
            terminal.foreground_agent_evidence(),
            Some((Agent::Claude, now + Duration::from_secs(1)))
        );
        assert_eq!(terminal.state, AgentState::Blocked);
    }
}
