//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ExtensionRegistry, topo_sort};
use crate::error::LoaderError;

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

impl ExtensionRegistry {
    pub fn validate_dependencies(&self) -> Result<(), LoaderError> {
        for ext in self.extensions.values() {
            for dep_id in ext.dependencies() {
                if !self.extensions.contains_key(dep_id) {
                    return Err(LoaderError::MissingDependency {
                        extension: ext.id().to_owned(),
                        dependency: dep_id.to_owned(),
                    });
                }
            }
        }

        let ids: Vec<String> = self.extensions.keys().cloned().collect();
        topo_sort(&ids, &self.extensions).map(|_| ())
    }

    pub fn validate_api_paths(&self, ctx: &dyn crate::ExtensionContext) -> Result<(), LoaderError> {
        for ext in self.extensions.values() {
            if let Some(router_config) = ext.router(ctx) {
                let base_path = router_config.base_path;

                if !base_path.starts_with("/api/") {
                    return Err(LoaderError::InvalidBasePath {
                        extension: ext.id().to_owned(),
                        path: base_path.to_owned(),
                    });
                }

                for reserved in RESERVED_PATHS {
                    if base_path.starts_with(reserved) {
                        return Err(LoaderError::ReservedPathCollision {
                            extension: ext.id().to_owned(),
                            path: base_path.to_owned(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
