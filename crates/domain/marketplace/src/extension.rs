//! Durable source, revision and publication storage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
        vec![
            SchemaDefinition::new("managed_resources", include_str!("../schema/managed.sql")),
            SchemaDefinition::new(
                "managed_consumer_evidence",
                include_str!("../schema/consumer_evidence.sql"),
            ),
            SchemaDefinition::new(
                "managed_evaluation_attestations",
                include_str!("../schema/evaluation_attestations.sql"),
            ),
            SchemaDefinition::new("managed_inventory", include_str!("../schema/inventory.sql")),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["ai", "users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(ManagedResourcesExtension);
