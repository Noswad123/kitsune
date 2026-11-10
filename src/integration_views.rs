//! Transient, source-keyed presentation IPC. Source and lease IDs are not authentication.
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use crate::api::schema::{
    IntegrationViewIntent, IntegrationViewItem, IntegrationViewSourceInfo, IntegrationViewsInfo,
    ViewIntents, ViewOpenParams, ViewOpened, ViewOwnerParams, ViewReplaceParams, ViewReplaced,
};
use crate::terminal::TerminalId;

const TTL: Duration = Duration::from_secs(5);
const TTL_MS: u64 = 5000;
const MAX_INTENTS: usize = 32;

pub(crate) fn valid_source(source: &str) -> bool {
    (1..=64).contains(&source.len())
        && source
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

pub fn validate_sources(sources: &[String]) -> Result<(), &'static str> {
    let mut seen = HashSet::new();
    if sources.len() > 8
        || sources
            .iter()
            .any(|source| !valid_source(source) || !seen.insert(source))
    {
        return Err("integration_view_source_disabled");
    }
    Ok(())
}

#[derive(Debug)]
struct Owner {
    lease: String,
    epoch: String,
    deadline: Instant,
    revision: u64,
    items: Vec<IntegrationViewItem>,
    intents: VecDeque<IntegrationViewIntent>,
}

#[derive(Debug, Default)]
pub(crate) struct IntegrationViewsState {
    sources: Vec<String>,
    owners: HashMap<String, Owner>,
}

#[derive(Debug)]
pub(crate) struct ViewProjection<'a> {
    pub epoch: &'a str,
    pub revision: u64,
    pub items: &'a [IntegrationViewItem],
}

impl IntegrationViewsState {
    pub(crate) fn new(sources: Vec<String>) -> Self {
        let mut state = Self::default();
        let _ = state.reconfigure(sources);
        state
    }

    pub(crate) fn configured_sources(&self) -> &[String] {
        &self.sources
    }

