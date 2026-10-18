//! The set of entities one ingestion pass owns.
//!
//! Pruning is the dangerous half of ingestion: a delete that is scoped only by
//! entity takes every writer's grants on that entity with it. With several
//! services bundles composed into one tree, and an operator editing rules in
//! the dashboard against the same table, "orphan" is only meaningful relative
//! to an owner.
//!
//! [`IngestScope`] is the caller's declaration of what this pass owns, by
//! kind. A prune then touches a row only when the row's `source` is the
//! ingesting source *and* its entity falls inside this scope. An empty scope
//! asserts no ownership boundary and prunes wherever the source matches, which
//! is the baked services tree's case: it is the only writer of `yaml` rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use super::super::types::EntityKind;

#[derive(Debug, Clone, Default)]
pub struct IngestScope {
    by_kind: HashMap<EntityKind, HashSet<String>>,
}

impl IngestScope {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_kind<I, S>(mut self, kind: EntityKind, ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.by_kind
            .entry(kind)
            .or_default()
            .extend(ids.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn is_unscoped(&self) -> bool {
        self.by_kind.is_empty()
    }

    #[must_use]
    pub fn owns(&self, kind: EntityKind, id: &str) -> bool {
        if self.is_unscoped() {
            return true;
        }
        self.by_kind
            .get(&kind)
            .is_some_and(|owned| owned.contains(id))
    }
}
