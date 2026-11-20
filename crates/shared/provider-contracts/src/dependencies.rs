//! [`Dependencies`]: the one type-keyed map of host handles and
//! extension-provided state.
//!
//! This crate sits below every crate that owns a concrete handle — the
//! database pool (`systemprompt-database`), the runtime `AppContext`
//! (`systemprompt-runtime`), `AppPaths` (`systemprompt-config`) and the
//! content configuration (`systemprompt-models`) — so the contexts cannot
//! name those types. Each value is keyed by its concrete type, and a reader
//! asks for exactly that type with [`Dependencies::get`] (a borrow) or
//! [`Dependencies::get_arc`] (shared ownership). A type that is not present
//! is a [`MissingDependency`] naming the requested type; it is never a panic
//! or a silent `None`. A reader for which absence is a normal state (an
//! optional collaborator such as a configured `GeoIP` database) asks with
//! [`Dependencies::find`], whose `Option` states that in the signature.
//!
//! Every entry records its [`DependencyProvider`]. The host seeds the map
//! with [`Dependencies::insert`] (a later insert of the same type replaces
//! the earlier one); an extension contributes through
//! [`Dependencies::provide`], which refuses a type that is already present,
//! whoever provided it, with a [`DuplicateDependency`] naming both
//! providers. The extension registry builds the boot-time extension state
//! this way: the kernel's handles first, then every enabled extension in
//! dependency order.
//!
//! The host can also [`Dependencies::reserve`] a type it owns but holds no
//! value for (a service that is absent because it is not configured): a
//! reserved type reads as missing, does not satisfy a requirement, and an
//! extension's `provide` of it is refused as a duplicate of the host's.
//!
//! The host can also [`Dependencies::insert_sealed`] a seed that carries
//! secrets or boot-only data. A sealed entry is read like any other through
//! this map; [`Dependencies::is_sealed`] lets the one reader that must
//! withhold it refuse it with the same [`MissingDependency`] an absent type
//! gets: the resolver a member's command runs against checks it per read,
//! and [`Dependencies::withholding_sealed`] returns a copy in which every
//! sealed entry is removed and its type reserved, so neither a read nor a
//! later `provide` reaches it. A job, runner-service and config-plane
//! context is built only from such a copy. Whether a type reaches member
//! runtime code is decided here and nowhere else: a value carrying signing
//! material, secrets or the profile is sealed in the kernel state, and a
//! member that needs a capability derived from one builds a narrow handle
//! from it in its `provide` fn or factory. [`Dependencies::sealed_types`]
//! enumerates the sealed set, so a check over every context sees a type the
//! moment it is sealed.
//!
//! The map is copy-on-write: a clone shares every entry, and inserting into a
//! clone never changes the original, so the state the host owns after boot
//! is immutable while a job or prerender context can extend its own copy.
//!
//! | Context | Seeded from |
//! |---------|-------------|
//! | `Resolved` (provide fns, factories, slots, route sets, commands) | kernel handles + every enabled extension's `provide` |
//! | `JobContext` (scheduler, runner services) | the extension state without its sealed seeds + per-run handles |
//! | config-plane context | the extension state without its sealed seeds + per-call handles |
//! | prerender contexts | the extension state + per-call handles |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId, type_name};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use thiserror::Error;

/// Who put an entry into a [`Dependencies`] map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyProvider {
    Host,
    Extension(ExtensionId),
}

impl fmt::Display for DependencyProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host => f.write_str("the host"),
            Self::Extension(id) => write!(f, "extension '{id}'"),
        }
    }
}

#[derive(Clone)]
struct Entry {
    type_name: &'static str,
    value: Arc<dyn Any + Send + Sync>,
    provider: DependencyProvider,
}

/// Values keyed by concrete type; `get::<Arc<T>>()` and `get::<T>()` are
/// different keys.
#[derive(Clone, Default)]
pub struct Dependencies {
    entries: Arc<HashMap<TypeId, Entry>>,
    reserved: Arc<HashMap<TypeId, &'static str>>,
    sealed: Arc<HashSet<TypeId>>,
}

impl Dependencies {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with<T: Any + Send + Sync>(mut self, value: T) -> Self {
        self.insert(value);
        self
    }

    #[must_use]
    pub fn with_sealed<T: Any + Send + Sync>(mut self, value: T) -> Self {
        self.insert_sealed(value);
        self
    }

    #[must_use]
    pub fn reserving<T: Any + Send + Sync>(mut self) -> Self {
        self.reserve::<T>();
        self
    }

