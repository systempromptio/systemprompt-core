mod discovery;
mod queries;
mod validation;

use crate::Extension;
use crate::error::LoaderError;
use std::collections::HashMap;
use std::sync::Arc;

pub use validation::RESERVED_PATHS;

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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn sort_by_priority(&mut self) {
        self.sorted_extensions.sort_by_key(|e| e.priority());
    }

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

    pub fn merge(&mut self, extensions: Vec<Arc<dyn Extension>>) -> Result<(), LoaderError> {
        for ext in extensions {
            self.register(ext)?;
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), LoaderError> {
        self.validate_dependencies()?;
        Ok(())
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

#[derive(Debug, Clone, Copy)]
pub struct ExtensionRegistration {
    pub factory: fn() -> Arc<dyn Extension>,
}

inventory::collect!(ExtensionRegistration);
