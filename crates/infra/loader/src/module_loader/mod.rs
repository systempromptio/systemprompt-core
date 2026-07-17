//! Thin wrapper around the `inventory`-driven extension registry.
//!
//! Re-exported here so that the extension framework, the schema
//! collector, and [`crate::ConfigLoader`] are all reachable from a single
//! re-export point in this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod modules;

use std::sync::Arc;
use systemprompt_extension::{Extension, LoaderError, SchemaDefinition};

#[derive(Debug, Clone, Copy)]
pub struct ModuleLoader;

impl ModuleLoader {
    pub fn discover_extensions() -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
        modules::discover_extensions()
    }

    pub fn collect_extension_schemas() -> Result<Vec<SchemaDefinition>, LoaderError> {
        modules::collect_extension_schemas()
    }
}
