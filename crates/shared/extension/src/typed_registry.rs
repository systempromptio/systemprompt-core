//! ID-keyed, priority-ordered registry of built extensions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use crate::any::AnyExtension;
use crate::error::LoaderError;
pub use crate::registry::RESERVED_PATHS;
use crate::typed::{ApiExtensionTypedDyn, SchemaExtensionTyped};

pub struct TypedExtensionRegistry {
    extensions: Vec<Box<dyn AnyExtension>>,
    by_id: HashMap<String, usize>,
    api_paths: Vec<String>,
}

impl Default for TypedExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TypedExtensionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            by_id: HashMap::new(),
            api_paths: Vec::new(),
        }
    }

    pub(crate) fn add_boxed(&mut self, ext: Box<dyn AnyExtension>) {
        let idx = self.extensions.len();
        self.by_id.insert(ext.id().to_owned(), idx);

        if let Some(api) = ext.as_api() {
            self.api_paths.push(api.base_path().to_owned());
        }

        self.extensions.push(ext);
    }

    pub fn validate_api_path(&self, extension_id: &str, path: &str) -> Result<(), LoaderError> {
        if !path.starts_with("/api/") && !path.starts_with("/.") {
            return Err(LoaderError::InvalidBasePath {
                extension: extension_id.to_owned(),
                path: path.to_owned(),
            });
        }

        for reserved in RESERVED_PATHS {
            if path.starts_with(reserved) {
                return Err(LoaderError::ReservedPathCollision {
                    extension: extension_id.to_owned(),
                    path: path.to_owned(),
                });
            }
        }

        for existing in &self.api_paths {
            if path.starts_with(existing.as_str()) || existing.starts_with(path) {
                return Err(LoaderError::ReservedPathCollision {
                    extension: extension_id.to_owned(),
                    path: format!("{} (conflicts with {})", path, existing),
                });
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&dyn AnyExtension> {
        self.by_id.get(id).map(|&idx| self.extensions[idx].as_ref())
    }

    /// Schema-bearing extensions in registration order. The typed
    /// [`crate::ExtensionBuilder`] enforces dependency-before-dependent at
    /// compile time via its `Subset` typestate, and `build()` stores them in
    /// `priority()` order — so iteration order already respects dependencies.
    pub fn schema_extensions(&self) -> impl Iterator<Item = &dyn SchemaExtensionTyped> {
        self.extensions.iter().filter_map(|e| e.as_schema())
    }

    pub fn api_extensions(&self) -> impl Iterator<Item = &dyn ApiExtensionTypedDyn> {
        self.extensions.iter().filter_map(|e| e.as_api())
    }

    pub fn all_extensions(&self) -> impl Iterator<Item = &dyn AnyExtension> {
        self.extensions.iter().map(AsRef::as_ref)
    }

    #[must_use]
    pub fn api_paths(&self) -> &[String] {
        &self.api_paths
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.extensions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }
}

impl std::fmt::Debug for TypedExtensionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedExtensionRegistry")
            .field("count", &self.extensions.len())
            .field("ids", &self.by_id.keys().collect::<Vec<_>>())
            .field("api_paths", &self.api_paths)
            .finish_non_exhaustive()
    }
}
