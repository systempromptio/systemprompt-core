//! The pure collectors behind the set-based reindex: [`ReindexBatch`],
//! the fact ids a consumer round lands on, each once in landing order,
//! and [`AliasRows`], the alias rows one `INSERT` writes for a set of
//! facts, folded the way the sequential upserts would have left the table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

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

/// The columns of one `INSERT … SELECT FROM unnest(…)` over
/// `brain_aliases`: parallel arrays, one entry per distinct
/// `(subject_kind, alias_norm)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliasRows {
    pub kinds: Vec<String>,
    pub subjects: Vec<String>,
    pub aliases: Vec<String>,
    pub norms: Vec<String>,
}

impl AliasRows {
    #[must_use]
    pub fn collect<'a>(facts: impl IntoIterator<Item = (&'a SubjectKey, Vec<String>)>) -> Self {
        let mut slots: HashMap<(String, String), usize> = HashMap::new();
        let mut rows = Self::default();
        for (subject, aliases) in facts {
            for alias in aliases {
                let norm = AliasRepository::normalise(&alias);
                if norm.is_empty() || norm.chars().count() > MAX_ALIAS_NORM_LEN {
                    continue;
                }
                let key = (subject.kind.clone(), norm.clone());
                if let Some(&at) = slots.get(&key) {
                    rows.subjects[at].clone_from(&subject.id);
                    rows.aliases[at] = alias;
                } else {
                    slots.insert(key, rows.kinds.len());
                    rows.kinds.push(subject.kind.clone());
                    rows.subjects.push(subject.id.clone());
                    rows.aliases.push(alias);
                    rows.norms.push(norm);
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
