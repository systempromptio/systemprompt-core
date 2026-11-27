//! Sealed entries of [`Dependencies`]: storing them, and the copies that
//! withhold them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId};
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;

use super::Dependencies;
use crate::dependency_error::{DependencyProvider, DuplicateDependency};

impl Dependencies {
    #[must_use]
    pub fn with_sealed<T: Any + Send + Sync>(mut self, value: T) -> Self {
        self.insert_sealed(value);
        self
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

    pub fn provide_sealed<T: Any + Send + Sync>(
        &mut self,
        provider: &ExtensionId,
        value: Arc<T>,
    ) -> Result<(), DuplicateDependency> {
        self.provide(provider, value)?;
        Arc::make_mut(&mut self.sealed).insert(TypeId::of::<T>());
        Ok(())
    }
}
