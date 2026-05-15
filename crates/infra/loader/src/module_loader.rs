//! Thin wrapper around the `inventory`-driven extension registry.
//!
//! Re-exported here so that the extension framework, the schema
//! collector, and [`crate::ConfigLoader`] are all reachable from a single
//! re-export point in this crate.

use std::sync::Arc;
use systemprompt_extension::{Extension, LoaderError, SchemaDefinition};

use crate::modules;

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
