//! Filesystem module discovery for services and extensions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_extension::{Extension, ExtensionRegistry, LoaderError, SchemaDefinition};

pub(super) fn discover_extensions() -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
    Ok(ExtensionRegistry::discover()?.extensions().to_vec())
}

pub(super) fn collect_extension_schemas() -> Result<Vec<SchemaDefinition>, LoaderError> {
    let registry = ExtensionRegistry::discover()?;
    Ok(registry
        .schema_extensions()
        .into_iter()
        .flat_map(|ext| ext.schemas())
        .collect())
}