    pub(crate) fn reconfigure(&mut self, sources: Vec<String>) -> Result<bool, &'static str> {
        validate_sources(&sources)?;
        if self.sources == sources {
            return Ok(false);
        }
        self.owners.retain(|source, _| sources.contains(source));
        self.sources = sources;
        Ok(true)
    }

    pub(crate) fn expire(&mut self, now: Instant) -> bool {
        let before = self.owners.len();
        self.owners.retain(|_, owner| now < owner.deadline);
        before != self.owners.len()
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.owners.values().map(|owner| owner.deadline).min()
    }

    pub(crate) fn project(&self, source: &str, now: Instant) -> Option<ViewProjection<'_>> {
        let owner = self.owners.get(source)?;
        (now < owner.deadline && owner.revision > 0).then_some(ViewProjection {
            epoch: &owner.epoch,
            revision: owner.revision,
            items: &owner.items,
        })
    }

    pub(crate) fn descriptor(&self, now: Instant) -> IntegrationViewsInfo {
        IntegrationViewsInfo {
            version: 1,
            sources: self
                .sources
                .iter()
                .map(|source| IntegrationViewSourceInfo {
                    id: source.clone(),
                    epoch: self
                        .owners
                        .get(source)
                        .filter(|owner| now < owner.deadline)
                        .map(|owner| owner.epoch.clone()),
                })
                .collect(),
        }
    }

    fn check_source(&self, source: &str) -> Result<(), &'static str> {
        if self.sources.is_empty() {
            Err("integration_view_disabled")
        } else if !self.sources.iter().any(|configured| configured == source) {
            Err("integration_view_source_disabled")
        } else {
            Ok(())
        }
    }

    fn check_owner(&self, source: &str, lease: &str, epoch: &str) -> Result<(), &'static str> {
        self.check_source(source)?;
        if self
            .owners
            .get(source)
            .is_some_and(|owner| owner.lease == lease && owner.epoch == epoch)
        {
            Ok(())
        } else {
            Err("integration_view_lease_invalid_or_expired")
        }
    }

    pub(crate) fn open(
        &mut self,
        params: &ViewOpenParams,
        now: Instant,
    ) -> Result<ViewOpened, &'static str> {
        self.expire(now);
        self.check_source(&params.source)?;
        if params.version != 1 {
            return Err("integration_view_unsupported_version");
        }
        if self.owners.contains_key(&params.source) {
            return Err("integration_view_busy");
        }
        let lease = TerminalId::alloc().to_string();
        let epoch = TerminalId::alloc().to_string();
        self.owners.insert(
            params.source.clone(),
            Owner {
                lease: lease.clone(),
                epoch: epoch.clone(),
                deadline: now + TTL,
                revision: 0,
                items: Vec::new(),
                intents: VecDeque::new(),
            },
        );
        Ok(ViewOpened {
            lease,
            epoch,
            revision: 0,
            ttl_ms: TTL_MS,
        })
    }

    pub(crate) fn replace(
        &mut self,
        params: &ViewReplaceParams,
        now: Instant,
    ) -> Result<ViewReplaced, &'static str> {
        self.expire(now);
        self.check_owner(&params.source, &params.lease, &params.epoch)?;
        let Some(owner) = self.owners.get_mut(&params.source) else {
            return Err("integration_view_lease_invalid_or_expired");
        };
        if params.revision <= owner.revision {
            return Err("integration_view_stale_revision");
        }
        validate_frame(&params.items)?;
        owner.items.clone_from(&params.items);
        owner.revision = params.revision;
        owner.intents.clear();
        owner.deadline = now + TTL;
        Ok(ViewReplaced {
            epoch: owner.epoch.clone(),
            revision: owner.revision,
        })
    }

    pub(crate) fn poll(
        &mut self,
        params: &ViewOwnerParams,
        now: Instant,
    ) -> Result<ViewIntents, &'static str> {
        self.expire(now);
        self.check_owner(&params.source, &params.lease, &params.epoch)?;
        let Some(owner) = self.owners.get_mut(&params.source) else {
            return Err("integration_view_lease_invalid_or_expired");
        };
        owner.deadline = now + TTL;
        Ok(ViewIntents {
            epoch: owner.epoch.clone(),
            revision: owner.revision,
            intents: owner.intents.drain(..).collect(),
            ttl_ms: TTL_MS,
        })
    }

    pub(crate) fn close(
        &mut self,
        params: &ViewOwnerParams,
        now: Instant,
    ) -> Result<(), &'static str> {
        self.expire(now);
        self.check_owner(&params.source, &params.lease, &params.epoch)?;
        self.owners.remove(&params.source);
        Ok(())
    }

    pub(crate) fn activate(
        &mut self,
        source: &str,
        epoch: &str,
        revision: u64,
        item_id: &str,
        now: Instant,
    ) -> bool {
        self.expire(now);
        let Some(owner) = self.owners.get_mut(source) else {
            return false;
        };
        if owner.epoch != epoch
            || revision == 0
            || owner.revision != revision
            || owner.intents.len() >= MAX_INTENTS
            || !owner.items.iter().any(|item| item.id == item_id)
        {
            return false;
        }
        owner.intents.push_back(IntegrationViewIntent {
            epoch: epoch.to_owned(),
            revision,
            item_id: item_id.to_owned(),
        });
        true
    }
}

