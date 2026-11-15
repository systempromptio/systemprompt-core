//! The deferred and typed entries a member's sink holds: provide fns,
//! member dependencies and jobs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId, type_name};
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Dependencies, DuplicateDependency, Job};

use super::error::ProvideError;
use super::resolved::Resolved;

type ErasedValue = Arc<dyn Any + Send + Sync>;
type ComputeFn = Box<dyn Fn(&Resolved<'_>) -> Result<ErasedValue, ProvideError> + Send + Sync>;
type InsertFn = fn(&mut Dependencies, &ExtensionId, ErasedValue) -> Result<(), DuplicateDependency>;

/// One deferred state value: its type, the provide fn and the monomorphised
/// insert into the extension state.
pub struct ProvideEntry {
    type_id: TypeId,
    type_name: &'static str,
    compute: ComputeFn,
    insert: InsertFn,
}

impl ProvideEntry {
    pub(crate) fn new<T, F>(f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: Fn(&Resolved<'_>) -> Result<T, ProvideError> + Send + Sync + 'static,
    {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
            compute: Box::new(move |r| {
                f(r).map(|value| {
                    let erased: ErasedValue = Arc::new(value);
                    erased
                })
            }),
            insert: insert_provided::<T>,
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

    pub fn compute(&self, r: &Resolved<'_>) -> Result<ProvidedValue, ProvideError> {
        (self.compute)(r).map(|value| ProvidedValue {
            value,
            insert: self.insert,
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
}

impl ProvidedValue {
    pub fn insert_into(
        self,
        state: &mut Dependencies,
        provider: &ExtensionId,
    ) -> Result<(), DuplicateDependency> {
        (self.insert)(state, provider, self.value)
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
    match value.downcast::<T>() {
        Ok(typed) => state.provide(provider, typed),
        Err(_) => Ok(()),
    }
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
        f.debug_tuple("ContributedJob").field(&self.job.name()).finish()
    }
}

