use regex::Regex;

use crate::api::schema::{
    legacy_agent_retirement_error, ErrorBody, ErrorResponse, Method, PaneOutputMatchedEvent,
    PaneScrollChangedEvent, PaneScrollInfo, Request, Subscription, SubscriptionEventData,
    SubscriptionEventEnvelope, SubscriptionEventKind,
};
use crate::api::server::{dispatch_to_app_with_timeout, APP_RESPONSE_TIMEOUT};
use crate::api::{ApiRequestSender, EventHub};

pub(super) fn output_match_read_source(
    source: &crate::api::schema::ReadSource,
) -> crate::api::schema::ReadSource {
    match source {
        crate::api::schema::ReadSource::Recent => crate::api::schema::ReadSource::RecentUnwrapped,
        other => *other,
    }
}

pub(super) fn match_output(
    text: &str,
    matcher: &crate::api::schema::OutputMatch,
    regex: Option<&Regex>,
) -> Option<String> {
    match matcher {
        crate::api::schema::OutputMatch::Substring { value } => text
            .lines()
            .find(|line| line.contains(value))
            .map(|line| line.to_string()),
        crate::api::schema::OutputMatch::Regex { .. } => regex.and_then(|re| {
            text.lines()
                .find(|line| re.is_match(line))
                .map(|line| line.to_string())
        }),
    }
}

pub(super) struct ActiveOutputMatchedSubscription {
    pane_id: String,
    source: crate::api::schema::ReadSource,
    lines: Option<u32>,
    matcher: crate::api::schema::OutputMatch,
    regex: Option<Regex>,
    strip_ansi: bool,
    currently_matching: bool,
    request_prefix: String,
}

pub(super) struct ActiveScrollChangedSubscription {
    pane_id: String,
    last_scroll: Option<PaneScrollInfo>,
    request_prefix: String,
}

pub(super) struct ActiveEventSubscription {
    event_kind: crate::api::schema::EventKind,
    last_sequence: u64,
}

pub(super) enum ActiveSubscription {
    Event(ActiveEventSubscription),
    OutputMatched(ActiveOutputMatchedSubscription),
    ScrollChanged(ActiveScrollChangedSubscription),
}

