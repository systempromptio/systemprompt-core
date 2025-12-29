use crate::error::LoaderError;
use crate::Extension;
use std::collections::HashMap;
use std::sync::Arc;

pub const RESERVED_PATHS: &[&str] = &[
    "/api/v1/oauth",
    "/api/v1/users",
    "/api/v1/agents",
    "/api/v1/mcp",
    "/api/v1/stream",
    "/api/v1/content",
    "/api/v1/files",
    "/api/v1/analytics",
    "/api/v1/scheduler",
    "/api/v1/core",
    "/api/v1/admin",
    "/.well-known",
];

#[derive(Default)]
pub struct ExtensionRegistry {
    extensions: HashMap<String, Arc<dyn Extension>>,
    sorted_extensions: Vec<Arc<dyn Extension>>,
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

    #[must_use]
    pub fn discover() -> Self {
        let mut registry = Self::new();

        for ext in inventory::iter::<ExtensionRegistration> {
            let ext_arc = (ext.factory)();
            registry
                .extensions
                .insert(ext_arc.id().to_string(), Arc::clone(&ext_arc));
            registry.sorted_extensions.push(ext_arc);
        }

        registry.sort_by_priority();
        registry
    }

    fn sort_by_priority(&mut self) {
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

    pub fn validate_dependencies(&self) -> Result<(), LoaderError> {
        for ext in self.extensions.values() {
            for dep_id in ext.dependencies() {
                if !self.extensions.contains_key(dep_id) {
                    return Err(LoaderError::MissingDependency {
                        extension: ext.id().to_string(),
                        dependency: dep_id.to_string(),
                    });
                }
            }
        }

        self.detect_cycles()
    }

    fn detect_cycles(&self) -> Result<(), LoaderError> {
        const WHITE: u8 = 0;
        const GRAY: u8 = 1;
        const BLACK: u8 = 2;

        fn dfs<'a>(
            node: &'a str,
            extensions: &'a HashMap<String, Arc<dyn Extension>>,
            color: &mut HashMap<&'a str, u8>,
            path: &mut Vec<&'a str>,
        ) -> Result<(), Vec<&'a str>> {
            color.insert(node, GRAY);
            path.push(node);

            if let Some(ext) = extensions.get(node) {
                for dep_id in ext.dependencies() {
                    match color.get(dep_id) {
                        Some(&GRAY) => {
                            path.push(dep_id);
                            return Err(path.clone());
                        },
                        Some(&WHITE) | None => {
                            dfs(dep_id, extensions, color, path)?;
                        },
                        _ => {},
                    }
                }
            }

            path.pop();
            color.insert(node, BLACK);
            Ok(())
        }

        let mut color: HashMap<&str, u8> = self
            .extensions
            .keys()
            .map(|id| (id.as_str(), WHITE))
            .collect();

        let mut path = Vec::new();
        for id in self.extensions.keys() {
            if color.get(id.as_str()) == Some(&WHITE) {
                if let Err(cycle_path) = dfs(id.as_str(), &self.extensions, &mut color, &mut path) {
                    let cycle_start = cycle_path.last().copied().unwrap_or("");
                    let cycle_start_idx = cycle_path
                        .iter()
                        .position(|&x| x == cycle_start)
                        .unwrap_or(0);
                    let cycle: Vec<_> = cycle_path[cycle_start_idx..].to_vec();

                    return Err(LoaderError::CircularDependency {
                        chain: cycle.join(" -> "),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn validate_api_paths(&self, ctx: &dyn crate::ExtensionContext) -> Result<(), LoaderError> {
        for ext in self.extensions.values() {
            if let Some(router_config) = ext.router(ctx) {
                let base_path = router_config.base_path;

                if !base_path.starts_with("/api/") {
                    return Err(LoaderError::InvalidBasePath {
                        extension: ext.id().to_string(),
                        path: base_path.to_string(),
                    });
                }

                for reserved in RESERVED_PATHS {
                    if base_path.starts_with(reserved) {
                        return Err(LoaderError::ReservedPathCollision {
                            extension: ext.id().to_string(),
                            path: base_path.to_string(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), LoaderError> {
        self.validate_dependencies()?;
        Ok(())
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Arc<dyn Extension>> {
        self.extensions.get(id)
    }

    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.extensions.contains_key(id)
    }

    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.extensions.keys().map(String::as_str).collect()
    }

    #[must_use]
    pub fn extensions(&self) -> &[Arc<dyn Extension>] {
        &self.sorted_extensions
    }

    #[must_use]
    pub fn schema_extensions(&self) -> Vec<Arc<dyn Extension>> {
        let mut exts: Vec<_> = self
            .sorted_extensions
            .iter()
            .filter(|e| e.has_schemas())
            .cloned()
            .collect();
        exts.sort_by_key(|e| e.migration_weight());
        exts
    }

    #[must_use]
    pub fn api_extensions(&self, ctx: &dyn crate::ExtensionContext) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_router(ctx))
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn job_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_jobs())
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn config_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_config())
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn llm_provider_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_llm_providers())
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn tool_provider_extensions(&self) -> Vec<Arc<dyn Extension>> {
        self.sorted_extensions
            .iter()
            .filter(|e| e.has_tool_providers())
            .cloned()
            .collect()
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
