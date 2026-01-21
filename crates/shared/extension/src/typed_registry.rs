use std::any::TypeId;
use std::collections::HashMap;

use crate::any::AnyExtension;
use crate::error::LoaderError;
#[cfg(feature = "web")]
use crate::typed::ApiExtensionTypedDyn;
use crate::typed::SchemaExtensionTyped;
use crate::types::ExtensionType;

pub const RESERVED_PATHS: &[&str] = &[
    "/api/v1/oauth",
    "/api/v1/users",
    "/api/v1/agents",
    "/api/v1/mcp",
    "/api/v1/stream",
    "/api/v1/files",
    "/api/v1/analytics",
    "/api/v1/scheduler",
    "/api/v1/core",
    "/api/v1/admin",
    "/.well-known",
];

pub struct TypedExtensionRegistry {
    extensions: Vec<Box<dyn AnyExtension>>,
    by_id: HashMap<String, usize>,
    by_type: HashMap<TypeId, usize>,
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
            by_type: HashMap::new(),
            api_paths: Vec::new(),
        }
    }

    pub(crate) fn add_boxed(&mut self, ext: Box<dyn AnyExtension>) {
        let idx = self.extensions.len();
        self.by_id.insert(ext.id().to_string(), idx);

        #[cfg(feature = "web")]
        if let Some(api) = ext.as_api() {
            self.api_paths.push(api.base_path().to_string());
        }

        self.extensions.push(ext);
    }

    pub fn validate_api_path(&self, extension_id: &str, path: &str) -> Result<(), LoaderError> {
        if !path.starts_with("/api/") && !path.starts_with("/.") {
            return Err(LoaderError::InvalidBasePath {
                extension: extension_id.to_string(),
                path: path.to_string(),
            });
        }

        for reserved in RESERVED_PATHS {
            if path.starts_with(reserved) {
                return Err(LoaderError::ReservedPathCollision {
                    extension: extension_id.to_string(),
                    path: path.to_string(),
                });
            }
        }

        for existing in &self.api_paths {
            if path.starts_with(existing.as_str()) || existing.starts_with(path) {
                return Err(LoaderError::ReservedPathCollision {
                    extension: extension_id.to_string(),
                    path: format!("{} (conflicts with {})", path, existing),
                });
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn has_type<E: ExtensionType>(&self) -> bool {
        self.by_type.contains_key(&TypeId::of::<E>())
    }

    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&dyn AnyExtension> {
        self.by_id.get(id).map(|&idx| self.extensions[idx].as_ref())
    }

    #[must_use]
    pub fn get_typed<E: ExtensionType + 'static>(&self) -> Option<&E> {
        self.by_type
            .get(&TypeId::of::<E>())
            .and_then(|&idx| self.extensions[idx].as_any().downcast_ref())
    }

    pub fn schema_extensions(&self) -> impl Iterator<Item = &dyn SchemaExtensionTyped> {
        let mut schemas: Vec<_> = self
            .extensions
            .iter()
            .filter_map(|e| e.as_schema())
            .collect();
        schemas.sort_by_key(|s| s.migration_weight());
        schemas.into_iter()
    }

    #[cfg(feature = "web")]
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