impl ActiveSubscription {
    pub(super) fn new(
        subscription: Subscription,
        request_id: &str,
        index: usize,
        api_tx: &ApiRequestSender,
        _event_hub: &EventHub,
    ) -> Result<Self, ErrorResponse> {
        match subscription {
            Subscription::WorkspaceCreated {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorkspaceCreated,
                last_sequence: 0,
            })),
            Subscription::WorkspaceUpdated {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorkspaceUpdated,
                last_sequence: 0,
            })),
            Subscription::WorkspaceMetadataUpdated {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorkspaceMetadataUpdated,
                last_sequence: 0,
            })),
            Subscription::WorkspaceRenamed {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorkspaceRenamed,
                last_sequence: 0,
            })),
            Subscription::WorkspaceMoved {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorkspaceMoved,
                last_sequence: 0,
            })),
            Subscription::WorkspaceClosed {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorkspaceClosed,
                last_sequence: 0,
            })),
            Subscription::WorkspaceFocused {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorkspaceFocused,
                last_sequence: 0,
            })),
            Subscription::WorktreeCreated {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorktreeCreated,
                last_sequence: 0,
            })),
            Subscription::WorktreeOpened {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorktreeOpened,
                last_sequence: 0,
            })),
            Subscription::WorktreeRemoved {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::WorktreeRemoved,
                last_sequence: 0,
            })),
            Subscription::TabCreated {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::TabCreated,
                last_sequence: 0,
            })),
            Subscription::TabClosed {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::TabClosed,
                last_sequence: 0,
            })),
            Subscription::TabFocused {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::TabFocused,
                last_sequence: 0,
            })),
            Subscription::TabRenamed {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::TabRenamed,
                last_sequence: 0,
            })),
            Subscription::TabMoved {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::TabMoved,
                last_sequence: 0,
            })),
            Subscription::PaneCreated {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::PaneCreated,
                last_sequence: 0,
            })),
            Subscription::PaneClosed {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::PaneClosed,
                last_sequence: 0,
            })),
            Subscription::PaneUpdated {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::PaneUpdated,
                last_sequence: 0,
            })),
            Subscription::PaneFocused {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::PaneFocused,
                last_sequence: 0,
            })),
            Subscription::PaneMoved {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::PaneMoved,
                last_sequence: 0,
            })),
            Subscription::PaneExited {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::PaneExited,
                last_sequence: 0,
            })),
            Subscription::PaneAgentDetected {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::PaneAgentDetected,
                last_sequence: 0,
            })),
            Subscription::LayoutUpdated {} => Ok(Self::Event(ActiveEventSubscription {
                event_kind: crate::api::schema::EventKind::LayoutUpdated,
                last_sequence: 0,
            })),
            Subscription::PaneOutputMatched {
                pane_id,
                source,
                lines,
                r#match,
                strip_ansi,
            } => {
                let regex = match &r#match {
                    crate::api::schema::OutputMatch::Regex { value } => match Regex::new(value) {
                        Ok(regex) => Some(regex),
                        Err(err) => {
                            return Err(ErrorResponse {
                                id: request_id.to_string(),
                                error: ErrorBody {
                                    code: "invalid_regex".into(),
                                    message: err.to_string(),
                                },
                            });
                        }
                    },
                    crate::api::schema::OutputMatch::Substring { .. } => None,
                };

                let probe = pane_read(
                    format!("{request_id}:sub:{index}:probe"),
                    &pane_id,
                    source,
                    lines,
                    strip_ansi,
                    api_tx,
                );
                probe?;

                Ok(Self::OutputMatched(ActiveOutputMatchedSubscription {
                    pane_id,
                    source,
                    lines,
                    matcher: r#match,
                    regex,
                    strip_ansi,
                    currently_matching: false,
                    request_prefix: format!("{request_id}:sub:{index}"),
                }))
            }
            Subscription::PaneAgentStatusChanged { .. } => Err(ErrorResponse {
                id: request_id.into(),
                error: legacy_agent_retirement_error("events.subscribe"),
            }),
            Subscription::PaneScrollChanged { pane_id } => {
                let probe = pane_get(format!("{request_id}:sub:{index}:probe"), &pane_id, api_tx)?;

                Ok(Self::ScrollChanged(ActiveScrollChangedSubscription {
                    pane_id: probe.pane_id,
                    last_scroll: probe.scroll,
                    request_prefix: format!("{request_id}:sub:{index}"),
                }))
            }
        }
    }

    pub(super) fn poll(
        &mut self,
        api_tx: &ApiRequestSender,
        event_hub: &EventHub,
    ) -> Option<serde_json::Value> {
        match self {
            Self::Event(subscription) => subscription.poll(event_hub),
            Self::OutputMatched(subscription) => {
                serde_json::to_value(subscription.poll(api_tx)?).ok()
            }
            Self::ScrollChanged(subscription) => {
                serde_json::to_value(subscription.poll(api_tx)?).ok()
            }
        }
    }
}

impl ActiveEventSubscription {
    fn poll(&mut self, event_hub: &EventHub) -> Option<serde_json::Value> {
        for (sequence, event) in event_hub.events_after(self.last_sequence) {
            self.last_sequence = sequence;
            if event.event == self.event_kind {
                return serde_json::to_value(event).ok();
            }
        }
        None
    }
}

impl ActiveOutputMatchedSubscription {
    fn poll(&mut self, api_tx: &ApiRequestSender) -> Option<SubscriptionEventEnvelope> {
        let read = pane_read(
            format!("{}:read", self.request_prefix),
            &self.pane_id,
            output_match_read_source(&self.source),
            self.lines,
            self.strip_ansi,
            api_tx,
        )
        .ok()?;

        let matched_line = match_output(&read.text, &self.matcher, self.regex.as_ref());
        match matched_line {
            Some(matched_line) => {
                if self.currently_matching {
                    return None;
                }
                self.currently_matching = true;
                Some(SubscriptionEventEnvelope {
                    event: SubscriptionEventKind::PaneOutputMatched,
                    data: SubscriptionEventData::PaneOutputMatched(PaneOutputMatchedEvent {
                        pane_id: read.pane_id.clone(),
                        matched_line,
                        read,
                    }),
                })
            }
            None => {
                self.currently_matching = false;
                None
            }
        }
    }
}

impl ActiveScrollChangedSubscription {
    fn poll(&mut self, api_tx: &ApiRequestSender) -> Option<SubscriptionEventEnvelope> {
        let pane = pane_get(
            format!("{}:pane", self.request_prefix),
            &self.pane_id,
            api_tx,
        )
        .ok()?;
        self.event_from_snapshot(pane)
    }

