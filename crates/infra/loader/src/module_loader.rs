//! Thin wrapper around the `inventory`-driven extension registry.
//!
//! Re-exported here so that the extension framework, the schema
//! collector, and [`crate::ConfigLoader`] are all reachable from a single
//! re-export point in this crate.

use std::sync::Arc;
use systemprompt_extension::{Extension, SchemaDefinition};

use crate::modules;

/// Stateless module-level extension discovery.
#[derive(Debug, Clone, Copy)]
pub struct ModuleLoader;

impl ModuleLoader {
    /// Returns every compiled-in [`Extension`] registered via the
    /// `inventory` macro.
    #[must_use]
    pub fn discover_extensions() -> Vec<Arc<dyn Extension>> {
        modules::discover_extensions()
    }

    /// Collects the union of every schema declared by every compiled-in
    /// schema-extension.
    #[must_use]
    pub fn collect_extension_schemas() -> Vec<SchemaDefinition> {
        modules::collect_extension_schemas()
    }
}
