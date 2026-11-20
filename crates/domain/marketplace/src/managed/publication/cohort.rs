//! Cohort resolution: which configured ring a consumer resolves a managed
//! resource through.
//!
//! A consumer's membership is read per ring dimension — `role` from the
//! consumer's roles, any other dimension from the registered subject-attribute
//! provider of that slug. A ring naming a dimension no provider serves is a
//! resolution failure, never a silent fall-through to the base selection.
//! Rings are tried in configured order; the first matched ring with a cohort
//! selection for a resource decides that resource, otherwise the base
//! selection does.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ManagedResourceId, PublicationId, ResourceRevisionId, UserId};
use systemprompt_manifest::services::CohortRing;

use super::{
    AssetDigest, ManagedError, ManagedRepository, ManagedResolution, ResourceKind,
    ResourceResolutionRow, Result, invalid,
};

const ROLE_DIMENSION: &str = "role";

/// The cohort a publication targets instead of the resource's base selection:
/// `dimension` and `value` name one ring of the services `marketplace.rings`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct CohortSelector {
    pub dimension: String,
    pub value: String,
}

impl ManagedRepository {
    pub async fn consumer_rings(&self, consumer: &UserId) -> Result<Vec<CohortRing>> {
        let policy = self.publication_policy();
        let mut held: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut matched = Vec::new();
        for ring in &policy.config.rings {
            let dimension = ring.dimension.as_str();
            if !held.contains_key(dimension) {
                let values = self.dimension_values(dimension, consumer).await?;
                held.insert(dimension, values);
            }
            if held
                .get(dimension)
                .is_some_and(|values| values.contains(&ring.value))
            {
                matched.push(ring.clone());
            }
        }
        Ok(matched)
    }

    async fn dimension_values(&self, dimension: &str, consumer: &UserId) -> Result<Vec<String>> {
        if dimension == ROLE_DIMENSION {
            let directory = self.publication_policy().roles.as_ref().ok_or_else(|| {
                invalid("A cohort ring names roles but no role directory is registered")
            })?;
            return directory
                .roles_of(consumer)
                .await
                .map_err(|source| ManagedError::Internal {
                    context: "cohort role membership",
                    source: source.into(),
                });
        }
        let provider = self
            .publication_policy()
            .subjects
            .find(dimension)
            .ok_or_else(|| invalid("A cohort ring names an unregistered subject dimension"))?;
        provider
            .values_for(consumer)
            .await
            .map_err(|source| ManagedError::Internal {
                context: "cohort membership",
                source: source.into(),
            })
    }

    pub async fn list_resolutions_for(
        &self,
        owner: &UserId,
        consumer: &UserId,
        kind: ResourceKind,
    ) -> Result<Vec<ResourceResolutionRow>> {
        let mut rows = self.list_resolutions(owner, kind).await?;
        let rings = self.consumer_rings(consumer).await?;
        if rings.is_empty() {
            return Ok(rows);
        }
        let mut overrides = self.cohort_overrides(owner, &rings, kind).await?;
        for row in &mut rows {
            if let Some(resolution) = overrides.remove(&row.resource_key) {
                row.resolution = resolution;
            }
        }
        Ok(rows)
    }

    async fn cohort_overrides(
        &self,
        owner: &UserId,
        rings: &[CohortRing],
        kind: ResourceKind,
    ) -> Result<BTreeMap<String, ManagedResolution>> {
        let dimensions: Vec<String> = rings.iter().map(|ring| ring.dimension.clone()).collect();
        let selections = sqlx::query!(
            r#"SELECT r.resource_key, c.resource_id, c.cohort_dimension, c.cohort_value, c.generation,
                c.publication_id, c.revision_id, c.bundle_digest,
                p.revision_id AS "published_revision_id?", p.bundle_digest AS "published_bundle_digest?"
            FROM managed_publication_cohort_selections c
            JOIN managed_resources r ON r.owner_id=c.owner_id AND r.id=c.resource_id
            LEFT JOIN managed_publications p ON p.owner_id=c.owner_id AND p.resource_id=c.resource_id
                AND p.id=c.publication_id AND p.generation=c.generation
            WHERE c.owner_id=$1 AND r.kind=$3 AND c.cohort_dimension = ANY($2)
            ORDER BY r.resource_key, c.cohort_dimension, c.cohort_value"#,
            owner.as_str(),
            &dimensions,
            kind.as_str()
        )
        .fetch_all(&self.pool)
        .await?;
        let mut chosen: BTreeMap<String, (usize, ManagedResolution)> = BTreeMap::new();
        for row in selections {
            let Some(rank) = rings.iter().position(|ring| {
                ring.dimension == row.cohort_dimension && ring.value == row.cohort_value
            }) else {
                continue;
            };
            if chosen
                .get(&row.resource_key)
                .is_some_and(|(best, _)| *best <= rank)
            {
                continue;
            }
            let resource_id = ManagedResourceId::new(row.resource_id);
            let resolution = if row.published_revision_id.as_deref()
                == Some(row.revision_id.as_str())
                && row.published_bundle_digest.as_deref() == Some(row.bundle_digest.as_str())
            {
                ManagedResolution::Published {
                    publication_id: PublicationId::new(row.publication_id),
                    resource_id,
                    generation: row.generation,
                    revision_id: ResourceRevisionId::new(row.revision_id),
                    bundle_digest: AssetDigest::try_from(row.bundle_digest)?,
                }
            } else {
                ManagedResolution::IntegrityFailure {
                    resource_id,
                    generation: row.generation,
                }
            };
            chosen.insert(row.resource_key, (rank, resolution));
        }
        Ok(chosen
            .into_iter()
            .map(|(key, (_, resolution))| (key, resolution))
            .collect())
    }

    pub(crate) async fn cohort_stamp(&self, consumer: &UserId) -> Result<String> {
        if self.publication_policy().config.rings.is_empty() {
            return Ok(String::new());
        }
        let rings = self.consumer_rings(consumer).await?;
        Ok(rings
            .iter()
            .map(|ring| format!("{}={}", ring.dimension, ring.value))
            .collect::<Vec<_>>()
            .join(";"))
    }
}
