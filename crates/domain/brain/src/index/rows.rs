//! The pure collectors behind the set-based reindex: [`ReindexBatch`],
//! the fact ids a consumer round lands on, each once in landing order,
//! and [`AliasRows`], the alias rows one `INSERT` writes for a set of
//! facts, folded to one row per `(subject_kind, alias_norm)`.
//!
//! Who owns a shared alias is a rule, not an order of arrival: the fact
//! with the greatest `(updated_at, id)` among those that contribute it.
//! The caller hands facts in that order, so the fold keeps the last one,
//! and each row carries its owner's id and stamp so the upsert
//! ([`crate::repository::AliasRepository::upsert_many`]) only moves an
//! alias to a fact at least as recent as its current owner. Two batches of
//! one reindex, in any order, then converge on the same table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::SubjectKey;

use crate::repository::AliasRepository;

// Why: Postgres — brain_aliases.alias_norm carries CHECK (length BETWEEN 1
// AND 255); a row outside it fails the whole set statement, so it is left
// out here rather than failing every fact in the round with it.
pub const MAX_ALIAS_NORM_LEN: usize = 255;

/// The fact ids one round has landed on, in order, each once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReindexBatch {
    ids: Vec<BrainFactId>,
}

impl ReindexBatch {
    pub fn extend(&mut self, ids: impl IntoIterator<Item = BrainFactId>) {
        for id in ids {
            if !self.ids.contains(&id) {
                self.ids.push(id);
            }
        }
    }

    #[must_use]
    pub fn ids(&self) -> &[BrainFactId] {
        &self.ids
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// The fact that contributes a set of aliases, and the stamp that decides
/// whether it may own them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AliasOwner<'a> {
    pub subject: &'a SubjectKey,
    pub fact: &'a BrainFactId,
    pub updated_at: DateTime<Utc>,
}

/// The columns of one `INSERT … SELECT FROM unnest(…)` over
/// `brain_aliases`: parallel arrays, one entry per distinct
/// `(subject_kind, alias_norm)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliasRows {
    pub kinds: Vec<String>,
    pub subjects: Vec<String>,
    pub aliases: Vec<String>,
    pub norms: Vec<String>,
    pub owners: Vec<String>,
    pub stamps: Vec<DateTime<Utc>>,
}

impl AliasRows {
    #[must_use]
    pub fn collect<'a>(facts: impl IntoIterator<Item = (AliasOwner<'a>, Vec<String>)>) -> Self {
        let mut slots: HashMap<(String, String), usize> = HashMap::new();
        let mut rows = Self::default();
        for (owner, aliases) in facts {
            let subject = owner.subject;
            for alias in aliases {
                let norm = AliasRepository::normalise(&alias);
                if norm.is_empty() || norm.chars().count() > MAX_ALIAS_NORM_LEN {
                    continue;
                }
                let key = (subject.kind.clone(), norm.clone());
                if let Some(&at) = slots.get(&key) {
                    rows.subjects[at].clone_from(&subject.id);
                    rows.aliases[at] = alias;
                    owner.fact.as_str().clone_into(&mut rows.owners[at]);
                    rows.stamps[at] = owner.updated_at;
                } else {
                    slots.insert(key, rows.kinds.len());
                    rows.kinds.push(subject.kind.clone());
                    rows.subjects.push(subject.id.clone());
                    rows.aliases.push(alias);
                    rows.norms.push(norm);
                    rows.owners.push(owner.fact.as_str().to_owned());
                    rows.stamps.push(owner.updated_at);
                }
            }
        }
        rows
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.kinds.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}