    fn event_from_snapshot(
        &mut self,
        pane: crate::api::schema::PaneInfo,
    ) -> Option<SubscriptionEventEnvelope> {
        let scroll = pane.scroll;
        if self.last_scroll == scroll {
            return None;
        }
        self.last_scroll = scroll;
        let scroll = scroll?;

        Some(SubscriptionEventEnvelope {
            event: SubscriptionEventKind::ScrollChanged,
            data: SubscriptionEventData::ScrollChanged(PaneScrollChangedEvent {
                pane_id: pane.pane_id,
                workspace_id: pane.workspace_id,
                scroll,
            }),
        })
    }
}

fn pane_read(
    request_id: String,
    pane_id: &str,
    source: crate::api::schema::ReadSource,
    lines: Option<u32>,
    strip_ansi: bool,
    api_tx: &ApiRequestSender,
) -> Result<crate::api::schema::PaneReadResult, ErrorResponse> {
    let response = dispatch_to_app_with_timeout(
        Request {
            id: request_id.clone(),
            method: Method::PaneRead(crate::api::schema::PaneReadParams {
                pane_id: pane_id.to_string(),
                source,
                lines,
                format: crate::api::schema::ReadFormat::Text,
                strip_ansi,
            }),
        },
        api_tx,
        Some(APP_RESPONSE_TIMEOUT),
    );
    let value: serde_json::Value = serde_json::from_str(&response).map_err(|_| ErrorResponse {
        id: request_id.clone(),
        error: ErrorBody {
            code: "internal_error".into(),
            message: "failed to decode pane read response".into(),
        },
    })?;
    if value.get("error").is_some() {
        return serde_json::from_value(value).map_err(|_| ErrorResponse {
            id: request_id,
            error: ErrorBody {
                code: "internal_error".into(),
                message: "failed to decode pane read error".into(),
            },
        });
    }
    serde_json::from_value(value["result"]["read"].clone()).map_err(|_| ErrorResponse {
        id: request_id,
        error: ErrorBody {
            code: "internal_error".into(),
            message: "failed to decode pane read result".into(),
        },
    })
}