    pub fn reserve<T: Any + Send + Sync>(&mut self) {
        Arc::make_mut(&mut self.reserved).insert(TypeId::of::<T>(), type_name::<T>());
    }

    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) {
        Arc::make_mut(&mut self.entries).insert(
            TypeId::of::<T>(),
            Entry {
                type_name: type_name::<T>(),
                value: Arc::new(value),
                provider: DependencyProvider::Host,
            },
        );
    }

    pub fn insert_sealed<T: Any + Send + Sync>(&mut self, value: T) {
        self.insert(value);
        Arc::make_mut(&mut self.sealed).insert(TypeId::of::<T>());
    }

    #[must_use]
    pub fn withholding_sealed(mut self) -> Self {
        if self.sealed.is_empty() {
            return self;
        }
        let entries = Arc::make_mut(&mut self.entries);
        let reserved = Arc::make_mut(&mut self.reserved);
        for id in self.sealed.iter() {
            if let Some(entry) = entries.remove(id) {
                reserved.insert(*id, entry.type_name);
            }
        }
        self
    }

    #[must_use]
    pub fn is_sealed<T: Any + Send + Sync>(&self) -> bool {
        self.sealed.contains(&TypeId::of::<T>())
    }

    pub fn sealed_types(&self) -> impl Iterator<Item = (TypeId, &'static str)> + '_ {
        self.sealed.iter().filter_map(|id| {
            self.entries
                .get(id)
                .map(|e| e.type_name)
                .or_else(|| self.reserved.get(id).copied())
                .map(|name| (*id, name))
        })
    }

    pub fn provide<T: Any + Send + Sync>(
        &mut self,
        provider: &ExtensionId,
        value: Arc<T>,
    ) -> Result<(), DuplicateDependency> {
        let key = TypeId::of::<T>();
        if let Some(existing) = self.entries.get(&key) {
            return Err(DuplicateDependency {
                type_name: type_name::<T>(),
                first: existing.provider.clone(),
                second: provider.clone(),
            });
        }
        if self.reserved.contains_key(&key) {
            return Err(DuplicateDependency {
                type_name: type_name::<T>(),
                first: DependencyProvider::Host,
                second: provider.clone(),
            });
        }
        let value: Arc<dyn Any + Send + Sync> = value;
        Arc::make_mut(&mut self.entries).insert(
            key,
            Entry {
                type_name: type_name::<T>(),
                value,
                provider: DependencyProvider::Extension(provider.clone()),
            },
        );
        Ok(())
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Result<&T, MissingDependency> {
        self.find::<T>().ok_or_else(MissingDependency::of::<T>)
    }

    #[must_use]
    pub fn find<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.entries
            .get(&TypeId::of::<T>())
            .and_then(|entry| entry.value.downcast_ref::<T>())
    }

    pub fn get_arc<T: Any + Send + Sync>(&self) -> Result<Arc<T>, MissingDependency> {
        self.entries
            .get(&TypeId::of::<T>())
            .and_then(|entry| Arc::clone(&entry.value).downcast::<T>().ok())
            .ok_or_else(MissingDependency::of::<T>)
    }

    #[must_use]
    pub fn contains_type(&self, type_id: TypeId) -> bool {
        self.entries.contains_key(&type_id)
    }

    #[must_use]
    pub fn provider_of<T: Any + Send + Sync>(&self) -> Option<&DependencyProvider> {
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

impl fmt::Debug for Dependencies {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names: Vec<&str> = self.entries.values().map(|e| e.type_name).collect();
        names.sort_unstable();
        let mut reserved: Vec<&str> = self.reserved.values().copied().collect();
        reserved.sort_unstable();
        let mut sealed: Vec<&str> = self.sealed_types().map(|(_, name)| name).collect();
        sealed.sort_unstable();
        f.debug_struct("Dependencies")
            .field("types", &names)
            .field("reserved", &reserved)
            .field("sealed", &sealed)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("provider context has no {type_name} dependency")]
pub struct MissingDependency {
    type_name: &'static str,
}

impl MissingDependency {
    #[must_use]
    pub fn of<T: Any>() -> Self {
        Self {
            type_name: type_name::<T>(),
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "{type_name} is already provided by {first}; extension '{second}' cannot provide it again, \
     every type has exactly one provider"
)]
pub struct DuplicateDependency {
    pub type_name: &'static str,
    pub first: DependencyProvider,
    pub second: ExtensionId,
}
