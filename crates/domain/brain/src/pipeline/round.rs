//! [`RoundCache`]: what one consumer round reads once for every event.
//!
//! The per-event stages then read nothing but the tombstone: the
//! result-ledger claims the round made as a set, the alias resolution of
//! every entered subject, and the dedupe rows (facts by identity, live
//! neighbours by kind and subject, tombstones) of every entered candidate.
//! The tombstones preloaded here only short-circuit; a candidate the cache
//! does not hold as tombstoned is re-checked against the store at write
//! time, since an erase can land while the round runs.
//!
//! Coverage is explicit. A lookup answers `None` when the round did not
//! preload that pair or identity (a document event's extracted candidates,
//! a validator that rewrote the fields), and the stage reads the store as
//! it always did. Within-round ordering: every write an event commits is
//! applied to the cache through [`DedupeWrite`] before the next event
//! runs, so two events for one identity in one round resolve to one fact
//! revised twice, exactly as two rounds would. Writes are applied only
//! after the event's savepoint commits, so a rolled-back event leaves no
//! trace here either.
//!
//! [`RoundCache::forget_reads`] turns the preloaded reads off for the rest
//! of the round: every lookup answers `None` and applied writes are
//! dropped, so a round that rolled its per-event writes back and runs its
//! events again reads the store instead of rows that no longer exist.
//! The ledger claims and the remembered extractions
//! ([`super::round_extractions`]) stay, since both survive that rollback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{BrainFactId, EventOutboxId, FactKindId};
use systemprompt_models::brain::{FactStatus, SubjectKey};

use super::dedupe::{DedupeSnapshot, Existing};
use super::round_extractions::Extractions;

const NEIGHBOUR_LIMIT: usize = 50;

/// One stored fact as the round's dedupe reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedFact {
    pub id: BrainFactId,
    pub kind: FactKindId,
    pub subject: SubjectKey,
    pub identity_hash: String,
    pub statement: String,
    pub statement_hash: String,
    pub live: bool,
    pub updated_at: DateTime<Utc>,
}

/// A write an event committed that changes what later events of the round
/// dedupe against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DedupeWrite {
    Created(CachedFact),
    Revised {
        id: BrainFactId,
        statement: String,
        statement_hash: String,
        status: FactStatus,
    },
    Staled(Vec<BrainFactId>),
}

/// What the preload hands the cache.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoundRows {
    pub claimed: HashSet<EventOutboxId>,
    pub resolved: HashMap<SubjectKey, SubjectKey>,
    pub pairs: HashSet<(FactKindId, SubjectKey)>,
    pub identities: HashSet<(FactKindId, String)>,
    pub tombstones: HashSet<(FactKindId, String)>,
    pub facts: Vec<CachedFact>,
}

#[derive(Debug)]
pub struct RoundCache {
    inner: Mutex<RoundRows>,
    reads: AtomicBool,
    pub(super) extractions: Extractions,
}

impl Default for RoundCache {
    fn default() -> Self {
        Self::new(RoundRows::default())
    }
}

impl RoundCache {
    #[must_use]
    pub fn new(rows: RoundRows) -> Self {
        Self {
            inner: Mutex::new(rows),
            reads: AtomicBool::new(true),
            extractions: Extractions::new(),
        }
    }

    pub fn forget_reads(&self) {
        self.reads.store(false, Ordering::SeqCst);
    }

    #[must_use]
    pub fn reads(&self) -> bool {
        self.reads.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn claimed(&self, id: &EventOutboxId) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .claimed
            .contains(id)
    }

    #[must_use]
    pub fn resolved(&self, subject: &SubjectKey) -> Option<SubjectKey> {
        if !self.reads() {
            return None;
        }
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .resolved
            .get(subject)
            .cloned()
    }

    #[must_use]
    pub fn snapshot(
        &self,
        kind: &FactKindId,
        subject: &SubjectKey,
        identity_hash: &str,
    ) -> Option<DedupeSnapshot> {
        if !self.reads() {
            return None;
        }
        let rows = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let pair = (kind.clone(), subject.clone());
        let identity = (kind.clone(), identity_hash.to_owned());
        if !rows.pairs.contains(&pair) || !rows.identities.contains(&identity) {
            return None;
        }
        if rows.tombstones.contains(&identity) {
            return Some(DedupeSnapshot {
                tombstoned: true,
                ..DedupeSnapshot::default()
            });
        }
        let by_identity = rows
            .facts
            .iter()
            .find(|fact| fact.kind == *kind && fact.identity_hash == identity_hash)
            .map(existing);
        let mut neighbours: Vec<&CachedFact> = rows
            .facts
            .iter()
            .filter(|fact| {
                fact.live
                    && fact.kind == *kind
                    && fact.subject == *subject
                    && fact.identity_hash != identity_hash
            })
            .collect();
        neighbours.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
        neighbours.truncate(NEIGHBOUR_LIMIT);
        neighbours.sort_by(|a, b| a.id.cmp(&b.id));
        let snapshot = DedupeSnapshot {
            tombstoned: false,
            by_identity,
            neighbours: neighbours.into_iter().map(existing).collect(),
        };
        drop(rows);
        Some(snapshot)
    }

    pub fn apply(&self, writes: Vec<DedupeWrite>, now: DateTime<Utc>) {
        if !self.reads() {
            return;
        }
        let mut rows = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        for write in writes {
            match write {
                DedupeWrite::Created(mut fact) => {
                    rows.pairs.insert((fact.kind.clone(), fact.subject.clone()));
                    rows.identities
                        .insert((fact.kind.clone(), fact.identity_hash.clone()));
                    fact.updated_at = now;
                    rows.facts.retain(|known| known.id != fact.id);
                    rows.facts.push(fact);
                },
                DedupeWrite::Revised {
                    id,
                    statement,
                    statement_hash,
                    status,
                } => {
                    if let Some(fact) = rows.facts.iter_mut().find(|fact| fact.id == id) {
                        fact.statement = statement;
                        fact.statement_hash = statement_hash;
                        fact.live = is_live(status);
                        fact.updated_at = now;
                    }
                },
                DedupeWrite::Staled(ids) => {
                    for fact in rows.facts.iter_mut().filter(|fact| ids.contains(&fact.id)) {
                        fact.live = false;
                    }
                },
            }
        }
    }
}

#[must_use]
pub const fn is_live(status: FactStatus) -> bool {
    matches!(
        status,
        FactStatus::Candidate | FactStatus::Proposed | FactStatus::Confirmed | FactStatus::Disputed
    )
}

fn existing(fact: &CachedFact) -> Existing {
    Existing {
        id: fact.id.clone(),
        statement: fact.statement.clone(),
        statement_hash: fact.statement_hash.clone(),
    }
}
