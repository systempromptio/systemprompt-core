//! [`ManagedResourcesExtension`]: durable source, revision and publication
//! storage. It contributes the managed-resource schemas and migrations and
//! opens the [`MarketplaceSlot`], so the built marketplace seams are present
//! whenever this member is admitted.
//!
//! It contributes the `git-credentials` `cloud doctor` check
//! ([`GitCredentialCheck`]) and provides the marketplace's runtime state:
//!
//! - after governance, the `Arc<ManagedRepository>` with its publication
//!   policy, assembled from the services marketplace configuration, the
//!   marketplace seams this member's slot builds and the subject providers;
//! - the per-process `Arc<MarketplaceCache>` of the resolved catalogue and
//!   plugin bundles;
//! - the per-process `Arc<Mutex<PublishGuard>>` that serializes inventory
//!   publication passes;
//! - after governance, the inventory refresher, built from the sealed bundle
//!   sources, services root and system admin. It is provided as the
//!   crate-private `MemberRefresher` the `managed_inventory_refresh` job
//!   (contributed here) resolves, and as the open `DynInventoryPublisher` the
//!   admin services refresh runs: publish the latest configured skills with the
//!   caller as the actor, and nothing else;
//! - after governance, the organization skill resolver, bound to the system
//!   admin's managed inventory: as `Arc<OrganizationSkillResolver>` (the `core
//!   skills` commands list and resolve the owner's skills through it without
//!   learning the owner) and as `DynManagedSkillResolver`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_loader::bundle::BundleSources;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_manifest::services::{ServicesView, SystemAdmin};
use systemprompt_traits::{DynInventoryPublisher, DynManagedSkillResolver};
use tokio::sync::Mutex;

use crate::catalog::MarketplaceCache;
use crate::contract::{MarketplaceSeams, MarketplaceSlot};
use crate::inventory::{
    InventoryRefreshJob, InventoryRefresher, InventoryRefresherParts, MemberRefresher, PublishGuard,
};
use crate::managed::{
    GitCredentialCheck, ManagedRepository, OrganizationSkillResolver, PublicationPolicy,
};

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
        c.job(InventoryRefreshJob::default())
            .doctor_check(Arc::new(GitCredentialCheck));
        Ok(())
    }
}

fn provide_state(c: &mut Contributions<'_>) {
    c.provide(|_: &Resolved<'_>| Ok(Arc::new(MarketplaceCache::default())))
        .provide(|_: &Resolved<'_>| Ok(Arc::new(Mutex::new(PublishGuard::default()))))
        .provide_after_governance(|r: &Resolved<'_>| {
            let db = r.get::<DbPool>()?;
            let marketplace = r
                .get::<ServicesView>()?
                .config()
                .marketplace
                .clone()
                .unwrap_or_default();
            let policy = PublicationPolicy::from_seams(
                marketplace,
                &*r.get::<Arc<MarketplaceSeams>>()?,
                r.get::<Arc<SubjectProviderSet>>()?.as_ref().clone(),
            );
            Ok(Arc::new(
                ManagedRepository::new(&db).with_publication_policy(policy),
            ))
        })
        .provide_after_governance(|r: &Resolved<'_>| {
            Ok(MemberRefresher(Arc::new(InventoryRefresher::new(
                InventoryRefresherParts {
                    repository: r.get::<Arc<ManagedRepository>>()?,
                    guard: r.get::<Arc<Mutex<PublishGuard>>>()?,
                    bundles: r.get::<BundleSources>()?,
                    active_root: r.get::<ActiveServicesRoot>()?,
                    services: r.get::<ServicesView>()?,
                    owner: r.get::<SystemAdmin>()?.id().clone(),
                },
            ))))
        })
        .provide_after_governance(
            |r: &Resolved<'_>| -> Result<DynInventoryPublisher, ProvideError> {
                let MemberRefresher(refresher) = r.get::<MemberRefresher>()?;
                Ok(refresher)
            },
        )
        .provide_after_governance(|r: &Resolved<'_>| {
            let repository = r.get::<Arc<ManagedRepository>>()?;
            let owner = r.get::<SystemAdmin>()?;
            Ok(Arc::new(OrganizationSkillResolver::new(
                repository.as_ref().clone(),
                owner.id().clone(),
            )))
        })
        .provide_after_governance(
            |r: &Resolved<'_>| -> Result<DynManagedSkillResolver, ProvideError> {
                let resolver = r.get::<Arc<OrganizationSkillResolver>>()?;
                Ok(resolver)
            },
        );
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
