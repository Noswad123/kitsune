use std::collections::HashMap;

use super::TerminalState;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EffectivePresentation {
    pub title: Option<String>,
    pub display_agent: Option<String>,
    pub state_labels: HashMap<String, String>,
}

impl TerminalState {
    pub(crate) fn metadata_report_sequence_is_fresh(&self, source: &str, seq: Option<u64>) -> bool {
        crate::metadata_tokens::sequence_is_fresh(&self.metadata_report_sequences, source, seq)
    }

    pub(crate) fn accept_metadata_report(
        &mut self,
        source: &str,
        seq: Option<u64>,
    ) -> Result<bool, ()> {
        crate::metadata_tokens::accept_sequence(&mut self.metadata_report_sequences, source, seq)
    }

    pub fn effective_presentation(&self) -> EffectivePresentation {
        Default::default()
    }
    pub fn effective_title(&self) -> Option<String> {
        None
    }
    pub fn effective_display_agent(&self) -> Option<String> {
        None
    }
}
