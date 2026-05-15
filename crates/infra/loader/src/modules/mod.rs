use std::sync::Arc;
use systemprompt_extension::{Extension, ExtensionRegistry, LoaderError, SchemaDefinition};

pub fn discover_extensions() -> Result<Vec<Arc<dyn Extension>>, LoaderError> {
    Ok(ExtensionRegistry::discover()?.extensions().to_vec())
}

pub fn collect_extension_schemas() -> Result<Vec<SchemaDefinition>, LoaderError> {
    let registry = ExtensionRegistry::discover()?;
    Ok(registry
        .schema_extensions()
        .into_iter()
        .flat_map(|ext| ext.schemas())
        .collect())
}
