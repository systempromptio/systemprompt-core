//! Managed catalog projections preserve organization ownership and grants.
//!
//! The overlay is set-based: one query lists every managed skill of a
//! principal with its pinned publication, one lists the keys a consumer has
//! been revoked, and the published revision closures are fetched together.
//! The per-key semantics are unchanged — a managed key is removed from the
//! disk catalogue whatever its state, a withheld or revoked one stays
//! removed, a published one is overlaid from its retained revision, and a
//! selection whose retained content fails verification is an error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use super::CatalogContent;
use crate::error::MarketplaceError;
use crate::managed::{ManagedError, ManagedRepository, ManagedResolution, SkillResolutionRow};
use systemprompt_identifiers::{ResourceRevisionId, UserId};

impl CatalogContent {
    pub async fn with_managed_skills(
        mut self,
        repository: ManagedRepository,
        owner: &UserId,
    ) -> Result<Self, MarketplaceError> {
        let rows = repository
            .list_skill_resolutions(owner)
            .await
            .map_err(MarketplaceError::Managed)?;
        self.overlay_managed(&repository, owner, rows, &BTreeSet::new())
            .await?;
        self.skills
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(self)
    }

    pub async fn with_organization_skills(
        self,
        repository: ManagedRepository,
        owner: &UserId,
        consumer: &UserId,
    ) -> Result<Self, MarketplaceError> {
        if consumer == owner {
            return self.with_managed_skills(repository, owner).await;
        }
        let mut catalog = self
            .with_managed_skills(repository.clone(), consumer)
            .await?;
        // Why: the manifest records the grant it hands out, so catalogue
        // inclusion runs without one; only an explicit revocation withholds.
        let revoked = repository
            .revoked_skill_keys(owner, consumer)
            .await
            .map_err(MarketplaceError::Managed)?;
        let rows = repository
            .list_skill_resolutions(owner)
            .await
            .map_err(MarketplaceError::Managed)?;
        catalog
            .overlay_managed(&repository, owner, rows, &revoked)
            .await?;
        catalog
            .skills
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(catalog)
    }

    pub async fn without_organization_skills(
        mut self,
        repository: ManagedRepository,
        owner: &UserId,
    ) -> Result<Self, MarketplaceError> {
        for row in repository
            .list_skill_resolutions(owner)
            .await
            .map_err(MarketplaceError::Managed)?
        {
            self.remove_managed_key(&row.resource_key);
        }
        Ok(self)
    }

    async fn overlay_managed(
        &mut self,
        repository: &ManagedRepository,
        principal: &UserId,
        rows: Vec<SkillResolutionRow>,
        revoked: &BTreeSet<String>,
    ) -> Result<(), MarketplaceError> {
        let mut published: Vec<(String, ManagedResolution)> = Vec::new();
        for row in rows {
            self.remove_managed_key(&row.resource_key);
            if revoked.contains(&row.resource_key) {
                tracing::info!(
                    skill = %row.resource_key,
                    reason = "not_granted",
                    "managed skill withheld from the catalogue"
                );
                continue;
            }
            match row.resolution {
                ManagedResolution::Published { .. } => {
                    published.push((row.resource_key, row.resolution));
                },
                ManagedResolution::NeverAdopted { .. } => {
                    tracing::info!(
                        skill = %row.resource_key,
                        reason = "never_adopted",
                        "managed skill withheld from the catalogue"
                    );
                },
                ManagedResolution::Withdrawn { .. } => {
                    tracing::info!(
                        skill = %row.resource_key,
                        reason = "withdrawn",
                        "managed skill withheld from the catalogue"
                    );
                },
                ManagedResolution::IntegrityFailure { .. } => {
                    return Err(MarketplaceError::Managed(ManagedError::Integrity));
                },
                ManagedResolution::NotManaged => {
                    return Err(MarketplaceError::Catalog(format!(
                        "managed skill {} lost its resource binding",
                        row.resource_key
                    )));
                },
            }
        }
        if published.is_empty() {
            return Ok(());
        }
        let roots: Vec<(UserId, ResourceRevisionId)> = published
            .iter()
            .filter_map(|(_, state)| match state {
                ManagedResolution::Published { revision_id, .. } => {
                    Some((principal.clone(), revision_id.clone()))
                },
                _ => None,
            })
            .collect();
        let bundles = repository
            .get_revision_bundles(&roots)
            .await
            .map_err(MarketplaceError::Managed)?;
        for (key, state) in published {
            let ManagedResolution::Published {
                revision_id,
                bundle_digest,
                ..
            } = &state
            else {
                return Err(MarketplaceError::Managed(ManagedError::Integrity));
            };
            let bundle = bundles
                .get(revision_id)
                .ok_or(MarketplaceError::Managed(ManagedError::Integrity))?;
            if &bundle.digest().map_err(ManagedError::from)? != bundle_digest {
                return Err(MarketplaceError::Managed(ManagedError::Integrity));
            }
            let skill = crate::managed::managed_skill_from_bundle(&key, state, bundle)
                .map_err(MarketplaceError::Managed)?;
            let (entry, files) = crate::catalog::skills::build_managed_skill_entry(skill)?;
            self.managed_files.insert(entry.id.clone(), files);
            self.skills.push(entry);
        }
        Ok(())
    }

    fn remove_managed_key(&mut self, key: &str) {
        self.skills.retain(|entry| entry.id.as_str() != key);
        self.managed_files.retain(|id, _| id.as_str() != key);
    }
}
