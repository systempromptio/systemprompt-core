//! [`Dependencies`]: the host handles a job or prerender context carries.
//!
//! This crate sits below every crate that owns a concrete handle — the
//! database pool (`systemprompt-database`), the runtime `AppContext`
//! (`systemprompt-runtime`), `AppPaths` (`systemprompt-config`) and the
//! content configuration (`systemprompt-models`) — so the contexts cannot
//! name those types. The host inserts each handle once, keyed by its concrete
//! type, and a provider asks for exactly that type with
//! [`Dependencies::get`]. A handle the host did not insert is a
//! [`MissingDependency`] naming the requested type; it is never a panic or a
//! silent `None`.
//!
//! The handles the platform's own hosts insert:
//!
//! | Context | Types |
//! |---------|-------|
//! | `JobContext` (scheduler) | `DbPool`, `Arc<AppContext>`, `Arc<AppPaths>` |
//! | prerender contexts (generator) | `DbPool`, `ContentConfigRaw` |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use thiserror::Error;

#[derive(Clone)]
struct Entry {
    type_name: &'static str,
    value: Arc<dyn Any + Send + Sync>,
}

/// Host handles keyed by concrete type; inserting a second value of a type
/// replaces the first, and `get::<Arc<T>>()` and `get::<T>()` are different
/// keys.
#[derive(Clone, Default)]
pub struct Dependencies {
    entries: HashMap<TypeId, Entry>,
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

    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) {
        self.entries.insert(
            TypeId::of::<T>(),
            Entry {
                type_name: type_name::<T>(),
                value: Arc::new(value),
            },
        );
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Result<&T, MissingDependency> {
        self.entries
            .get(&TypeId::of::<T>())
            .and_then(|entry| entry.value.downcast_ref::<T>())
            .ok_or_else(|| MissingDependency {
                type_name: type_name::<T>(),
            })
    }
}

impl fmt::Debug for Dependencies {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names: Vec<&str> = self.entries.values().map(|e| e.type_name).collect();
        names.sort_unstable();
        f.debug_struct("Dependencies")
            .field("types", &names)
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
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }
}
