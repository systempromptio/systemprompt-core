//! Registry validation for extension dependencies, and the base-path
//! constants the server's mount judges extension routers by.
//!
//! An extension router is either nested under `/api/` or merged at the web
//! root ([`WEB_ROOT_BASE_PATH`], the site extension that serves pages); any
//! other base would shadow core routes. [`RESERVED_PATHS`] names the prefixes
//! core route groups own. The server's mount refuses an extension declaration
//! with any other base path, or under one of these prefixes, unless its
//! closed table of core declarations grants that exact declaration the path,
//! so each refusal and its exceptions live in one place.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ExtensionRegistry, dependency_order};
use crate::error::LoaderError;

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
}
