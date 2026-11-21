//! The read side of composition: what a provide fn, a factory, a slot build,
//! a command and a route set's state resolve their dependencies from.
//!
//! A [`Resolved`] is opaque. It reads the built extension state by type
//! ([`Resolved::get`], [`Resolved::optional`]) and hands out the kernel's
//! database handle and background-task owner; it is built only by the
//! runtime's single [`KernelAccess`] implementation and, for a route set, by
//! the API mount.
//!
//! A seed the host inserted sealed (secrets, the runtime `Config`, boot-only
//! data) is served to a provide fn, a factory, a slot build, a kernel command
//! and a route set; the resolver a member's command runs against
//! ([`Resolved::__member_command`]) refuses it with the same
//! `MissingDependency` an absent type gets. A value a member provided from a
//! sealed seed is the member's own and stays readable; a value a member
//! provided sealed is withheld from that resolver too, and from the state of a
//! route set another member owns ([`Resolved::__route_scoped`] names the
//! set's owner).
//!
//! A kernel handle is not reachable through a public method. It is read only
//! through [`Handed<K>`]'s [`FromState`] impl, which consults the resolution's
//! private conferred-handle list. The list is empty for every resolution but
//! the one the mount builds for a route set whose base carries a
//! `KernelHandle(K::ID)` conferral for that set's owner, so a provide fn, a
//! factory, a slot build, a command or a route set at an unconferred base
//! gets `MissingDependency`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::Any;
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Dependencies, DependencyProvider, MissingDependency};
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

use super::error::{FactoryError, ProvideError, SlotError};

/// What the kernel exposes to a resolution; implemented once, in the runtime.
#[doc(hidden)]
pub trait KernelAccess: Send + Sync {
    fn database(&self) -> Arc<dyn DatabaseHandle>;
    fn tasks(&self) -> &BackgroundTasks;
    fn handle(&self, id: HandleId) -> Option<Arc<dyn Any + Send + Sync>>;
}

pub struct Resolved<'a> {
    access: &'a dyn KernelAccess,
    state: &'a Dependencies,
    conferred: &'a [HandleId],
    withhold_sealed: bool,
    viewer: Option<&'a ExtensionId>,
}

impl<'a> Resolved<'a> {
    #[doc(hidden)]
    pub const fn __new(access: &'a dyn KernelAccess, state: &'a Dependencies) -> Self {
        Self {
            access,
            state,
            conferred: &[],
            withhold_sealed: false,
            viewer: None,
        }
    }

    #[doc(hidden)]
    pub const fn __member_command(access: &'a dyn KernelAccess, state: &'a Dependencies) -> Self {
        Self {
            access,
            state,
            conferred: &[],
            withhold_sealed: true,
            viewer: None,
        }
    }

    #[doc(hidden)]
    pub const fn __route_scoped(
        access: &'a dyn KernelAccess,
        state: &'a Dependencies,
        owner: &'a ExtensionId,
        conferred: &'a [HandleId],
    ) -> Self {
        Self {
            access,
            state,
            conferred,
            withhold_sealed: false,
            viewer: Some(owner),
        }
    }
}

impl Resolved<'_> {
    pub fn get<T: Clone + Send + Sync + 'static>(&self) -> Result<T, MissingDependency> {
        self.optional::<T>().ok_or_else(MissingDependency::of::<T>)
    }

    pub fn optional<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
        if self.state.is_sealed::<T>() && self.withholds_sealed::<T>() {
            return None;
        }
        self.state.find::<T>().cloned()
    }

    fn withholds_sealed<T: Send + Sync + 'static>(&self) -> bool {
        if self.withhold_sealed {
            return true;
        }
        match (self.viewer, self.state.provider_of::<T>()) {
            (Some(viewer), Some(DependencyProvider::Extension(provider))) => provider != viewer,
            _ => false,
        }
    }

    pub fn db(&self) -> Arc<dyn DatabaseHandle> {
        self.access.database()
    }

    pub fn tasks(&self) -> &BackgroundTasks {
        self.access.tasks()
    }

    fn handed<K: KernelHandle>(&self) -> Result<K::Value, MissingDependency> {
        if !self.conferred.contains(&K::ID) {
            return Err(MissingDependency::of::<K::Value>());
        }
        self.access
            .handle(K::ID)
            .and_then(|value| value.downcast_ref::<K::Value>().cloned())
            .ok_or_else(MissingDependency::of::<K::Value>)
    }
}

impl std::fmt::Debug for Resolved<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resolved")
            .field("state", self.state)
            .field("conferred", &self.conferred)
            .field("withhold_sealed", &self.withhold_sealed)
            .field("viewer", &self.viewer)
            .finish_non_exhaustive()
    }
}

/// A value resolved from the built extension state; written by hand for every
/// route set's state type.
pub trait FromState: Sized + Send + Sync + 'static {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency>;
}

/// A deferred provider of one state value, run at admission in composition
/// order.
pub trait ProvideFn<T>:
    Fn(&Resolved<'_>) -> Result<T, ProvideError> + Send + Sync + 'static
{
}

impl<T, F> ProvideFn<T> for F where
    F: Fn(&Resolved<'_>) -> Result<T, ProvideError> + Send + Sync + 'static
{
}

/// A deferred constructor of one contributed service, run after admission.
pub trait Factory<T>: Send + Sync + 'static {
    fn build(&self, r: &Resolved<'_>) -> Result<T, FactoryError>;
}

impl<T, F> Factory<T> for F
where
    F: Fn(&Resolved<'_>) -> Result<T, FactoryError> + Send + Sync + 'static,
{
    fn build(&self, r: &Resolved<'_>) -> Result<T, FactoryError> {
        self(r)
    }
}

pub fn build_factories<T: 'static>(
    slot: &'static str,
    owner: &ExtensionId,
    factories: &[Box<dyn Factory<T>>],
    r: &Resolved<'_>,
) -> Result<Vec<T>, SlotError> {
    factories
        .iter()
        .map(|factory| {
            factory
                .build(r)
                .map_err(|error| error.into_slot_error(slot, owner))
        })
        .collect()
}

/// A kernel-built value handed to exactly the routes it is conferred on.
pub trait KernelHandle: 'static {
    type Value: Clone + Send + Sync + 'static;
    const ID: HandleId;
}

pub struct Handed<K: KernelHandle>(pub K::Value);

impl<K: KernelHandle> Clone for Handed<K> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<K: KernelHandle> FromState for Handed<K> {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        r.handed::<K>().map(Self)
    }
}

impl<K: KernelHandle> std::fmt::Debug for Handed<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Handed").field(&K::ID).finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandleId {
    AnalyticsBroadcaster,
}
