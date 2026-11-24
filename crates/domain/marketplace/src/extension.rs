//! [`ManagedResourcesExtension`]: durable source, revision and publication
//! storage. It contributes the managed-resource schemas and migrations and
//! opens the [`MarketplaceSlot`], so the built marketplace seams are present
//! whenever this member is admitted.
//!
//! It contributes the `git-credentials` `cloud doctor` check
//! ([`GitCredentialCheck`]) with its probe source, the open
//! `DynGitCredentialDirectory`, and the marketplace's runtime state:
//!
//! - after governance, the `Arc<ManagedRepository>` with its publication
//!   policy, assembled from the services marketplace configuration, the
//!   marketplace seams this member's slot builds and the subject providers;
//! - the per-process `Arc<MarketplaceCache>` of the resolved catalogue and
//!   plugin bundles;
//! - the per-process `Arc<Mutex<PublishGuard>>` that serializes inventory
//!   publication passes;
//! - after governance, two inventory refreshers, each built from the sealed
//!   bundle sources, services root and system admin, both provided sealed. The
//!   `DynInventoryPublisher` is the one the kernel's admin services refresh
//!   route runs with the authenticated request's user as the actor. The
//!   `Arc<InventoryRefresher>` is read only by this member's own
//!   `managed_inventory_refresh` job, whose context keeps its member's sealed
//!   values, and publishes only as the configured owner; no other member's job,
//!   command or route state resolves either;
//! - after governance, two separate values bound to the system admin's managed
//!   inventory. `Arc<OwnerSkillView>` is open: the `core skills` commands list
//!   and resolve the owner's own skills through it, with no consumer parameter
//!   and without learning the owner. The `DynManagedSkillResolver` the kernel
//!   hands the agent runtime is a separately built `OrganizationSkillResolver`
//!   provided sealed: it resolves for a caller-named consumer, so no member
//!   runtime context resolves it, and no open value wraps, aliases or coerces
//!   to it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

use crate::contract::MarketplaceSlot;
use crate::extension_state::provide_state;
use crate::inventory::InventoryRefreshJob;
use crate::managed::GitCredentialCheck;
use crate::managed::git_sources::GitCredentialReferences;

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

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in source_schemas()
            .into_iter()
            .chain(publication_schemas())
            .chain(attestation_schemas())
            .chain(consumer_schemas())
            .chain(inventory_schemas())
            .chain(operation_schemas())
        {
            c.schema(schema);
        }
        c.migrations(extension_migrations!());
        c.retire(SchemaDefinition {
            table: None,
            sql: include_str!("../schema/retirements/retired_tables.sql").to_owned(),
            required_columns: Vec::new(),
            schema: None,
        });
        provide_state(c);
        c.slot::<MarketplaceSlot>();
        GitCredentialCheck::contribute(c);
        c.job(InventoryRefreshJob::default())
            .secret_references(|_: &Resolved<'_>| {
                Ok::<_, FactoryError>(GitCredentialReferences::shared())
            });
        Ok(())
    }
}

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
            "managed_publication_cohort_selections",
            include_str!("../schema/managed_publication_cohort_selections.sql"),
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
    vec![SchemaDefinition::new(
        "managed_installation_receipts",
        include_str!("../schema/managed_installation_receipts.sql"),
    )]
}

fn consumer_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "managed_consumer_credentials",
            include_str!("../schema/managed_consumer_credentials.sql"),
        ),
        SchemaDefinition::new(
            "managed_consumer_grants",
            include_str!("../schema/managed_consumer_grants.sql"),
        ),
        SchemaDefinition::new(
            "managed_consumer_session_bindings",
            include_str!("../schema/managed_consumer_session_bindings.sql"),
        ),
    ]
}

fn inventory_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "managed_inventory_state",
            include_str!("../schema/managed_inventory_state.sql"),
        ),
        SchemaDefinition::new(
            "managed_inventory_entries",
            include_str!("../schema/managed_inventory_entries.sql"),
        ),
        SchemaDefinition::new(
            "managed_inventory_membership",
            include_str!("../schema/managed_inventory_membership.sql"),
        ),
        SchemaDefinition::new(
            "managed_inventory_bindings",
            include_str!("../schema/managed_inventory_bindings.sql"),
        ),
        SchemaDefinition::new(
            "managed_inventory_authoring_heads",
            include_str!("../schema/managed_inventory_authoring_heads.sql"),
        ),
        SchemaDefinition::new(
            "managed_inventory_observations",
            include_str!("../schema/managed_inventory_observations.sql"),
        ),
    ]
}

fn operation_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "managed_installation_coverage_state",
            include_str!("../schema/managed_installation_coverage_state.sql"),
        ),
        SchemaDefinition::new(
            "managed_installation_coverage",
            include_str!("../schema/managed_installation_coverage.sql"),
        ),
        SchemaDefinition::sql_only(include_str!(
            "../schema/managed_installation_coverage_queue.sql"
        )),
    ]
}
