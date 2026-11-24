//! The deferred and typed entries a member's sink holds: provide fns,
//! member dependencies and jobs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId, type_name};
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Conferred, Dependencies, DuplicateDependency, Job};

use super::error::ProvideError;
use super::resolved::Resolved;
use crate::reserved::ConferralRow;

type ErasedValue = Arc<dyn Any + Send + Sync>;
type ComputeFn = Box<dyn Fn(&Resolved<'_>) -> Result<ErasedValue, ProvideError> + Send + Sync>;
type InsertFn = fn(&mut Dependencies, &ExtensionId, ErasedValue) -> Result<(), DuplicateDependency>;
type ConferFn =
    fn(&mut Dependencies, &ExtensionId, ErasedValue, Conferred) -> Result<(), DuplicateDependency>;
type ReserveFn = fn(&mut Dependencies);

/// When the kernel runs a provide fn.
///
/// `Early` provide fns run in composition order before any slot is built.
/// `AfterGovernance` provide fns run once every slot, the subject providers,
/// the policy catalog and the gateway guards are built, so they may read
/// `Arc<SubjectProviderSet>`, `Arc<PolicyCatalog>` and every slot-built
/// value; no slot build or governance factory may read their values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvidePhase {
    Early,
    AfterGovernance,
}

/// One deferred state value: its type, the provide fn, the phase it runs in
/// and the monomorphised insert into the extension state.
pub struct ProvideEntry {
    phase: ProvidePhase,
    sealed: bool,
    conferral: Option<(ConferralRow, ConferFn)>,
    type_id: TypeId,
    type_name: &'static str,
    compute: ComputeFn,
    insert: InsertFn,
    reserve: ReserveFn,
}

impl ProvideEntry {
    #[must_use]
    pub const fn phase(&self) -> ProvidePhase {
        self.phase
    }

    #[must_use]
    pub const fn is_sealed(&self) -> bool {
        self.sealed
    }

    #[must_use]
    pub fn conferral(&self) -> Option<ConferralRow> {
        self.conferral.map(|(row, _)| row)
    }

    pub(crate) fn conferred<T, F>(phase: ProvidePhase, row: ConferralRow, f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: Fn(&Resolved<'_>) -> Result<T, ProvideError> + Send + Sync + 'static,
    {
        let mut entry = Self::new::<T, F>(phase, true, f);
        entry.conferral = Some((row, insert_conferred::<T>));
        entry
    }

    pub(crate) fn new<T, F>(phase: ProvidePhase, sealed: bool, f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: Fn(&Resolved<'_>) -> Result<T, ProvideError> + Send + Sync + 'static,
    {
        Self {
            phase,
            sealed,
            conferral: None,
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
            compute: Box::new(move |r| {
                f(r).map(|value| {
                    let erased: ErasedValue = Arc::new(value);
                    erased
                })
            }),
            insert: if sealed {
                insert_sealed::<T>
            } else {
                insert_provided::<T>
            },
            reserve: Dependencies::reserve::<T>,
        }
    }

    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn withhold_into(&self, state: &mut Dependencies) {
        (self.reserve)(state);
    }

    pub fn compute(&self, r: &Resolved<'_>) -> Result<ProvidedValue, ProvideError> {
        (self.compute)(r).map(|value| ProvidedValue {
            value,
            insert: self.insert,
            conferral: self.conferral,
        })
    }
}

impl std::fmt::Debug for ProvideEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProvideEntry")
            .field("type", &self.type_name)
            .finish_non_exhaustive()
    }
}

/// A computed state value, held until it is inserted under its provider.
pub struct ProvidedValue {
    value: ErasedValue,
    insert: InsertFn,
    conferral: Option<(ConferralRow, ConferFn)>,
}

impl ProvidedValue {
    pub fn insert_into(
        self,
        state: &mut Dependencies,
        provider: &ExtensionId,
    ) -> Result<(), DuplicateDependency> {
        match self.conferral {
            Some((row, confer)) => confer(
                state,
                provider,
                self.value,
                Conferred {
                    holder: ExtensionId::new(row.entry().holder.id()),
                    row: row.name(),
                },
            ),
            None => (self.insert)(state, provider, self.value),
        }
    }
}

impl std::fmt::Debug for ProvidedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProvidedValue").finish_non_exhaustive()
    }
}

fn insert_provided<T: Send + Sync + 'static>(
    state: &mut Dependencies,
    provider: &ExtensionId,
    value: ErasedValue,
) -> Result<(), DuplicateDependency> {
    value
        .downcast::<T>()
        .map_or(Ok(()), |typed| state.provide(provider, typed))
}

fn insert_sealed<T: Send + Sync + 'static>(
    state: &mut Dependencies,
    provider: &ExtensionId,
    value: ErasedValue,
) -> Result<(), DuplicateDependency> {
    value
        .downcast::<T>()
        .map_or(Ok(()), |typed| state.provide_sealed(provider, typed))
}

fn insert_conferred<T: Send + Sync + 'static>(
    state: &mut Dependencies,
    provider: &ExtensionId,
    value: ErasedValue,
    conferred: Conferred,
) -> Result<(), DuplicateDependency> {
    value.downcast::<T>().map_or(Ok(()), |typed| {
        state.provide_conferred(provider, typed, conferred)
    })
}

/// A declared dependency on another member of the same capability, by type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberDependency {
    type_id: TypeId,
    type_name: &'static str,
}

impl MemberDependency {
    pub(crate) fn of<E: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<E>(),
            type_name: type_name::<E>(),
        }
    }

    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }
}

/// A contributed job and its type, which projection drains bind to.
#[derive(Clone)]
pub struct ContributedJob {
    type_id: TypeId,
    job: Arc<dyn Job>,
}

impl ContributedJob {
    pub(crate) fn new<J: Job + 'static>(job: J) -> Self {
        Self {
            type_id: TypeId::of::<J>(),
            job: Arc::new(job),
        }
    }

    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[must_use]
    pub const fn job(&self) -> &Arc<dyn Job> {
        &self.job
    }
}

impl std::fmt::Debug for ContributedJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ContributedJob")
            .field(&self.job.name())
            .finish()
    }
}
