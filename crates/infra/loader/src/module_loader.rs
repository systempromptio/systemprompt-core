//! Thin wrapper around the `inventory`-driven extension registry.
//!
//! Re-exported here so that the extension framework, the schema
//! collector, and [`crate::ConfigLoader`] are all reachable from a single
//! re-export point in this crate.

use std::sync::Arc;
use systemprompt_extension::{Extension, SchemaDefinition};

use crate::modules;

#[derive(Debug, Clone, Copy)]
pub struct ModuleLoader;

impl ModuleLoader {
    #[must_use]
    pub fn discover_extensions() -> Vec<Arc<dyn Extension>> {
        modules::discover_extensions()
    }

    #[must_use]
    pub fn collect_extension_schemas() -> Vec<SchemaDefinition> {
        modules::collect_extension_schemas()
    }
}
