//! Per-entity catalogue row and rules for [`super::RuleBasedHook`], held for
//! as long as the [`ParentChainIndex`] they were read with.
//!
//! The chain index is reloaded whenever the fingerprint over
//! `access_control_rules` and `access_control_entities` moves or its TTL
//! elapses. An entry here is only served while the hook is resolving against
//! the very index instance that was current when the entry was cached, so a
//! reload drops every entry and per-entity staleness is bounded exactly as
//! the chain index's is. That spares the two per-decision lookups on the
//! common path without introducing a second staleness window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use super::parent_chain::ParentChainIndex;
use super::types::{AccessRule, EntityKind, EntityRow};

#[derive(Debug)]
pub(super) struct EntityRules {
    pub(super) entity: Option<EntityRow>,
    pub(super) rules: Vec<AccessRule>,
}

type EntityKey = (EntityKind, String);

#[derive(Debug, Default)]
struct Slot {
    index: Option<Arc<ParentChainIndex>>,
    entries: HashMap<EntityKey, Arc<EntityRules>>,
}

#[derive(Debug, Default)]
pub(super) struct EntityRulesCache {
    slot: Mutex<Slot>,
}

impl EntityRulesCache {
    pub(super) fn get(
        &self,
        index: &Arc<ParentChainIndex>,
        kind: EntityKind,
        id: &str,
    ) -> Option<Arc<EntityRules>> {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        if !slot
            .index
            .as_ref()
            .is_some_and(|held| Arc::ptr_eq(held, index))
        {
            slot.index = Some(Arc::clone(index));
            slot.entries.clear();
            return None;
        }
        slot.entries.get(&(kind, id.to_owned())).cloned()
    }

    pub(super) fn insert(
        &self,
        index: &Arc<ParentChainIndex>,
        kind: EntityKind,
        id: &str,
        value: EntityRules,
    ) -> Arc<EntityRules> {
        let value = Arc::new(value);
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        if slot
            .index
            .as_ref()
            .is_some_and(|held| Arc::ptr_eq(held, index))
        {
            slot.entries
                .insert((kind, id.to_owned()), Arc::clone(&value));
        }
        value
    }
}
