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
        let mut schemas = source_schemas();
        schemas.extend(publication_schemas());
        schemas.extend(attestation_schemas());
        schemas
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["ai"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(ManagedResourcesExtension);

fn source_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "managed_sources",
            include_str!("../schema/managed_sources.sql"),
        ),
        SchemaDefinition::new(
            "managed_source_snapshots",
            include_str!("../schema/managed_source_snapshots.sql"),
        ),
        SchemaDefinition::new(
            "managed_assets",
            include_str!("../schema/managed_assets.sql"),
        ),
        SchemaDefinition::new(
            "managed_resources",
            include_str!("../schema/managed_resources.sql"),
        ),
        SchemaDefinition::new(
            "managed_revisions",
            include_str!("../schema/managed_revisions.sql"),
        ),
        SchemaDefinition::new(
            "managed_revision_assets",
            include_str!("../schema/managed_revision_assets.sql"),
        ),
        SchemaDefinition::new(
            "managed_revision_dependencies",
            include_str!("../schema/managed_revision_dependencies.sql"),
        ),
    ]
}

fn publication_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "managed_publication_reviews",
            include_str!("../schema/managed_publication_reviews.sql"),
        ),
        SchemaDefinition::new(
            "managed_publications",
            include_str!("../schema/managed_publications.sql"),
        ),
        SchemaDefinition::new(
            "managed_publication_selections",
            include_str!("../schema/managed_publication_selections.sql"),
        ),
        SchemaDefinition::new(
            "managed_distribution_outbox",
            include_str!("../schema/managed_distribution_outbox.sql"),
        ),
        SchemaDefinition::new(
            "managed_reconciliations",
            include_str!("../schema/managed_reconciliations.sql"),
        ),
        SchemaDefinition::new(
            "managed_reconciliation_conflicts",
            include_str!("../schema/managed_reconciliation_conflicts.sql"),
        ),
        SchemaDefinition::new(
            "managed_withdrawal_proposals",
            include_str!("../schema/managed_withdrawal_proposals.sql"),
        ),
        SchemaDefinition::new(
            "managed_distribution_deliveries",
            include_str!("../schema/managed_distribution_deliveries.sql"),
        ),
    ]
}

fn attestation_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "managed_installation_receipts",
            include_str!("../schema/managed_installation_receipts.sql"),
        ),
        SchemaDefinition::new(
            "managed_invocation_attributions",
            include_str!("../schema/managed_invocation_attributions.sql"),
        ),
        SchemaDefinition::new(
            "managed_evaluation_attestations",
            include_str!("../schema/managed_evaluation_attestations.sql"),
        ),
        SchemaDefinition::new(
            "managed_git_verifications",
            include_str!("../schema/managed_git_verifications.sql"),
        ),
    ]
}
