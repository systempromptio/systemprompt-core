use super::ExtensionRegistry;
use crate::Extension;
use crate::error::LoaderError;
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

impl ExtensionRegistry {
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

        detect_cycles(&self.extensions)
    }

    #[cfg(feature = "web")]
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

    #[cfg(not(feature = "web"))]
    pub fn validate_api_paths(
        &self,
        _ctx: &dyn crate::ExtensionContext,
    ) -> Result<(), LoaderError> {
        Ok(())
    }
}

fn detect_cycles(extensions: &HashMap<String, Arc<dyn Extension>>) -> Result<(), LoaderError> {
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

    let mut color: HashMap<&str, u8> = extensions.keys().map(|id| (id.as_str(), WHITE)).collect();

    let mut path = Vec::new();
    for id in extensions.keys() {
        if color.get(id.as_str()) == Some(&WHITE) {
            if let Err(cycle_path) = dfs(id.as_str(), extensions, &mut color, &mut path) {
                let Some(&cycle_start) = cycle_path.last() else {
                    return Err(LoaderError::CircularDependency {
                        chain: "unknown cycle".to_string(),
                    });
                };
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
