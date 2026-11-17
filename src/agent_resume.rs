//! Bounded historical session references. Nothing in this module constructs
//! provider commands or schedules execution.
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionRef {
    pub kind: AgentSessionRefKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionRefKind {
    Id,
    Path,
}

/// Already-held legacy intent remains inspectable data, never executable work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResumePlan {
    pub agent: String,
    pub argv: Vec<String>,
    pub dedupe_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedAgentSession {
    pub source: String,
    pub agent: String,
    pub session_ref: AgentSessionRef,
}

impl AgentSessionRef {
    pub fn id(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)).then_some(
            Self {
                kind: AgentSessionRefKind::Id,
                value,
            },
        )
    }

    pub fn path(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty()
            && value.len() <= 4096
            && !value.chars().any(char::is_control)
            && Path::new(&value).is_absolute())
        .then_some(Self {
            kind: AgentSessionRefKind::Path,
            value,
        })
    }
}

pub fn session_ref_from_snapshot(
    source: &str,
    agent: &str,
    kind: AgentSessionRefKind,
    value: &str,
) -> Option<PersistedAgentSession> {
    if !is_official_agent_source(source, agent) {
        return None;
    }
    let session_ref = match (agent, kind) {
        ("pi" | "omp", AgentSessionRefKind::Path) => AgentSessionRef::path(value)?,
        (_, AgentSessionRefKind::Id) => AgentSessionRef::id(value)?,
        _ => return None,
    };
    Some(PersistedAgentSession {
        source: source.into(),
        agent: agent.into(),
        session_ref,
    })
}

pub(crate) fn is_official_agent_source(source: &str, agent: &str) -> bool {
    matches!(
        (source, agent),
        ("kitsune:claude", "claude")
            | ("kitsune:codex", "codex")
            | ("kitsune:copilot", "copilot")
            | ("kitsune:devin", "devin")
            | ("kitsune:djinn", "djinn")
            | ("kitsune:droid", "droid")
            | ("kitsune:kimi", "kimi")
            | ("kitsune:omp", "omp")
            | ("kitsune:mastracode", "mastracode")
            | ("kitsune:pi", "pi")
            | ("kitsune:hermes", "hermes")
            | ("kitsune:opencode", "opencode")
            | ("kitsune:qodercli", "qodercli")
            | ("kitsune:kilo", "kilo")
            | ("kitsune:cursor", "cursor")
            | ("kitsune:grok", "grok")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_session_references_are_bounded_and_source_checked() {
        assert!(AgentSessionRef::id("").is_none());
        assert!(AgentSessionRef::id("bad\nid").is_none());
        assert!(AgentSessionRef::id("x".repeat(513)).is_none());
        assert!(AgentSessionRef::path("relative.jsonl").is_none());
        assert!(AgentSessionRef::path(format!("/{}", "x".repeat(4096))).is_none());
        for (source, agent, kind, value) in [
            ("kitsune:codex", "codex", AgentSessionRefKind::Id, "old-id"),
            (
                "kitsune:pi",
                "pi",
                AgentSessionRefKind::Path,
                "/old/session.jsonl",
            ),
        ] {
            let session = session_ref_from_snapshot(source, agent, kind, value).unwrap();
            assert_eq!(session.session_ref.value, value);
            assert_eq!(session.session_ref.kind, kind);
        }
        assert!(
            session_ref_from_snapshot("custom:codex", "codex", AgentSessionRefKind::Id, "id")
                .is_none()
        );
        assert!(session_ref_from_snapshot(
            "kitsune:codex",
            "codex",
            AgentSessionRefKind::Path,
            "/old"
        )
        .is_none());
    }
}
