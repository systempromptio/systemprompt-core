use std::marker::PhantomData;

use crate::any::{AnyExtension, ApiExtensionWrapper, ExtensionWrapper, SchemaExtensionWrapper};
use crate::error::LoaderError;
use crate::hlist::{Subset, TypeList};
use crate::typed::{ApiExtensionTypedDyn, SchemaExtensionTyped};
use crate::typed_registry::TypedExtensionRegistry;
use crate::types::{Dependencies, ExtensionType};

pub struct ExtensionBuilder<Registered: TypeList = ()> {
    extensions: Vec<Box<dyn AnyExtension>>,
    _marker: PhantomData<Registered>,
}

impl<R: TypeList> std::fmt::Debug for ExtensionBuilder<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionBuilder")
            .field("extension_count", &self.extensions.len())
            .finish_non_exhaustive()
    }
}

impl ExtensionBuilder<()> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            _marker: PhantomData,
        }
    }
}

impl Default for ExtensionBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: TypeList> ExtensionBuilder<R> {
    pub fn extension<E>(mut self, ext: E) -> ExtensionBuilder<(E, R)>
    where
        E: ExtensionType + Dependencies + std::fmt::Debug + 'static,
        E::Deps: Subset<R>,
    {
        self.extensions.push(Box::new(ExtensionWrapper::new(ext)));
        ExtensionBuilder {
            extensions: self.extensions,
            _marker: PhantomData,
        }
    }

    pub fn schema_extension<E>(mut self, ext: E) -> ExtensionBuilder<(E, R)>
    where
        E: ExtensionType + Dependencies + SchemaExtensionTyped + std::fmt::Debug + 'static,
        E::Deps: Subset<R>,
    {
        self.extensions
            .push(Box::new(SchemaExtensionWrapper::new(ext)));
        ExtensionBuilder {
            extensions: self.extensions,
            _marker: PhantomData,
        }
    }

    pub fn api_extension<E>(mut self, ext: E) -> ExtensionBuilder<(E, R)>
    where
        E: ExtensionType + Dependencies + ApiExtensionTypedDyn + std::fmt::Debug + 'static,
        E::Deps: Subset<R>,
    {
        self.extensions
            .push(Box::new(ApiExtensionWrapper::new(ext)));
        ExtensionBuilder {
            extensions: self.extensions,
            _marker: PhantomData,
        }
    }

    pub fn build(self) -> Result<TypedExtensionRegistry, LoaderError> {
        let mut registry = TypedExtensionRegistry::new();
        let mut sorted = self.extensions;
        sorted.sort_by_key(|e| e.priority());

        for ext in sorted {
            if registry.has(ext.id()) {
                return Err(LoaderError::DuplicateExtension(ext.id().to_string()));
            }

            if let Some(api) = ext.as_api() {
                registry.validate_api_path(ext.id(), api.base_path())?;
            }

            registry.add_boxed(ext);
        }

        Ok(registry)
    }
}
