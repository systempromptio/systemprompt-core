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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::CatalogContent;
use crate::error::MarketplaceError;
use crate::managed::{
    AssetDigest, ManagedError, ManagedRepository, ManagedResolution, ResourceKind,
    ResourceResolutionRow, RevisionBundle,
};
use systemprompt_identifiers::{ResourceRevisionId, UserId};

impl CatalogContent {
    pub async fn with_managed_skills(
        mut self,
        repository: ManagedRepository,
        owner: &UserId,
    ) -> Result<Self, MarketplaceError> {
        let rows = repository
            .list_resolutions(owner, ResourceKind::Skill)
            .await
            .map_err(MarketplaceError::Managed)?;
        self.overlay_managed(&repository, owner, rows, &BTreeSet::new())
            .await?;
        self.skills
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(self)
    }

    pub async fn with_organization_resources(
        self,
        repository: ManagedRepository,
        owner: &UserId,
        consumer: &UserId,
    ) -> Result<Self, MarketplaceError> {
        let mut catalog = if consumer == owner {
            self
        } else {
            self.with_managed_skills(repository.clone(), consumer)
                .await?
        };
        // Why: the manifest records the grant it hands out, so catalogue
        // inclusion runs without one; only an explicit revocation withholds.
        let revoked = revoked_keys(&repository, owner, consumer, ResourceKind::Skill).await?;
        let rows = repository
            .list_resolutions_for(owner, consumer, ResourceKind::Skill)
            .await
            .map_err(MarketplaceError::Managed)?;
        catalog
            .overlay_managed(&repository, owner, rows, &revoked)
            .await?;
        catalog
            .skills
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        catalog
            .overlay_harness(&repository, owner, consumer)
            .await?;
        Ok(catalog)
    }

    pub async fn without_organization_resources(
        mut self,
        repository: ManagedRepository,
        owner: &UserId,
    ) -> Result<Self, MarketplaceError> {
        for row in repository
            .list_resolutions(owner, ResourceKind::Skill)
            .await
            .map_err(MarketplaceError::Managed)?
        {
            self.remove_managed_key(&row.resource_key);
        }
        self.withhold_harness(&repository, owner).await?;
        Ok(self)
    }

    async fn overlay_managed(
        &mut self,
        repository: &ManagedRepository,
        principal: &UserId,
        rows: Vec<ResourceResolutionRow>,
        revoked: &BTreeSet<String>,
    ) -> Result<(), MarketplaceError> {
        let admitted = admit_published(rows, revoked, ResourceKind::Skill)?;
        for key in &admitted.managed {
            self.remove_managed_key(key);
        }
        let published = admitted.published;
        if published.is_empty() {
            return Ok(());
        }
        let bundles = verified_bundles(repository, principal, &published).await?;
        for (key, state) in published {
            let ManagedResolution::Published { revision_id, .. } = &state else {
                return Err(MarketplaceError::Managed(ManagedError::Integrity));
            };
            let bundle = bundles
                .get(revision_id)
                .ok_or(MarketplaceError::Managed(ManagedError::Integrity))?;
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

pub(super) struct Admitted {
    pub managed: BTreeSet<String>,
    pub published: Vec<(String, ManagedResolution)>,
}

pub(super) async fn revoked_keys(
    repository: &ManagedRepository,
    owner: &UserId,
    consumer: &UserId,
    kind: ResourceKind,
) -> Result<BTreeSet<String>, MarketplaceError> {
    if consumer == owner {
        return Ok(BTreeSet::new());
    }
    repository
        .revoked_keys(owner, consumer, kind)
        .await
        .map_err(MarketplaceError::Managed)
}

pub(super) fn admit_published(
    rows: Vec<ResourceResolutionRow>,
    revoked: &BTreeSet<String>,
    kind: ResourceKind,
) -> Result<Admitted, MarketplaceError> {
    let mut admitted = Admitted {
        managed: BTreeSet::new(),
        published: Vec::new(),
    };
    for row in rows {
        admitted.managed.insert(row.resource_key.clone());
        let reason = if revoked.contains(&row.resource_key) {
            "not_granted"
        } else {
            match row.resolution {
                ManagedResolution::Published { .. } => {
                    admitted.published.push((row.resource_key, row.resolution));
                    continue;
                },
                ManagedResolution::NeverAdopted { .. } => "never_adopted",
                ManagedResolution::Withdrawn { .. } => "withdrawn",
                ManagedResolution::IntegrityFailure { .. } => {
                    return Err(MarketplaceError::Managed(ManagedError::Integrity));
                },
                ManagedResolution::NotManaged => {
                    return Err(MarketplaceError::Catalog(format!(
                        "managed {} {} lost its resource binding",
                        kind.as_str(),
                        row.resource_key
                    )));
                },
            }
        };
        tracing::info!(
            kind = kind.as_str(),
            key = %row.resource_key,
            reason,
            "managed resource withheld from the catalogue"
        );
    }
    Ok(admitted)
}

// Why: a published revision never changes, so only revisions this repository
// has not yet verified are pulled from Postgres; the rest come from the memo.
pub(super) async fn verified_bundles(
    repository: &ManagedRepository,
    principal: &UserId,
    published: &[(String, ManagedResolution)],
) -> Result<BTreeMap<ResourceRevisionId, Arc<RevisionBundle>>, MarketplaceError> {
    let mut out = BTreeMap::new();
    let mut missing: Vec<(ResourceRevisionId, AssetDigest)> = Vec::new();
    for (_, state) in published {
        let ManagedResolution::Published {
            revision_id,
            bundle_digest,
            ..
        } = state
        else {
            continue;
        };
        match repository
            .verified_revisions()
            .get(revision_id, bundle_digest.as_str())
        {
            Some(bundle) => {
                out.insert(revision_id.clone(), bundle);
            },
            None => missing.push((revision_id.clone(), bundle_digest.clone())),
        }
    }
    if missing.is_empty() {
        return Ok(out);
    }
    let roots: Vec<(UserId, ResourceRevisionId)> = missing
        .iter()
        .map(|(revision_id, _)| (principal.clone(), revision_id.clone()))
        .collect();
    let mut fetched = repository
        .get_revision_bundles(&roots)
        .await
        .map_err(MarketplaceError::Managed)?;
    for (revision_id, bundle_digest) in missing {
        let bundle = fetched
            .remove(&revision_id)
            .ok_or(MarketplaceError::Managed(ManagedError::Integrity))?;
        if bundle.digest().map_err(ManagedError::from)? != bundle_digest {
            return Err(MarketplaceError::Managed(ManagedError::Integrity));
        }
        let bundle = Arc::new(bundle);
        repository.verified_revisions().insert(
            revision_id.clone(),
            bundle_digest.as_str().to_owned(),
            Arc::clone(&bundle),
        );
        out.insert(revision_id, bundle);
    }
    Ok(out)
}
