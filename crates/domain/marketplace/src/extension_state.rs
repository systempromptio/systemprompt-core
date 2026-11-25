//! The runtime state `ManagedResourcesExtension` provides:
//! the managed repository with its publication policy, the catalogue cache,
//! the publish guard, the sealed inventory refreshers and the owner-bound
//! skill views. What each value is for, and why the refreshers and the
//! organization skill resolver are sealed, is the extension module's head.
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
use crate::contract::MarketplaceSeams;
use crate::inventory::{InventoryRefresher, InventoryRefresherParts, PublishGuard};
use crate::managed::{
    GitSourceDirectory, GitSourceSyncs, ManagedRepository, OrganizationSkillResolver,
    OwnerSkillView, PublicationPolicy,
};

fn refresher_parts(r: &Resolved<'_>) -> Result<InventoryRefresherParts, ProvideError> {
    Ok(InventoryRefresherParts {
        repository: r.get::<Arc<ManagedRepository>>()?,
        guard: r.get::<Arc<Mutex<PublishGuard>>>()?,
        bundles: r.get::<BundleSources>()?,
        active_root: r.get::<ActiveServicesRoot>()?,
        services: r.get::<ServicesView>()?,
        owner: r.get::<SystemAdmin>()?.id().clone(),
    })
}

pub(crate) fn provide_state(c: &mut Contributions<'_>) {
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
        .provide_sealed_after_governance(
            |r: &Resolved<'_>| -> Result<Arc<InventoryRefresher>, ProvideError> {
                Ok(Arc::new(InventoryRefresher::new(refresher_parts(r)?)))
            },
        )
        .provide_sealed_after_governance(
            |r: &Resolved<'_>| -> Result<DynInventoryPublisher, ProvideError> {
                Ok(Arc::new(InventoryRefresher::new(refresher_parts(r)?)))
            },
        )
        .provide_after_governance(GitSourceDirectory::provide)
        .provide_sealed_after_governance(GitSourceSyncs::provide)
        .provide_after_governance(|r: &Resolved<'_>| {
            let repository = r.get::<Arc<ManagedRepository>>()?;
            let owner = r.get::<SystemAdmin>()?;
            Ok(Arc::new(OwnerSkillView::new(
                repository.as_ref().clone(),
                owner.id().clone(),
            )))
        })
        .provide_sealed_after_governance(
            |r: &Resolved<'_>| -> Result<DynManagedSkillResolver, ProvideError> {
                let repository = r.get::<Arc<ManagedRepository>>()?;
                let owner = r.get::<SystemAdmin>()?;
                Ok(Arc::new(OrganizationSkillResolver::new(
                    repository.as_ref().clone(),
                    owner.id().clone(),
                )))
            },
        );
}
