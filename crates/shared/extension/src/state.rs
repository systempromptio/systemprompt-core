//! Typed, extension-scoped state built once at boot.
//!
//! Each extension contributes values in [`Extension::provide`], which the
//! registry runs in dependency order ([`ExtensionRegistry::build_state`]),
//! handing each extension a [`StateBuilder`] scoped to it. The builder records
//! which extension provided each type and refuses a second provider of the
//! same type. While an
//! extension provides, it reads what its dependencies already provided
//! through [`StateBuilder::get`] and [`StateBuilder::require`].
//!
//! The finished [`ExtensionState`] is immutable: a map keyed by [`TypeId`]
//! holding one `Arc<T>` per provided type. The host owns it and hands it to
//! every factory through [`ExtensionContext::extension_state`], so a router,
//! a readiness check or a gateway guard factory reads the same instance the
//! extension built, with no global and no interior mutability.
//!
//! A type an extension cannot run without is declared in
//! [`Extension::requires`]; the registry checks every declaration against the
//! built state before anything serves, so an unlinked provider fails boot
//! rather than a request.
//!
//! [`Extension::provide`]: crate::Extension::provide
//! [`Extension::requires`]: crate::Extension::requires
//! [`ExtensionRegistry::build_state`]: crate::ExtensionRegistry::build_state
//! [`ExtensionContext::extension_state`]: crate::ExtensionContext::extension_state
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;

use crate::error::ExtensionStateError;

struct StateEntry {
    value: Arc<dyn Any + Send + Sync>,
    type_name: &'static str,
    provider: ExtensionId,
}

/// The immutable state every extension provided at boot, keyed by type.
#[derive(Clone, Default)]
pub struct ExtensionState {
    entries: Arc<HashMap<TypeId, StateEntry>>,
}

impl ExtensionState {
    #[must_use]
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        lookup::<T>(&self.entries)
    }

    pub fn require<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, ExtensionStateError> {
        self.get::<T>().ok_or(ExtensionStateError::Missing {
            type_name: type_name::<T>(),
        })
    }

    #[must_use]
    pub fn contains(&self, requirement: &StateRequirement) -> bool {
        self.entries.contains_key(&requirement.type_id)
    }

    #[must_use]
    pub fn provider_of<T: Send + Sync + 'static>(&self) -> Option<&ExtensionId> {
        self.entries
            .get(&TypeId::of::<T>())
            .map(|entry| &entry.provider)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl std::fmt::Debug for ExtensionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut types: Vec<&'static str> = self.entries.values().map(|e| e.type_name).collect();
        types.sort_unstable();
        f.debug_struct("ExtensionState")
            .field("types", &types)
            .finish_non_exhaustive()
    }
}

/// The build-phase view of [`ExtensionState`] handed to
/// [`Extension::provide`](crate::Extension::provide), scoped to the one
/// extension currently providing.
pub struct StateBuilder<'a> {
    entries: &'a mut HashMap<TypeId, StateEntry>,
    current: ExtensionId,
}

#[derive(Default)]
pub(crate) struct StateEntries(HashMap<TypeId, StateEntry>);

impl StateEntries {
    pub(crate) fn builder(&mut self, extension: ExtensionId) -> StateBuilder<'_> {
        StateBuilder {
            entries: &mut self.0,
            current: extension,
        }
    }

    pub(crate) fn finish(self) -> ExtensionState {
        ExtensionState {
            entries: Arc::new(self.0),
        }
    }
}

impl StateBuilder<'_> {
    pub fn provide<T: Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> Result<(), ExtensionStateError> {
        self.provide_arc(Arc::new(value))
    }

    pub fn provide_arc<T: Send + Sync + 'static>(
        &mut self,
        value: Arc<T>,
    ) -> Result<(), ExtensionStateError> {
        let key = TypeId::of::<T>();
        if let Some(existing) = self.entries.get(&key) {
            return Err(ExtensionStateError::DuplicateProvider {
                type_name: type_name::<T>(),
                first: existing.provider.clone(),
                second: self.current.clone(),
            });
        }
        let value: Arc<dyn Any + Send + Sync> = value;
        self.entries.insert(
            key,
            StateEntry {
                value,
                type_name: type_name::<T>(),
                provider: self.current.clone(),
            },
        );
        Ok(())
    }

    #[must_use]
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        lookup::<T>(&*self.entries)
    }

    pub fn require<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, ExtensionStateError> {
        self.get::<T>().ok_or(ExtensionStateError::Missing {
            type_name: type_name::<T>(),
        })
    }

    #[must_use]
    pub const fn extension(&self) -> &ExtensionId {
        &self.current
    }
}

impl std::fmt::Debug for StateBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateBuilder")
            .field("current", &self.current)
            .field("provided", &self.entries.len())
            .finish_non_exhaustive()
    }
}

/// A type an extension declares it cannot run without, checked against the
/// built [`ExtensionState`] at boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateRequirement {
    type_id: TypeId,
    type_name: &'static str,
}

impl StateRequirement {
    #[must_use]
    pub fn of<T: Send + Sync + 'static>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }
}

fn lookup<T: Send + Sync + 'static>(entries: &HashMap<TypeId, StateEntry>) -> Option<Arc<T>> {
    let entry = entries.get(&TypeId::of::<T>())?;
    Arc::clone(&entry.value).downcast::<T>().ok()
}
