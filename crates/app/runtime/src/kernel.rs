//! The slim runtime kernel the admitted composition resolves against.
//!
//! A [`Kernel`] holds the database handle, the process's background-task
//! owner, the kernel handles a conferral can hand a route set, and the
//! extension state. [`Kernel::resolve`] seeds that state with the admitted
//! members' projection drains ([`AdmittedProjections`]), then builds it from
//! the [`Admitted`] handoff in three passes: every admitted member's provide
//! fns in composition order, each reading what was provided before it; every
//! member's declared requirements, checked against the finished state; and
//! every slot, built from its admitted entries (core first where the slot
//! asks for it) and inserted as `Arc<S::Built>`. Only admitted members'
//! entries are present, so nothing a disabled member contributed runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_extension::contribute::{SlotBuildInput, build_slot};
use systemprompt_extension::{
    Admitted, AdmittedProjections, HandleId, KernelHandle, ProvideError, Resolved, SlotError,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Dependencies, DuplicateDependency};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    #[error("{owner} failed to provide {ty}")]
    Provide {
        owner: ExtensionId,
        ty: &'static str,
        #[source]
        source: ProvideError,
    },
    #[error("state value provided twice")]
    Duplicate(#[from] DuplicateDependency),
    #[error("{owner} requires {ty}, which no enabled member or the kernel provides")]
    Missing {
        owner: ExtensionId,
        ty: &'static str,
    },
    #[error("slot build failed")]
    Slot(#[from] SlotError),
}

pub struct Kernel {
    pub(crate) database: Arc<dyn DatabaseHandle>,
    pub(crate) tasks: BackgroundTasks,
    pub(crate) handles: HashMap<HandleId, Arc<dyn Any + Send + Sync>>,
    state: Dependencies,
}

impl std::fmt::Debug for Kernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kernel")
            .field("handles", &self.handles.keys().collect::<Vec<_>>())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Kernel {
    #[must_use]
    pub fn new(
        database: Arc<dyn DatabaseHandle>,
        tasks: BackgroundTasks,
        seed: Dependencies,
    ) -> Self {
        Self {
            database,
            tasks,
            handles: HashMap::new(),
            state: seed,
        }
    }

    #[must_use]
    pub fn with_handle<K: KernelHandle>(mut self, value: K::Value) -> Self {
        self.handles.insert(K::ID, Arc::new(value));
        self
    }

    pub fn resolve(&mut self, admitted: &Admitted) -> Result<(), KernelError> {
        self.state.insert(AdmittedProjections::of(admitted.built()));
        self.provide_all(admitted)?;
        self.check_requirements(admitted)?;
        self.build_slots(admitted)
    }

    #[must_use]
    pub fn resolved(&self) -> Resolved<'_> {
        Resolved::__new(self, &self.state)
    }

    #[must_use]
    pub fn route_scoped<'a>(&'a self, conferred: &'a [HandleId]) -> Resolved<'a> {
        Resolved::__route_scoped(self, &self.state, conferred)
    }

    #[must_use]
    pub const fn state(&self) -> &Dependencies {
        &self.state
    }

    fn provide_all(&mut self, admitted: &Admitted) -> Result<(), KernelError> {
        for member in admitted.built().members() {
            for entry in member.sink.provides() {
                let value = entry
                    .compute(&Resolved::__new(&*self, &self.state))
                    .map_err(|source| KernelError::Provide {
                        owner: member.owner.clone(),
                        ty: entry.type_name(),
                        source,
                    })?;
                value.insert_into(&mut self.state, &member.owner)?;
            }
        }
        Ok(())
    }

    fn check_requirements(&self, admitted: &Admitted) -> Result<(), KernelError> {
        for member in admitted.built().members() {
            if let Some(unmet) = member
                .sink
                .requires()
                .iter()
                .find(|requirement| !requirement.is_met_by(&self.state))
            {
                return Err(KernelError::Missing {
                    owner: member.owner.clone(),
                    ty: unmet.type_name(),
                });
            }
        }
        Ok(())
    }

    fn build_slots(&mut self, admitted: &Admitted) -> Result<(), KernelError> {
        let mut order: Vec<TypeId> = Vec::new();
        let mut grouped: HashMap<TypeId, Vec<SlotBuildInput>> = HashMap::new();
        for member in admitted.built().members() {
            for record in member.sink.slots() {
                if let Some(input) = record.take_entry(member.owner.clone(), member.standing) {
                    if !grouped.contains_key(&input.slot()) {
                        order.push(input.slot());
                    }
                    grouped.entry(input.slot()).or_default().push(input);
                }
            }
        }
        for slot in order {
            let inputs = grouped.remove(&slot).unwrap_or_default();
            let built = build_slot(inputs, &Resolved::__new(&*self, &self.state))?;
            if let Some(built) = built {
                built.insert_into(&mut self.state);
            }
        }
        Ok(())
    }
}