fn validate_frame(items: &[IntegrationViewItem]) -> Result<(), &'static str> {
    let valid_text = |text: &str, max: usize, nonblank: bool| {
        text.len() <= max
            && (!nonblank || !text.trim().is_empty())
            && !text.chars().any(char::is_control)
    };
    let mut ids = HashSet::new();
    let mut bytes = 0usize;
    if items.len() > 128 {
        return Err("integration_view_invalid_frame");
    }
    for item in items {
        if !valid_text(&item.id, 128, true)
            || !valid_text(&item.label, 160, true)
            || !valid_text(&item.detail, 512, false)
            || !ids.insert(item.id.as_str())
        {
            return Err("integration_view_invalid_frame");
        }
        bytes = bytes
            .checked_add(item.id.len())
            .and_then(|bytes| bytes.checked_add(item.label.len()))
            .and_then(|bytes| bytes.checked_add(item.detail.len()))
            .ok_or("integration_view_invalid_frame")?;
        if bytes > 96 * 1024 {
            return Err("integration_view_invalid_frame");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(state: &mut IntegrationViewsState, source: &str, now: Instant) -> ViewOwnerParams {
        let opened = state
            .open(
                &ViewOpenParams {
                    source: source.into(),
                    version: 1,
                },
                now,
            )
            .unwrap();
        assert_eq!((opened.revision, opened.ttl_ms), (0, 5000));
        assert!(state.project(source, now).is_none());
        ViewOwnerParams {
            source: source.into(),
            lease: opened.lease,
            epoch: opened.epoch,
        }
    }

    fn frame(owner: &ViewOwnerParams, revision: u64) -> ViewReplaceParams {
        ViewReplaceParams {
            source: owner.source.clone(),
            lease: owner.lease.clone(),
            epoch: owner.epoch.clone(),
            revision,
            items: vec![IntegrationViewItem {
                id: "same".into(),
                label: owner.source.clone(),
                detail: String::new(),
            }],
        }
    }

    #[test]
    fn sources_are_ordered_unique_bounded_and_reconfigured_atomically() {
        let now = Instant::now();
        let mut state = IntegrationViewsState::new(vec!["a".into(), "b".into()]);
        let a = open(&mut state, "a", now);
        let b = open(&mut state, "b", now);
        state.replace(&frame(&a, 1), now).unwrap();
        assert!(state.activate("a", &a.epoch, 1, "same", now));
        for sources in [
            vec!["a".into(), "a".into()],
            vec!["a".into(), "bad source".into()],
            (0..9).map(|i| i.to_string()).collect(),
        ] {
            assert!(IntegrationViewsState::new(sources.clone())
                .configured_sources()
                .is_empty());
            assert!(state.reconfigure(sources).is_err());
            assert_eq!(state.configured_sources(), ["a", "b"]);
        }
        for source in ["", "has space", "slash/path", "\u{e9}", &"x".repeat(65)] {
            assert!(!valid_source(source));
        }
        assert!(valid_source(&"a".repeat(64)));
        assert!(valid_source("AZaz09_.-"));
        assert_eq!(state.reconfigure(vec!["b".into(), "a".into()]), Ok(true));
        assert_eq!(state.reconfigure(vec!["b".into(), "a".into()]), Ok(false));
        assert_eq!(
            state
                .descriptor(now)
                .sources
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert_eq!(state.poll(&a, now).unwrap().intents.len(), 1);
        assert_eq!(state.project("a", now).unwrap().epoch, a.epoch);
        assert_eq!(state.reconfigure(vec!["b".into()]), Ok(true));
        assert!(state.poll(&b, now).is_ok());
        assert_eq!(state.poll(&a, now), Err("integration_view_source_disabled"));
        state.reconfigure(vec!["a".into(), "b".into()]).unwrap();
        assert_eq!(state.descriptor(now).sources[0].epoch, None);
        let fresh = open(&mut state, "a", now);
        assert_ne!(fresh.epoch, a.epoch);
        assert!(state.poll(&fresh, now).unwrap().intents.is_empty());
    }

    #[test]
    fn concurrent_sources_isolate_ownership_revisions_queues_and_close() {
        let now = Instant::now();
        let mut state = IntegrationViewsState::new(vec!["a".into(), "b".into()]);
        let a = open(&mut state, "a", now);
        let b = open(&mut state, "b", now);
        assert_eq!(
            state.open(
                &ViewOpenParams {
                    source: "a".into(),
                    version: 1
                },
                now
            ),
            Err("integration_view_busy")
        );
        for owner in [&a, &b] {
            state.replace(&frame(owner, 1), now).unwrap();
        }
        let crossed = ViewOwnerParams {
            source: "b".into(),
            ..a.clone()
        };
        assert_eq!(
            state.poll(&crossed, now),
            Err("integration_view_lease_invalid_or_expired")
        );
        assert!(!state.activate("b", &a.epoch, 1, "same", now));
        assert!(!state.activate("a", &a.epoch, 2, "same", now));
        assert!(!state.activate("a", &a.epoch, 1, "missing", now));
        for _ in 0..32 {
            assert!(state.activate("a", &a.epoch, 1, "same", now));
        }
        assert!(!state.activate("a", &a.epoch, 1, "same", now));
        assert!(state.activate("b", &b.epoch, 1, "same", now));
        assert_eq!(state.poll(&b, now).unwrap().intents.len(), 1);
        assert!(state.poll(&b, now).unwrap().intents.is_empty());
        assert_eq!(state.poll(&a, now).unwrap().intents.len(), 32);
        assert!(state.activate("b", &b.epoch, 1, "same", now));
        state.replace(&frame(&a, 2), now).unwrap();
        assert_eq!(state.project("b", now).unwrap().revision, 1);
        assert_eq!(
            state.replace(&frame(&a, 2), now),
            Err("integration_view_stale_revision")
        );
        state.close(&a, now).unwrap();
        assert!(state.project("a", now).is_none());
        assert_eq!(state.project("b", now).unwrap().items[0].label, "b");
        assert_eq!(state.poll(&b, now).unwrap().intents.len(), 1);
    }

    #[test]
    fn polling_b_does_not_renew_a_and_expiry_only_removes_due_owners() {
        let now = Instant::now();
        let mut state = IntegrationViewsState::new(vec!["a".into(), "b".into()]);
        let a = open(&mut state, "a", now);
        let b = open(&mut state, "b", now);
        for owner in [&a, &b] {
            state.replace(&frame(owner, 1), now).unwrap();
        }
        let later = now + Duration::from_secs(4);
        state.poll(&b, later).unwrap();
        assert!(state.activate("a", &a.epoch, 1, "same", later));
        assert_eq!(state.next_deadline(), Some(now + TTL));
        assert!(state.project("a", now + TTL).is_none());
        assert_eq!(state.descriptor(now + TTL).sources[0].epoch, None);
        assert!(state.expire(now + TTL));
        assert!(!state.expire(now + TTL));
        assert_eq!(state.next_deadline(), Some(later + TTL));
        assert!(state.project("b", now + TTL).is_some());
        assert_eq!(
            state.poll(&a, now + TTL),
            Err("integration_view_lease_invalid_or_expired")
        );
        state.replace(&frame(&b, 2), now + TTL).unwrap();
        assert_eq!(state.next_deadline(), Some(now + TTL + TTL));
    }

    #[test]
    fn failed_operations_and_activation_do_not_renew_or_discard_pending_intents() {
        let now = Instant::now();
        let mut state = IntegrationViewsState::new(vec!["a".into()]);
        let owner = open(&mut state, "a", now);
        state.replace(&frame(&owner, 1), now).unwrap();
        let later = now + Duration::from_secs(4);
        assert!(state.activate("a", &owner.epoch, 1, "same", later));
        assert!(!state.activate("a", &owner.epoch, 0, "same", later));
        assert_eq!(
            state.open(
                &ViewOpenParams {
                    source: "a".into(),
                    version: 2
                },
                later
            ),
            Err("integration_view_unsupported_version")
        );
        for wrong in [
            ViewOwnerParams {
                lease: "wrong".into(),
                ..owner.clone()
            },
            ViewOwnerParams {
                epoch: "wrong".into(),
                ..owner.clone()
            },
        ] {
            assert_eq!(
                state.poll(&wrong, later),
                Err("integration_view_lease_invalid_or_expired")
            );
            assert_eq!(
                state.close(&wrong, later),
                Err("integration_view_lease_invalid_or_expired")
            );
            let invalid = frame(&wrong, 2);
            assert_eq!(
                state.replace(&invalid, later),
                Err("integration_view_lease_invalid_or_expired")
            );
        }
        let mut invalid = frame(&owner, 2);
        invalid.items[0].label = "bad\nlabel".into();
        assert_eq!(
            state.replace(&invalid, later),
            Err("integration_view_invalid_frame")
        );
        assert_eq!(state.next_deadline(), Some(now + TTL));
        assert_eq!(state.project("a", later).unwrap().items[0].label, "a");
        assert_eq!(state.poll(&owner, later).unwrap().intents.len(), 1);
        assert!(state.activate("a", &owner.epoch, 1, "same", later));
        let mut empty = frame(&owner, 2);
        empty.items.clear();
        state.replace(&empty, later).unwrap();
        assert!(state.project("a", later).unwrap().items.is_empty());
        assert!(state.poll(&owner, later).unwrap().intents.is_empty());
        state.replace(&frame(&owner, u64::MAX), later).unwrap();
        assert_eq!(
            state.replace(&frame(&owner, u64::MAX), later),
            Err("integration_view_stale_revision")
        );
    }

    #[test]
    fn every_operation_enforces_expiry_and_reopened_sources_are_fresh() {
        let now = Instant::now();
        for operation in 0..5 {
            let mut state = IntegrationViewsState::new(vec!["a".into()]);
            let owner = open(&mut state, "a", now);
            state.replace(&frame(&owner, 1), now).unwrap();
            assert!(state.activate("a", &owner.epoch, 1, "same", now));
            let expiry = now + TTL;
            match operation {
                0 => {
                    let fresh = open(&mut state, "a", expiry);
                    assert_ne!(fresh.lease, owner.lease);
                    assert_ne!(fresh.epoch, owner.epoch);
                    assert!(state.poll(&fresh, expiry).unwrap().intents.is_empty());
                    assert_eq!(
                        state.poll(&owner, expiry),
                        Err("integration_view_lease_invalid_or_expired")
                    );
                    state.close(&fresh, expiry).unwrap();
                    assert_eq!(
                        state.close(&fresh, expiry),
                        Err("integration_view_lease_invalid_or_expired")
                    );
                }
                1 => assert_eq!(
                    state.replace(&frame(&owner, 2), expiry),
                    Err("integration_view_lease_invalid_or_expired")
                ),
                2 => assert_eq!(
                    state.poll(&owner, expiry),
                    Err("integration_view_lease_invalid_or_expired")
                ),
                3 => assert_eq!(
                    state.close(&owner, expiry),
                    Err("integration_view_lease_invalid_or_expired")
                ),
                _ => assert!(!state.activate("a", &owner.epoch, 1, "same", expiry)),
            }
            assert_eq!(state.next_deadline(), None);
            assert_eq!(state.descriptor(expiry).sources[0].epoch, None);
        }
        assert_eq!(
            IntegrationViewsState::default().descriptor(now),
            IntegrationViewsInfo {
                version: 1,
                sources: Vec::new()
            }
        );
    }

    #[test]
    fn invalid_frames_are_atomic_and_per_source_bounds_allow_full_aggregate() {
        let now = Instant::now();
        let mut state = IntegrationViewsState::new((0..8).map(|i| i.to_string()).collect());
        let mut owners = Vec::new();
        for i in 0..8 {
            let owner = open(&mut state, &i.to_string(), now);
            let mut full = frame(&owner, 1);
            full.items = (0..128)
                .map(|i| IntegrationViewItem {
                    id: format!("{i:0128}"),
                    label: "l".repeat(160),
                    detail: "d".repeat(480),
                })
                .collect();
            state.replace(&full, now).unwrap();
            for _ in 0..32 {
                assert!(state.activate(&owner.source, &owner.epoch, 1, &full.items[0].id, now));
            }
            let mut invalid = full.clone();
            invalid.revision = 2;
            invalid.items[0].detail.push('x');
            assert_eq!(
                state.replace(&invalid, now + Duration::from_secs(4)),
                Err("integration_view_invalid_frame")
            );
            assert_eq!(state.project(&owner.source, now).unwrap().revision, 1);
            owners.push(owner);
        }
        assert_eq!(state.next_deadline(), Some(now + TTL));
        assert_eq!(
            owners
                .iter()
                .map(|o| state.project(&o.source, now).unwrap().items.len())
                .sum::<usize>(),
            1024
        );
        for owner in &owners {
            assert_eq!(state.poll(owner, now).unwrap().intents.len(), 32);
        }
        let item = frame(&owners[0], 1).items.remove(0);
        assert!(validate_frame(&vec![item.clone(); 129]).is_err());
        assert!(validate_frame(&[item.clone(), item.clone()]).is_err());
        for (field, value) in [
            ("id", " ".into()),
            ("id", "x".repeat(129)),
            ("id", "\u{7f}".into()),
            ("label", "\t".into()),
            ("label", "\u{e9}".repeat(81)),
            ("detail", "x".repeat(513)),
            ("detail", "\u{1b}[31m".into()),
        ] {
            let mut invalid = item.clone();
            match field {
                "id" => invalid.id = value,
                "label" => invalid.label = value,
                _ => invalid.detail = value,
            }
            assert!(validate_frame(&[invalid]).is_err());
        }
    }
}
