//! The sealed, core-first slot the supervised process sets are contributed
//! through.
//!
//! The agent and MCP members each add one `DynServiceProcessSet` factory with
//! `c.slot::<ServiceProcessSlot>().process_set(..)`. Factories run at slot
//! build, where sealed seeds (signing keys, the secret store) are readable, so
//! a set may hold what starting a process needs. The built
//! [`ServiceProcessSets`] is provided sealed: only kernel code reads it, and an
//! install's entry is never built when no core member opens the slot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{DynServiceProcessSet, ServiceProcessKind};

use crate::{Factory, Resolved, Slot, SlotEntry, SlotError, SlotOrder, build_factories};

const SERVICE_PROCESS_SLOT: &str = "service_process";

/// The supervised process sets of the admitted members.
#[derive(Debug, Clone, Copy)]
pub struct ServiceProcessSlot;

#[derive(Default)]
pub struct ServiceProcessBuilder {
    sets: Vec<Box<dyn Factory<DynServiceProcessSet>>>,
}

impl std::fmt::Debug for ServiceProcessBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceProcessBuilder")
            .field("sets", &self.sets.len())
            .finish()
    }
}

impl ServiceProcessBuilder {
    pub fn process_set<F: Factory<DynServiceProcessSet>>(&mut self, f: F) -> &mut Self {
        self.sets.push(Box::new(f));
        self
    }
}

/// Every built process set, in core-first composition order.
#[derive(Clone, Default)]
pub struct ServiceProcessSets(Vec<DynServiceProcessSet>);

impl std::fmt::Debug for ServiceProcessSets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(|set| set.kind()))
            .finish()
    }
}

impl ServiceProcessSets {
    #[must_use]
    pub fn sets(&self) -> &[DynServiceProcessSet] {
        &self.0
    }

    #[must_use]
    pub fn of_kind(&self, kind: ServiceProcessKind) -> Option<&DynServiceProcessSet> {
        self.0.iter().find(|set| set.kind() == kind)
    }
}

impl Slot for ServiceProcessSlot {
    type Builder = ServiceProcessBuilder;
    type Built = ServiceProcessSets;
    const ORDER: SlotOrder = SlotOrder::CoreFirst;
    const SEALED: bool = true;
    const LATE: bool = true;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut built = Vec::new();
        for entry in entries {
            built.extend(build_factories(
                SERVICE_PROCESS_SLOT,
                &entry.scope(),
                &entry.builder.sets,
                r,
            )?);
        }
        Ok(ServiceProcessSets(built))
    }
}