fn pane_get(
    request_id: String,
    pane_id: &str,
    api_tx: &ApiRequestSender,
) -> Result<crate::api::schema::PaneInfo, ErrorResponse> {
    let response = dispatch_to_app_with_timeout(
        Request {
            id: request_id.clone(),
            method: Method::PaneGet(crate::api::schema::PaneTarget {
                pane_id: pane_id.to_string(),
            }),
        },
        api_tx,
        Some(APP_RESPONSE_TIMEOUT),
    );
    let value: serde_json::Value = serde_json::from_str(&response).map_err(|_| ErrorResponse {
        id: request_id.clone(),
        error: ErrorBody {
            code: "internal_error".into(),
            message: "failed to decode pane get response".into(),
        },
    })?;
    if value.get("error").is_some() {
        let response =
            serde_json::from_value::<ErrorResponse>(value).map_err(|_| ErrorResponse {
                id: request_id,
                error: ErrorBody {
                    code: "internal_error".into(),
                    message: "failed to decode pane get error".into(),
                },
            })?;
        return Err(response);
    }
    serde_json::from_value(value["result"]["pane"].clone()).map_err(|_| ErrorResponse {
        id: request_id,
        error: ErrorBody {
            code: "internal_error".into(),
            message: "failed to decode pane get result".into(),
        },
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::api::schema::{AgentStatus, EventKind, PaneInfo};

    fn pane_info_with_scroll(scroll: Option<PaneScrollInfo>) -> PaneInfo {
        PaneInfo {
            pane_id: "pane_1".into(),
            terminal_id: "terminal_1".into(),
            workspace_id: "workspace_1".into(),
            tab_id: "tab_1".into(),
            focused: true,
            cwd: None,
            foreground_cwd: None,
            label: None,
            agent: None,
            title: None,
            terminal_title: None,
            terminal_title_stripped: None,
            display_agent: None,
            agent_status: AgentStatus::Unknown,
            state_labels: HashMap::new(),
            tokens: HashMap::new(),
            agent_session: None,
            scroll,
            revision: 0,
        }
    }

    #[test]
    fn workspace_metadata_subscription_uses_dedicated_event_kind() {
        let event_hub = EventHub::default();
        let (api_tx, _api_rx) = tokio::sync::mpsc::unbounded_channel();
        let subscription = ActiveSubscription::new(
            Subscription::WorkspaceMetadataUpdated {},
            "test",
            0,
            &api_tx,
            &event_hub,
        )
        .expect("workspace metadata subscription");

        assert!(matches!(
            subscription,
            ActiveSubscription::Event(ActiveEventSubscription {
                event_kind: EventKind::WorkspaceMetadataUpdated,
                ..
            })
        ));
    }

    #[test]
    fn scroll_subscription_emits_when_scroll_snapshot_changes() {
        let at_bottom = PaneScrollInfo {
            offset_from_bottom: 0,
            max_offset_from_bottom: 40,
            viewport_rows: 20,
        };
        let scrolled_back = PaneScrollInfo {
            offset_from_bottom: 8,
            max_offset_from_bottom: 40,
            viewport_rows: 20,
        };
        let mut subscription = ActiveScrollChangedSubscription {
            pane_id: "pane_1".into(),
            last_scroll: Some(at_bottom),
            request_prefix: "test".into(),
        };

        assert!(subscription
            .event_from_snapshot(pane_info_with_scroll(Some(at_bottom)))
            .is_none());

        let event = subscription
            .event_from_snapshot(pane_info_with_scroll(Some(scrolled_back)))
            .expect("scroll event");
        assert_eq!(event.event, SubscriptionEventKind::ScrollChanged);
        let SubscriptionEventData::ScrollChanged(data) = event.data else {
            panic!("wrong event data");
        };
        assert_eq!(data.pane_id, "pane_1");
        assert_eq!(data.workspace_id, "workspace_1");
        assert_eq!(data.scroll, scrolled_back);
    }

    #[test]
    fn generic_output_subscription_still_probes_and_emits() {
        let (api_tx, mut api_rx) =
            tokio::sync::mpsc::unbounded_channel::<crate::api::ApiRequestMessage>();
        let responder = std::thread::spawn(move || {
            for source in [
                crate::api::schema::ReadSource::Recent,
                crate::api::schema::ReadSource::RecentUnwrapped,
            ] {
                let message = api_rx.blocking_recv().unwrap();
                let Method::PaneRead(params) = message.request.method else {
                    panic!("non-generic probe")
                };
                assert_eq!(params.source, source);
                message.respond_to.send(serde_json::json!({"id":message.request.id, "result":{"type":"pane_read", "read":{
                    "pane_id":"w1:p1", "workspace_id":"w1", "tab_id":"w1:t1", "source":source,
                    "format":"text", "text":"generic ready\n", "revision":1, "truncated":false,
                }}}).to_string()).unwrap();
            }
        });
        let mut active = ActiveSubscription::new(
            Subscription::PaneOutputMatched {
                pane_id: "w1:p1".into(),
                source: crate::api::schema::ReadSource::Recent,
                lines: None,
                r#match: crate::api::schema::OutputMatch::Substring {
                    value: "ready".into(),
                },
                strip_ansi: true,
            },
            "output",
            0,
            &api_tx,
            &EventHub::default(),
        )
        .unwrap();
        let event = active.poll(&api_tx, &EventHub::default()).unwrap();
        drop(api_tx);
        responder.join().unwrap();
        assert_eq!(event["event"], "pane.output_matched");
        assert_eq!(event["data"]["matched_line"], "generic ready");
    }

    #[test]
    fn retired_agent_status_subscription_is_inert_for_every_filter() {
        for status in [
            None,
            Some(AgentStatus::Unknown),
            Some(AgentStatus::Working),
            Some(AgentStatus::Idle),
            Some(AgentStatus::Done),
            Some(AgentStatus::Blocked),
        ] {
            let hub = EventHub::default();
            let (api_tx, mut api_rx) = tokio::sync::mpsc::unbounded_channel();
            let result = ActiveSubscription::new(
                Subscription::PaneAgentStatusChanged {
                    pane_id: "private-target".into(),
                    agent_status: status,
                },
                "correlated",
                0,
                &api_tx,
                &hub,
            );
            let Err(response) = result else {
                panic!("retired subscription became active")
            };
            assert_eq!(response.id, "correlated");
            assert_eq!(
                response.error,
                legacy_agent_retirement_error("events.subscribe")
            );
            assert!(api_rx.try_recv().is_err(), "no pane probe");
            assert_eq!(hub.current_sequence(), 0);
        }
    }
}
