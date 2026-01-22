use std::sync::Arc;
use systemprompt_extension::{Extension, ExtensionRegistry, SchemaDefinition};

pub fn discover_extensions() -> Vec<Arc<dyn Extension>> {
    ExtensionRegistry::discover().extensions().to_vec()
}

pub fn collect_extension_schemas() -> Vec<SchemaDefinition> {
    let registry = ExtensionRegistry::discover();
    registry
        .schema_extensions()
        .into_iter()
        .flat_map(|ext| ext.schemas())
        .collect()
}
