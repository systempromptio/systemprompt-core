//! Registry validation for extension dependencies and reserved API path
//! collisions.
//!
//! An extension router is either nested under `/api/` or merged at the web
//! root `/` (the site extension that serves pages); any other base would
//! shadow core routes without the reserved-path table seeing it. Paths are
//! validated over the routers the enabled extensions actually declare, the
//! same set the server mounts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ApiRouter, ExtensionRegistry, dependency_order};
use crate::error::LoaderError;
use systemprompt_identifiers::ExtensionId;

pub const WEB_ROOT_BASE_PATH: &str = "/";

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
        dependency_order(&self.extensions).map(|_| ())
    }

    pub fn validate_api_paths(routers: &[ApiRouter]) -> Result<(), LoaderError> {
        for (ext, declared) in routers {
            let base_path = declared.base_path;

            if base_path != WEB_ROOT_BASE_PATH && !base_path.starts_with("/api/") {
                return Err(LoaderError::InvalidBasePath {
                    extension: ExtensionId::new(ext.id()),
                    path: base_path.to_owned(),
                });
            }

            if RESERVED_PATHS.iter().any(|reserved| base_path.starts_with(reserved)) {
                return Err(LoaderError::ReservedPathCollision {
                    extension: ExtensionId::new(ext.id()),
                    path: base_path.to_owned(),
                });
            }
        }
        Ok(())
    }
}
