//! Dynamic extension registry that stores extensions as `Arc<dyn
//! Extension>`.
//!
//! The dynamic registry is the lower-level counterpart of
//! [`crate::TypedExtensionRegistry`]: it accepts `Arc<dyn Extension>`
//! values supplied by either inventory discovery or runtime injection.

mod discovery;
mod queries;
mod validation;

use crate::Extension;
use crate::error::LoaderError;
use std::collections::HashMap;
use std::sync::Arc;

pub use validation::RESERVED_PATHS;

/// Dynamic registry of `Arc<dyn Extension>` values, indexed by ID and
/// kept in priority order.
#[derive(Default)]
pub struct ExtensionRegistry {
    pub(crate) extensions: HashMap<String, Arc<dyn Extension>>,
    pub(crate) sorted_extensions: Vec<Arc<dyn Extension>>,
}

impl std::fmt::Debug for ExtensionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionRegistry")
            .field("extension_count", &self.extensions.len())
            .finish_non_exhaustive()
    }
}

impl ExtensionRegistry {
    /// Constructs an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn sort_by_priority(&mut self) {
        self.sorted_extensions.sort_by_key(|e| e.priority());
    }

    /// Registers a single extension. Fails if its ID is already present.
    pub fn register(&mut self, ext: Arc<dyn Extension>) -> Result<(), LoaderError> {
        let id = ext.id().to_string();
        if self.extensions.contains_key(&id) {
            return Err(LoaderError::DuplicateExtension(id));
        }
        self.extensions.insert(id, Arc::clone(&ext));
        self.sorted_extensions.push(ext);
        self.sort_by_priority();
        Ok(())
    }

    /// Registers a batch of extensions, stopping at the first duplicate.
    pub fn merge(&mut self, extensions: Vec<Arc<dyn Extension>>) -> Result<(), LoaderError> {
        for ext in extensions {
            self.register(ext)?;
        }
        Ok(())
    }

    /// Validates the registry: dependency resolution and cycle detection.
    pub fn validate(&self) -> Result<(), LoaderError> {
        self.validate_dependencies()?;
        Ok(())
    }

    /// Returns the number of registered extensions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.extensions.len()
    }

    /// Returns true if no extensions are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }
}

/// `inventory`-collected registration hook. Each extension that uses
/// [`crate::register_extension!`] submits one of these.
#[derive(Debug, Clone, Copy)]
pub struct ExtensionRegistration {
    /// Factory function that produces the registered extension instance.
    pub factory: fn() -> Arc<dyn Extension>,
}

inventory::collect!(ExtensionRegistration);
