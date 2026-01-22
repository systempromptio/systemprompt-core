use std::sync::Arc;
use systemprompt_extension::{Extension, SchemaDefinition};

use crate::modules;

#[derive(Debug, Clone, Copy)]
pub struct ModuleLoader;

impl ModuleLoader {
    pub fn discover_extensions() -> Vec<Arc<dyn Extension>> {
        modules::discover_extensions()
    }

    pub fn collect_extension_schemas() -> Vec<SchemaDefinition> {
        modules::collect_extension_schemas()
    }
}
