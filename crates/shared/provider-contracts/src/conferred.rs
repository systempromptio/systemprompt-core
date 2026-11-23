//! Conferral records on [`Dependencies`]: which one core holder a sealed value
//! is conferred on, under which reserved row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId};
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;

use crate::dependencies::Dependencies;
use crate::dependency_errors::DuplicateDependency;

/// The one member a sealed value is conferred on, and the reserved row that
/// names the pair; the value is a member's sealed provide or a sealed host
/// seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conferred {
    pub holder: ExtensionId,
    pub row: &'static str,
}

impl Dependencies {
    #[must_use]
    pub fn with_sealed_conferred<T: Any + Send + Sync>(
        mut self,
        value: T,
        conferred: Conferred,
    ) -> Self {
        self.insert_sealed(value);
        Arc::make_mut(&mut self.conferred).insert(TypeId::of::<T>(), conferred);
        self
    }

    pub fn provide_conferred<T: Any + Send + Sync>(
        &mut self,
        provider: &ExtensionId,
        value: Arc<T>,
        conferred: Conferred,
    ) -> Result<(), DuplicateDependency> {
        self.provide_sealed(provider, value)?;
        Arc::make_mut(&mut self.conferred).insert(TypeId::of::<T>(), conferred);
        Ok(())
    }

    #[must_use]
    pub fn conferral_of<T: Any + Send + Sync>(&self) -> Option<&Conferred> {
        self.conferred.get(&TypeId::of::<T>())
    }
}
