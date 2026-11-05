//! Ephemeral, source-ordered annotations for server-owned terminal identities.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use unicode_width::UnicodeWidthStr;

use crate::terminal::TerminalId;

const MAX_SOURCES: usize = 8;
const MAX_TARGETS: usize = 64;
const MAX_ID_BYTES: usize = 64;
const MAX_TEXT_BYTES: usize = 256;
const MIN_TTL: Duration = Duration::from_millis(1);
const MAX_TTL: Duration = Duration::from_secs(300);
pub(crate) const MAX_VALID_UNTIL_UNIX_MS: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ResourceTarget {
    Terminal(TerminalId),
    #[cfg(test)]
    Workspace(String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Error {
    InvalidSources,
    UnselectedSource,
    StaleEpoch,
    #[cfg(test)]
    InvalidTarget,
    InvalidText,
    InvalidTtl,
    DeadlineOverflow,
    InvalidDeadline,
    Expired,
    ClockUnavailable,
    StaleRevision,
    TimeRegression,
    Capacity,
}

struct Record {
    revision: u64,
    accepted_at: Instant,
    deadline: Instant,
    text: Option<String>,
}

struct AnnotationStore {
    sources: Vec<String>,
    targets: HashMap<ResourceTarget, HashMap<usize, Record>>,
}

pub(crate) fn validate_sources(sources: &[String]) -> Result<(), Error> {
    if sources.len() > MAX_SOURCES
        || sources
            .iter()
            .enumerate()
            .any(|(index, source)| !valid_id(source) || sources[..index].contains(source))
    {
        return Err(Error::InvalidSources);
    }
    Ok(())
}

pub(crate) struct AnnotationState {
    epoch: String,
    store: AnnotationStore,
}

impl Default for AnnotationState {
    fn default() -> Self {
        Self {
            epoch: TerminalId::alloc().to_string(),
            store: AnnotationStore {
                sources: Vec::new(),
                targets: HashMap::new(),
            },
        }
    }
}

impl AnnotationState {
    pub(crate) fn new(sources: Vec<String>) -> Result<Self, Error> {
        Ok(Self {
            epoch: TerminalId::alloc().to_string(),
            store: AnnotationStore::new(sources)?,
        })
    }

    pub(crate) fn sources(&self) -> &[String] {
        &self.store.sources
    }
    pub(crate) fn epoch(&self) -> &str {
        &self.epoch
    }

    pub(crate) fn reconfigure(&mut self, sources: Vec<String>) -> Result<bool, Error> {
        validate_sources(&sources)?;
        if self.sources() == sources {
            return Ok(false);
        }
        *self = Self::new(sources)?;
        Ok(true)
    }

    pub(crate) fn replace(
        &mut self,
        epoch: &str,
        source: &str,
        terminal_id: TerminalId,
        revision: u64,
        text: Option<String>,
        ttl: Duration,
        now: Instant,
    ) -> Result<(), Error> {
        if epoch != self.epoch {
            return Err(Error::StaleEpoch);
        }
        self.store.replace(
            source,
            ResourceTarget::Terminal(terminal_id),
            revision,
            text,
            ttl,
            now,
        )
    }

    pub(crate) fn project(&self, terminal_id: &TerminalId, now: Instant) -> Option<&str> {
        self.store
            .project(&ResourceTarget::Terminal(terminal_id.clone()), &now)
    }

    /// Evaluate timing only after validating the pair, without acquiring clocks here.
    pub(crate) fn replace_with_timing(
        &mut self,
        epoch: &str,
        source: &str,
        terminal_id: TerminalId,
        revision: u64,
        text: Option<String>,
        ttl: Duration,
        timing: impl FnOnce() -> Result<(Instant, Instant), Error>,
    ) -> Result<(), Error> {
        if epoch != self.epoch {
            return Err(Error::StaleEpoch);
        }
        self.store.replace_with_timing(
            source,
            ResourceTarget::Terminal(terminal_id),
            revision,
            text,
            ttl,
            timing,
        )
    }

    pub(crate) fn next_deadline(&self, now: Instant) -> Option<Instant> {
        self.store
            .targets
            .values()
            .filter_map(|rows| {
                let winner = (0..self.store.sources.len()).find_map(|source| rows.get(&source))?;
                (winner.text.is_some() && winner.accepted_at <= now && now < winner.deadline)
                    .then_some(winner.deadline)
            })
            .min()
    }

    pub(crate) fn retain_terminals(&mut self, mut retain: impl FnMut(&TerminalId) -> bool) {
        self.store.targets.retain(|target, _| match target {
            ResourceTarget::Terminal(id) => retain(id),
            #[cfg(test)]
            ResourceTarget::Workspace(_) => false,
        });
    }
}

fn valid_id(id: &str) -> bool {
    (1..=MAX_ID_BYTES).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

impl AnnotationStore {
    fn new(sources: Vec<String>) -> Result<Self, Error> {
        validate_sources(&sources)?;
        Ok(Self {
            sources,
            targets: HashMap::new(),
        })
    }

    fn replace(
        &mut self,
        source: &str,
        target: ResourceTarget,
        revision: u64,
        text: Option<String>,
        ttl: Duration,
        now: Instant,
    ) -> Result<(), Error> {
        self.replace_with_timing(source, target, revision, text, ttl, || {
            let deadline = now.checked_add(ttl).ok_or(Error::DeadlineOverflow)?;
            Ok((now, deadline))
        })
    }

    fn replace_with_timing(
        &mut self,
        source: &str,
        target: ResourceTarget,
        revision: u64,
        text: Option<String>,
        ttl: Duration,
        timing: impl FnOnce() -> Result<(Instant, Instant), Error>,
    ) -> Result<(), Error> {
        let source = self
            .sources
            .iter()
            .position(|selected| selected == source)
            .ok_or(Error::UnselectedSource)?;
        #[cfg(test)]
        if matches!(&target, ResourceTarget::Workspace(id) if !valid_id(id)) {
            return Err(Error::InvalidTarget);
        }
        if text
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > MAX_TEXT_BYTES)
        {
            return Err(Error::InvalidText);
        }
        if !(MIN_TTL..=MAX_TTL).contains(&ttl) {
            return Err(Error::InvalidTtl);
        }
        if let Some(record) = self.targets.get(&target).and_then(|rows| rows.get(&source)) {
            if revision <= record.revision {
                return Err(Error::StaleRevision);
            }
        }
        if !self.targets.contains_key(&target) && self.targets.len() >= MAX_TARGETS {
            return Err(Error::Capacity);
        }
        // Timing supplies an acceptance instant and an already absolute expiry.
        // Never restart a mapped residual lifetime from this insertion time.
        let (now, deadline) = timing()?;
        if self
            .targets
            .get(&target)
            .and_then(|rows| rows.get(&source))
            .is_some_and(|record| now < record.accepted_at)
        {
            return Err(Error::TimeRegression);
        }
        if now >= deadline {
            return Err(Error::Expired);
        }
        // Retain cleared/expired records as bounded priority and revision watermarks.
        self.targets.entry(target).or_default().insert(
            source,
            Record {
                revision,
                accepted_at: now,
                deadline,
                text,
            },
        );
        Ok(())
    }

    fn project(&self, target: &ResourceTarget, now: &Instant) -> Option<&str> {
        let rows = self.targets.get(target)?;
        let winner = (0..self.sources.len()).find_map(|source| rows.get(&source))?;
        if *now < winner.accepted_at || *now >= winner.deadline {
            return None;
        }
        winner.text.as_deref()
    }
}

pub(crate) fn render_text(text: &str, maxcells: usize) -> String {
    if maxcells == 0 {
        return String::new();
    }
    let mut output = String::new();
    for ch in text.chars() {
        if ch.is_control()
            || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            output.extend(ch.escape_unicode());
        } else {
            output.push(ch);
        }
    }
    if output.width() > maxcells {
        if maxcells == 1 {
            return "\u{2026}".into();
        }
        output.push('\u{2026}');
        // Measure the actual candidate, including emoji join/variation effects.
        // Scalar boundaries preserve UTF-8, not necessarily full graphemes.
        while output.width() > maxcells {
            output.pop();
            output.pop();
            output.push('\u{2026}');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect, widgets::Paragraph, widgets::Widget};

    fn store() -> AnnotationStore {
        AnnotationStore::new(vec!["build".into(), "tests".into()]).unwrap()
    }

    #[test]
    fn production_epochs_reconfigure_atomically_and_restart_fresh() {
        let now = Instant::now();
        let terminal = TerminalId::alloc();
        let mut state = AnnotationState::default();
        assert!(state.sources().is_empty());
        let disabled_epoch = state.epoch().to_owned();
        assert_eq!(
            state.replace(
                &disabled_epoch,
                "build",
                terminal.clone(),
                0,
                None,
                MIN_TTL,
                now
            ),
            Err(Error::UnselectedSource)
        );
        assert!(state
            .reconfigure(vec!["build".into(), "tests".into()])
            .unwrap());
        let epoch = state.epoch().to_owned();
        assert_ne!(epoch, disabled_epoch);
        state
            .replace(
                &epoch,
                "build",
                terminal.clone(),
                1,
                Some("ready".into()),
                MIN_TTL,
                now,
            )
            .unwrap();
        assert!(!state.reconfigure(state.sources().to_vec()).unwrap());
        assert_eq!(state.epoch(), epoch);
        assert_eq!(state.project(&terminal, now), Some("ready"));
        assert_eq!(
            state.reconfigure(vec!["build".into(), "build".into()]),
            Err(Error::InvalidSources)
        );
        assert_eq!(state.epoch(), epoch);
        assert_eq!(state.project(&terminal, now), Some("ready"));
        assert!(state
            .reconfigure(vec!["tests".into(), "build".into()])
            .unwrap());
        assert_ne!(state.epoch(), epoch);
        assert_eq!(state.project(&terminal, now), None);
        assert_eq!(
            state.replace(&epoch, "build", terminal.clone(), 2, None, MIN_TTL, now),
            Err(Error::StaleEpoch)
        );
        let restarted = AnnotationState::new(state.sources().to_vec()).unwrap();
        assert_ne!(restarted.epoch(), state.epoch());
        assert_eq!(restarted.project(&terminal, now), None);
    }

    #[test]
    fn annotation_absolute_timing_rejection_preserves_every_pair_field() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        put(&mut store, "build", &target, 1, Some("old"), now).unwrap();
        for timing in [
            Err(Error::Expired),
            Err(Error::InvalidDeadline),
            Err(Error::ClockUnavailable),
            Ok((now + MIN_TTL, now + MIN_TTL)),
            Ok((now + MIN_TTL, now)),
            Ok((now - MIN_TTL, now + MIN_TTL)),
        ] {
            assert!(store
                .replace_with_timing("build", target.clone(), 2, None, MAX_TTL, || timing)
                .is_err());
            let record = &store.targets[&target][&0];
            assert_eq!(record.revision, 1);
            assert_eq!(record.accepted_at, now);
            assert_eq!(record.deadline, now + MAX_TTL);
            assert_eq!(record.text.as_deref(), Some("old"));
            assert_eq!(store.targets.len(), 1);
            assert_eq!(store.targets[&target].len(), 1);
        }
        // Rejection consumes neither a revision nor priority/capacity on an absent pair.
        let other = workspace();
        put(&mut store, "tests", &other, 1, Some("lower"), now).unwrap();
        assert_eq!(
            store.replace_with_timing("build", other.clone(), 2, None, MAX_TTL, || {
                Err(Error::Expired)
            }),
            Err(Error::Expired)
        );
        assert_eq!(store.project(&other, &now), Some("lower"));
        let absent = workspace();
        assert_eq!(
            store.replace_with_timing("build", absent.clone(), 2, None, MAX_TTL, || {
                Err(Error::Expired)
            }),
            Err(Error::Expired)
        );
        assert!(!store.targets.contains_key(&absent));
        store
            .replace_with_timing(
                "build",
                target.clone(),
                2,
                Some("new".into()),
                MAX_TTL,
                || Ok((now + MIN_TTL, now + MIN_TTL * 2)),
            )
            .unwrap();
        assert_eq!(store.project(&target, &(now + MIN_TTL)), Some("new"));
        assert_eq!(store.project(&target, &(now + MIN_TTL * 2)), None);
    }

    #[test]
    fn annotation_absolute_timing_runs_after_store_validation_and_keeps_quotas() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        put(&mut store, "build", &target, 1, Some("old"), now).unwrap();
        for (source, revision, text, ttl, error) in [
            ("other", 2, None, MAX_TTL, Error::UnselectedSource),
            ("build", 1, None, MAX_TTL, Error::StaleRevision),
            ("build", 2, Some(String::new()), MAX_TTL, Error::InvalidText),
            ("build", 2, None, Duration::ZERO, Error::InvalidTtl),
            ("build", 2, None, MAX_TTL + MIN_TTL, Error::InvalidTtl),
        ] {
            assert_eq!(
                store.replace_with_timing(source, target.clone(), revision, text, ttl, || {
                    panic!("timing must not run before store fences")
                }),
                Err(error)
            );
        }
        for _ in 1..MAX_TARGETS {
            put(&mut store, "build", &workspace(), 1, None, now).unwrap();
        }
        assert_eq!(
            store.replace_with_timing("build", workspace(), 2, None, MAX_TTL, || {
                panic!("timing must not run before quota check")
            }),
            Err(Error::Capacity)
        );
        // Existing target updates at capacity still retain absolute expiry and watermarks.
        store
            .replace_with_timing("build", target.clone(), 2, None, MAX_TTL, || {
                Ok((now, now + MIN_TTL))
            })
            .unwrap();
        assert_eq!(store.targets.len(), MAX_TARGETS);
        assert_eq!(store.project(&target, &(now + MIN_TTL)), None);
        assert_eq!(
            put(
                &mut store,
                "build",
                &target,
                2,
                Some("replay"),
                now + MIN_TTL
            ),
            Err(Error::StaleRevision)
        );
    }

    #[test]
    fn production_deadlines_only_track_visible_winners() {
        let now = Instant::now();
        let terminal = TerminalId::alloc();
        let mut state = AnnotationState::new(vec!["build".into(), "tests".into()]).unwrap();
        let epoch = state.epoch().to_owned();
        state
            .replace(
                &epoch,
                "tests",
                terminal.clone(),
                1,
                Some("lower".into()),
                MAX_TTL,
                now,
            )
            .unwrap();
        assert_eq!(state.next_deadline(now), Some(now + MAX_TTL));
        state
            .replace(
                &epoch,
                "build",
                terminal.clone(),
                1,
                Some("higher".into()),
                MIN_TTL,
                now,
            )
            .unwrap();
        assert_eq!(state.next_deadline(now), Some(now + MIN_TTL));
        assert_eq!(state.next_deadline(now + MIN_TTL), None);
        assert_eq!(state.project(&terminal, now + MIN_TTL), None);
        state
            .replace(&epoch, "build", terminal, 2, None, MAX_TTL, now)
            .unwrap();
        assert_eq!(state.next_deadline(now), None);
    }

    #[test]
    fn production_live_tombstones_are_bounded_and_deletion_reclaims_capacity() {
        let now = Instant::now();
        let mut state = AnnotationState::new((0..8).map(|i| format!("s{i}")).collect()).unwrap();
        let epoch = state.epoch().to_owned();
        let terminals: Vec<_> = (0..64).map(|_| TerminalId::alloc()).collect();
        for terminal in &terminals {
            for source in 0..8 {
                state
                    .replace(
                        &epoch,
                        &format!("s{source}"),
                        terminal.clone(),
                        1,
                        None,
                        MIN_TTL,
                        now,
                    )
                    .unwrap();
            }
        }
        let extra = TerminalId::alloc();
        assert_eq!(
            state.replace(&epoch, "s0", extra.clone(), 1, None, MIN_TTL, now + MAX_TTL),
            Err(Error::Capacity)
        );
        assert_eq!(
            state
                .store
                .targets
                .values()
                .map(HashMap::len)
                .sum::<usize>(),
            512
        );
        state.retain_terminals(|_| true);
        assert_eq!(
            state.replace(
                &epoch,
                "s0",
                terminals[0].clone(),
                1,
                None,
                MIN_TTL,
                now + MAX_TTL
            ),
            Err(Error::StaleRevision)
        );
        state.retain_terminals(|id| id != &terminals[0]);
        state
            .replace(&epoch, "s0", extra, 1, None, MIN_TTL, now + MAX_TTL)
            .unwrap();
        assert_eq!(state.store.targets.len(), 64);
    }

    fn workspace() -> ResourceTarget {
        ResourceTarget::Workspace(crate::workspace::generate_workspace_id())
    }

    fn put(
        store: &mut AnnotationStore,
        source: &str,
        target: &ResourceTarget,
        revision: u64,
        text: Option<&str>,
        now: Instant,
    ) -> Result<(), Error> {
        store.replace(
            source,
            target.clone(),
            revision,
            text.map(str::to_owned),
            MAX_TTL,
            now,
        )
    }

    #[test]
    fn empty_selection_and_unselected_sources_enable_nothing() {
        let target = workspace();
        let now = Instant::now();
        let mut empty = AnnotationStore::new(vec![]).unwrap();
        assert_eq!(empty.project(&target, &now), None);
        assert_eq!(
            put(&mut empty, "build", &target, 1, Some("ok"), now),
            Err(Error::UnselectedSource)
        );
        assert_eq!(
            put(&mut store(), "other", &target, 1, None, now),
            Err(Error::UnselectedSource)
        );
        assert!(empty.targets.is_empty());
    }

    #[test]
    fn sources_are_ordered_unique_safe_and_bounded() {
        for sources in [
            vec!["build".into(), "build".into()],
            vec!["".into()],
            vec!["a b".into()],
            vec!["\u{e9}".into()],
            vec!["a".repeat(65)],
            (0..9).map(|i| format!("s{i}")).collect(),
        ] {
            assert!(matches!(
                AnnotationStore::new(sources),
                Err(Error::InvalidSources)
            ));
        }
        assert!(AnnotationStore::new(vec!["a".repeat(64)]).is_ok());
        assert!(AnnotationStore::new((0..8).map(|i| format!("s_{i}.-")).collect()).is_ok());
    }

    #[test]
    fn target_identity_is_distinct_by_id_and_kind() {
        let now = Instant::now();
        let terminal = TerminalId::alloc();
        let target = ResourceTarget::Terminal(terminal.clone());
        let other = ResourceTarget::Terminal(TerminalId::alloc());
        let same_spelling = ResourceTarget::Workspace(terminal.to_string());
        let mut store = store();
        put(&mut store, "build", &target, 0, Some("shell"), now).unwrap();
        assert_eq!(
            store.project(&ResourceTarget::Terminal(terminal), &now),
            Some("shell")
        );
        assert_eq!(store.project(&other, &now), None);
        assert_eq!(store.project(&same_spelling, &now), None);
        put(
            &mut store,
            "build",
            &same_spelling,
            0,
            Some("workspace"),
            now,
        )
        .unwrap();
        assert_eq!(store.project(&target, &now), Some("shell"));
        assert_eq!(store.project(&same_spelling, &now), Some("workspace"));
        assert_eq!(store.project(&workspace(), &now), None);
    }

    #[test]
    fn priority_is_independent_of_arrival_order() {
        let now = Instant::now();
        let target = workspace();
        for order in [["build", "tests"], ["tests", "build"]] {
            let mut store = store();
            for source in order {
                put(&mut store, source, &target, 1, Some(source), now).unwrap();
            }
            assert_eq!(store.project(&target, &now), Some("build"));
        }
    }

    #[test]
    fn lower_source_shows_only_until_higher_first_records() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        put(&mut store, "tests", &target, 1, Some("passed"), now).unwrap();
        assert_eq!(store.project(&target, &now), Some("passed"));
        put(&mut store, "build", &target, 1, None, now).unwrap();
        assert_eq!(store.project(&target, &now), None);
    }

    #[test]
    fn exact_expiry_hides_winner_without_fallback() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        store
            .replace(
                "build",
                target.clone(),
                1,
                Some("busy".into()),
                MIN_TTL,
                now,
            )
            .unwrap();
        put(
            &mut store,
            "tests",
            &target,
            1,
            Some("passed"),
            now + MIN_TTL,
        )
        .unwrap();
        assert_eq!(
            store.project(&target, &(now + MIN_TTL - Duration::from_nanos(1))),
            Some("busy")
        );
        assert_eq!(store.project(&target, &(now + MIN_TTL)), None);
        assert_eq!(store.project(&target, &(now + MAX_TTL)), None);
    }

    #[test]
    fn clear_hides_even_newer_lower_updates() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        put(&mut store, "build", &target, 1, Some("busy"), now).unwrap();
        put(&mut store, "build", &target, 2, None, now).unwrap();
        put(
            &mut store,
            "tests",
            &target,
            99,
            Some("passed"),
            now + MIN_TTL,
        )
        .unwrap();
        assert_eq!(store.project(&target, &(now + MIN_TTL)), None);
    }

    #[test]
    fn stale_revisions_cannot_clear_replace_or_renew() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        store
            .replace(
                "build",
                target.clone(),
                2,
                Some("original".into()),
                MIN_TTL,
                now,
            )
            .unwrap();
        for revision in [0, 1, 2] {
            for text in [None, Some("replacement")] {
                assert_eq!(
                    put(&mut store, "build", &target, revision, text, now + MIN_TTL),
                    Err(Error::StaleRevision)
                );
            }
        }
        assert_eq!(store.project(&target, &now), Some("original"));
        assert_eq!(store.project(&target, &(now + MIN_TTL)), None);
    }

    #[test]
    fn cleared_watermarks_reject_replay_after_expiry() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        put(&mut store, "build", &target, 7, None, now).unwrap();
        assert_eq!(
            put(
                &mut store,
                "build",
                &target,
                7,
                Some("replay"),
                now + MAX_TTL
            ),
            Err(Error::StaleRevision)
        );
        assert_eq!(store.project(&target, &(now + MAX_TTL)), None);
    }

    #[test]
    fn newer_full_replacement_never_resurrects_old_text() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        put(&mut store, "build", &target, 1, Some("long lived"), now).unwrap();
        store
            .replace(
                "build",
                target.clone(),
                2,
                Some("short".into()),
                MIN_TTL,
                now,
            )
            .unwrap();
        assert_eq!(store.project(&target, &now), Some("short"));
        assert_eq!(store.project(&target, &(now + MIN_TTL)), None);
    }

    #[test]
    fn invalid_text_is_atomic_and_revision_can_be_reused() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        put(&mut store, "build", &target, 1, Some("old"), now).unwrap();
        for text in [String::new(), "a".repeat(257), "\u{754c}".repeat(86)] {
            assert_eq!(
                store.replace("build", target.clone(), 2, Some(text), MAX_TTL, now),
                Err(Error::InvalidText)
            );
            assert_eq!(store.project(&target, &now), Some("old"));
        }
        let text = "\u{e9}".repeat(128);
        put(&mut store, "build", &target, 2, Some(&text), now).unwrap();
        assert_eq!(store.project(&target, &now), Some(text.as_str()));
    }

    #[test]
    fn workspace_ids_are_safe_bounded_not_existence_checked() {
        let now = Instant::now();
        let mut store = store();
        for id in [
            "".into(),
            "a b".into(),
            "\u{e9}".into(),
            "a\n".into(),
            "a".repeat(65),
        ] {
            assert_eq!(
                put(
                    &mut store,
                    "build",
                    &ResourceTarget::Workspace(id),
                    1,
                    None,
                    now
                ),
                Err(Error::InvalidTarget)
            );
        }
        assert!(store.targets.is_empty());
        put(
            &mut store,
            "build",
            &ResourceTarget::Workspace("a".repeat(64)),
            1,
            Some("not verified"),
            now,
        )
        .unwrap();
    }

    #[test]
    fn capacity_retains_watermarks_and_allows_existing_target_updates() {
        let now = Instant::now();
        let mut store = AnnotationStore::new((0..8).map(|i| format!("s{i}")).collect()).unwrap();
        let targets: Vec<_> = (0..64)
            .map(|i| ResourceTarget::Workspace(format!("w{i}")))
            .collect();
        for target in &targets {
            for source in 0..8 {
                put(&mut store, &format!("s{source}"), target, 1, None, now).unwrap();
            }
        }
        let later = now + MAX_TTL;
        assert_eq!(
            put(
                &mut store,
                "s0",
                &ResourceTarget::Workspace("overflow".into()),
                1,
                None,
                later
            ),
            Err(Error::Capacity)
        );
        assert_eq!(store.targets.len(), 64);
        assert_eq!(
            store.targets.values().map(HashMap::len).sum::<usize>(),
            8 * 64
        );
        for target in &targets {
            assert_eq!(
                put(&mut store, "s0", target, 1, Some("replay"), later),
                Err(Error::StaleRevision)
            );
            put(&mut store, "s0", target, 2, Some("new"), later).unwrap();
        }
    }

    #[test]
    fn new_source_on_existing_target_is_allowed_at_capacity() {
        let now = Instant::now();
        let mut store = store();
        let targets: Vec<_> = (0..64).map(|_| workspace()).collect();
        for target in &targets {
            put(&mut store, "tests", target, 1, None, now).unwrap();
        }
        let rejected = workspace();
        assert_eq!(
            put(&mut store, "build", &rejected, 1, None, now),
            Err(Error::Capacity)
        );
        assert!(!store.targets.contains_key(&rejected));
        put(&mut store, "build", &targets[0], 1, Some("ok"), now).unwrap();
        assert_eq!(store.project(&targets[0], &now), Some("ok"));
    }

    #[test]
    fn ttl_bounds_are_inclusive_and_invalid_updates_are_atomic() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        for ttl in [
            Duration::ZERO,
            MIN_TTL - Duration::from_nanos(1),
            MAX_TTL + Duration::from_nanos(1),
        ] {
            assert_eq!(
                store.replace("build", target.clone(), 1, None, ttl, now),
                Err(Error::InvalidTtl)
            );
            assert!(store.targets.is_empty());
        }
        store
            .replace("build", target.clone(), 1, Some("min".into()), MIN_TTL, now)
            .unwrap();
        store
            .replace("build", target.clone(), 2, Some("max".into()), MAX_TTL, now)
            .unwrap();
        assert_eq!(
            store.project(&target, &(now + MAX_TTL - Duration::from_nanos(1))),
            Some("max")
        );
        assert_eq!(store.project(&target, &(now + MAX_TTL)), None);
    }

    #[test]
    fn checked_deadline_overflow_does_not_consume_revision() {
        let now = Instant::now();
        // Locate the platform's representable Instant horizon without assuming its size.
        let (mut low, mut high) = (0, u64::MAX);
        while low < high {
            let mid = low + (high - low) / 2 + 1;
            if now.checked_add(Duration::from_secs(mid)).is_some() {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        let horizon = now.checked_add(Duration::from_secs(low)).unwrap();
        let target = workspace();
        let mut store = store();
        assert_eq!(
            put(&mut store, "build", &target, 1, None, horizon),
            Err(Error::DeadlineOverflow)
        );
        assert!(store.targets.is_empty());
        put(&mut store, "build", &target, 1, Some("ok"), now).unwrap();
    }

    #[test]
    fn time_regression_is_per_pair_and_atomic() {
        let now = Instant::now();
        let later = now + MIN_TTL;
        let target = workspace();
        let mut store = store();
        put(&mut store, "build", &target, 1, Some("old"), later).unwrap();
        assert_eq!(
            put(&mut store, "build", &target, 2, None, now),
            Err(Error::TimeRegression)
        );
        assert_eq!(store.project(&target, &later), Some("old"));
        put(&mut store, "tests", &target, 1, Some("independent"), now).unwrap();
        put(&mut store, "build", &workspace(), 1, None, now).unwrap();
        put(&mut store, "build", &target, 2, Some("same instant"), later).unwrap();
    }

    #[test]
    fn future_accepted_winner_is_invisible_without_fallback() {
        let now = Instant::now();
        let target = workspace();
        let mut store = store();
        put(&mut store, "tests", &target, 1, Some("lower"), now).unwrap();
        put(
            &mut store,
            "build",
            &target,
            1,
            Some("future"),
            now + MIN_TTL,
        )
        .unwrap();
        assert_eq!(store.project(&target, &now), None);
        assert_eq!(store.project(&target, &(now + MIN_TTL)), Some("future"));
    }

    #[test]
    fn renderer_escapes_all_controls_and_preserves_literal_markup() {
        let controls: String = (0..=0x9f)
            .filter(|n| *n < 0x20 || *n >= 0x7f)
            .filter_map(char::from_u32)
            .chain(['\u{061c}', '\u{200e}', '\u{200f}', '\u{2028}', '\u{2029}'])
            .chain((0x202a..=0x202e).filter_map(char::from_u32))
            .chain((0x2066..=0x2069).filter_map(char::from_u32))
            .collect();
        let rendered = render_text(&controls, 4096);
        let expected: String = controls.chars().flat_map(char::escape_unicode).collect();
        assert_eq!(rendered, expected);
        assert!(rendered.is_ascii());
        assert!(!rendered.chars().any(char::is_control));
        let literal = "[bold] <b> & ANSI[31m \u{754c}e\u{301}";
        assert_eq!(render_text(literal, 100), literal);
    }

    #[test]
    fn renderer_respects_unicode_cell_budgets() {
        for text in [
            "",
            "ascii",
            "\u{754c}\u{754c}",
            "e\u{301}cole",
            "\u{301}",
            "\u{1f469}\u{200d}\u{1f4bb}abc",
            "\u{2764}\u{fe0f}abc",
            "\u{1f1fa}\u{1f1f8}abc",
            "\u{1f44d}\u{1f3fd}abc",
            "1\u{fe0f}\u{20e3}abc",
            "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}abc",
        ] {
            for budget in 0..=text.width() + 2 {
                let rendered = render_text(text, budget);
                assert!(
                    rendered.width() <= budget,
                    "{text:?}, budget {budget}, {rendered:?}"
                );
                if budget == 0 {
                    assert!(rendered.is_empty());
                } else if text.width() <= budget {
                    assert_eq!(rendered, text);
                } else {
                    assert!(rendered.ends_with('\u{2026}'));
                    if budget == 1 {
                        assert_eq!(rendered, "\u{2026}");
                    }
                }
            }
        }
        assert_eq!(render_text("abcdef", 4), "abc\u{2026}");
        assert_eq!(render_text("\u{754c}\u{754c}", 3), "\u{754c}\u{2026}");
    }

    #[test]
    fn escape_expansion_is_measured_before_clipping() {
        assert_eq!(render_text("\n", 6), "\\u{a}");
        assert_eq!(render_text("\n", 4), "\\u{\u{2026}");
        assert_eq!(render_text("\u{1b}[31m", 1), "\u{2026}");
        for budget in 0..20 {
            assert!(render_text("\n\t\u{1b}\u{202e}", budget).width() <= budget);
        }
    }

    #[test]
    fn ordinary_terminal_annotations_render_only_in_caller_supplied_area() {
        let now = Instant::now();
        let target = ResourceTarget::Terminal(TerminalId::alloc());
        let mut store = store();
        put(&mut store, "tests", &target, 1, Some("[passed]"), now).unwrap();
        store
            .replace(
                "build",
                target.clone(),
                1,
                Some("[ok]".into()),
                MIN_TTL,
                now,
            )
            .unwrap();
        let area = Rect::new(2, 1, 8, 1);
        let mut buffer = Buffer::filled(Rect::new(0, 0, 12, 3), ratatui::buffer::Cell::new("."));
        for x in area.x..area.right() {
            buffer[(x, area.y)].reset();
        }
        let before = buffer;
        for at in [now, now + MIN_TTL] {
            // Like a normal TUI frame, start with a fresh caller-owned buffer.
            let mut buffer = before.clone();
            let text = render_text(
                store.project(&target, &at).unwrap_or(""),
                area.width as usize,
            );
            assert_eq!(text, if at == now { "[ok]" } else { "" });
            Paragraph::new(text).render(area, &mut buffer);
            if at == now {
                assert_eq!(buffer[(2, 1)].symbol(), "[");
                assert_eq!(buffer[(3, 1)].symbol(), "o");
                assert_eq!(buffer[(5, 1)].symbol(), "]");
            }
            for y in 0..3 {
                for x in 0..12 {
                    if !area.contains((x, y).into()) {
                        assert_eq!(buffer[(x, y)], before[(x, y)]);
                    } else if at != now {
                        assert_eq!(buffer[(x, y)].symbol(), " ");
                    }
                }
            }
        }
    }
}
