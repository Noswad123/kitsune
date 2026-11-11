//! Internal detector provenance. Never serialized or a native process-generation ID.
use crate::detect::{Agent, AgentState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Server-local runtime incarnation; queued observations cannot adopt a new runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DetectorEpoch {
    value: u64,
    started_at: Instant,
}

impl DetectorEpoch {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let value = NEXT.fetch_add(1, Ordering::Relaxed);
        assert_ne!(value, 0, "detector runtime incarnation space exhausted");
        Self {
            value,
            started_at: Instant::now(),
        }
    }

    /// Observation admission starts at runtime creation, not event delivery.
    pub(crate) fn started_at(self) -> Instant {
        self.started_at
    }
}

/// A result of existing foreground-selection policy, including legacy hints.
/// Confirmation is retained under that policy, not continuous liveness or identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForegroundObservation {
    Confirmed {
        agent: Agent,
        process_group_id: Option<u32>,
    },
    /// A probe made no positive finding. This does not refresh prior confirmation.
    Inconclusive,
    /// The existing absence-confirmation policy reached its clearing threshold.
    Cleared,
    Exited {
        agent: Agent,
    },
    /// Detector reset/replacement invalidates evidence without claiming process exit.
    Reset,
}

/// Presentation input only. It cannot manufacture foreground or exit evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FallbackObservation {
    pub(crate) agent: Option<Agent>,
    pub(crate) state: AgentState,
    pub(crate) visible_blocker: bool,
    pub(crate) visible_working: bool,
}
