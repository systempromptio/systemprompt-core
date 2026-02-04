use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct DatabaseExtension;

impl Extension for DatabaseExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "database",
            name: "Database",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn migration_weight(&self) -> u32 {
        1
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline(
                "extension_migrations",
                include_str!("../schema/extension_migrations.sql"),
            ),
            SchemaDefinition::inline(
                "functions",
                include_str!("../schema/functions.sql"),
            ),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }
}

register_extension!(DatabaseExtension);
