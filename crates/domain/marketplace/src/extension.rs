//! Durable source, revision and publication storage.

use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct ManagedResourcesExtension;

impl Extension for ManagedResourcesExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "managed_resources",
            name: "Managed resources",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            "managed_resources",
            include_str!("../schema/managed.sql"),
        )]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["ai"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(ManagedResourcesExtension);
