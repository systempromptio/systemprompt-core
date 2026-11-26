//! [`Dependencies`]: the one type-keyed map of host handles and
//! extension-provided state.
//!
//! This crate sits below every crate that owns a concrete handle, so the
//! contexts cannot name those types. Each value is keyed by its concrete
//! type and read with [`Dependencies::get`] (a borrow) or
//! [`Dependencies::get_arc`] (shared ownership); an absent type is a
//! [`MissingDependency`] naming it, never a panic or a silent `None`. A
//! reader for which absence is normal asks with [`Dependencies::find`].
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
//! value for: it reads as missing, does not satisfy a requirement, and an
//! extension's `provide` of it is refused as a duplicate of the host's.
//!
//! A sealed entry ([`Dependencies::insert_sealed`] for a host seed carrying
//! secrets or boot-only data, [`Dependencies::provide_sealed`] for a member
//! value that acts with standing authority) is read like any other through
//! this map. [`Dependencies::is_sealed`] lets a resolver refuse it per read
//! with the same [`MissingDependency`] an absent type gets, and
//! [`Dependencies::withholding_sealed`] returns a copy with every sealed
//! entry removed and its type reserved; every runner-service and
//! config-plane context is built only from such a copy.
//! [`Dependencies::withholding_sealed_except`] keeps the sealed entries one
//! member provided and withholds every other: a job context is built from
//! that copy for the job's owning member, so a member's job reads its own
//! sealed values and never another member's or a host seed. A member that needs
//! a capability derived from a sealed value builds a narrow handle from it in
//! its `provide` fn or factory. [`Dependencies::sealed_types`] enumerates
//! the sealed set; [`Dependencies::host_types`] names the host's seeds.
//!
//! The map is copy-on-write: a clone shares every entry, and inserting into a
//! clone never changes the original, so the state the host owns after boot
//! is immutable while a job or prerender context can extend its own copy.
//!
//! | Context | Seeded from |
//! |---------|-------------|
//! | `Resolved` (provide fns, factories, slots, route sets, commands) | kernel handles + every enabled extension's `provide` |
//! | `JobContext` (scheduler) | the extension state without every sealed entry but the job owner's own + per-run handles |
//! | `JobContext` (runner services) | the extension state without its sealed seeds + per-run handles |
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

use crate::conferred::Conferred;
use crate::dependency_error::{DependencyProvider, DuplicateDependency, MissingDependency};

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
    withheld: Arc<HashSet<TypeId>>,
    sealed: Arc<HashSet<TypeId>>,
    pub(crate) conferred: Arc<HashMap<TypeId, Conferred>>,
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

    pub fn withhold<T: Any + Send + Sync>(&mut self) {
        self.reserve::<T>();
        Arc::make_mut(&mut self.withheld).insert(TypeId::of::<T>());
    }

    #[must_use]
    pub fn is_withheld<T: Any + Send + Sync>(&self) -> bool {
        self.withheld.contains(&TypeId::of::<T>())
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
    pub fn withholding_sealed_except(mut self, owner: &ExtensionId) -> Self {
        if self.sealed.is_empty() {
            return self;
        }
        let entries = Arc::make_mut(&mut self.entries);
        let reserved = Arc::make_mut(&mut self.reserved);
        for id in self.sealed.iter() {
            let own = entries.get(id).is_some_and(|entry| {
                matches!(&entry.provider, DependencyProvider::Extension(provider) if provider == owner)
            });
            if own {
                continue;
            }
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

    pub fn host_types(&self) -> impl Iterator<Item = (TypeId, &'static str)> + '_ {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.provider == DependencyProvider::Host)
            .map(|(id, entry)| (*id, entry.type_name))
            .chain(self.reserved.iter().map(|(id, name)| (*id, *name)))
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

    pub fn provide_sealed<T: Any + Send + Sync>(
        &mut self,
        provider: &ExtensionId,
        value: Arc<T>,
    ) -> Result<(), DuplicateDependency> {
        self.provide(provider, value)?;
        Arc::make_mut(&mut self.sealed).insert(TypeId::of::<T>());
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
        let mut conferred: Vec<&str> = self.conferred.values().map(|c| c.row).collect();
        conferred.sort_unstable();
        f.debug_struct("Dependencies")
            .field("types", &names)
            .field("reserved", &reserved)
            .field("sealed", &sealed)
            .field("conferred", &conferred)
            .finish()
    }
}
