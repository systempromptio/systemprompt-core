//! Bounded authoring inspection does not imply a revision is published.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ManagedRepository;
use crate::managed::error::invalid;
use crate::managed::{AssetDigest, Result};
use serde::Serialize;
use systemprompt_identifiers::{ManagedResourceId, ManagedSourceId, ResourceRevisionId, UserId};

#[derive(Debug, Clone, Serialize)]
pub struct ResourceSummary {
    pub id: ManagedResourceId,
    pub source_id: ManagedSourceId,
    pub source_name: String,
    pub kind: String,
    pub resource_key: String,
    pub revision_count: i64,
    pub latest_revision: Option<ResourceRevisionId>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RevisionSummary {
    pub id: ResourceRevisionId,
    pub digest: AssetDigest,
    pub parent_id: Option<ResourceRevisionId>,
    pub rationale: String,
    pub created_at: String,
}

impl ManagedRepository {
    pub async fn list_resources(
        &self,
        owner: &UserId,
        offset: i64,
    ) -> Result<Vec<ResourceSummary>> {
        validate_offset(offset)?;
        let rows = sqlx::query!(r#"SELECT r.id,r.source_id,s.name AS source_name,r.kind,r.resource_key,
            (SELECT count(*) FROM managed_revisions v WHERE v.resource_id=r.id) AS "revision_count!",
            (SELECT v.id FROM managed_revisions v WHERE v.resource_id=r.id ORDER BY v.created_at DESC,v.id DESC LIMIT 1) AS "latest_revision?"
            FROM managed_resources r JOIN managed_sources s ON s.id=r.source_id AND s.owner_id=r.owner_id
            WHERE r.owner_id=$1 ORDER BY r.kind,r.resource_key,r.id LIMIT 51 OFFSET $2"#,
            owner.as_str(), offset).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| ResourceSummary {
                id: ManagedResourceId::new(row.id),
                source_id: ManagedSourceId::new(row.source_id),
                source_name: row.source_name,
                kind: row.kind,
                resource_key: row.resource_key,
                revision_count: row.revision_count,
                latest_revision: row.latest_revision.map(ResourceRevisionId::new),
            })
            .collect())
    }

    pub async fn list_revisions(
        &self,
        owner: &UserId,
        resource: &ManagedResourceId,
        offset: i64,
    ) -> Result<Vec<RevisionSummary>> {
        validate_offset(offset)?;
        let resource_exists = sqlx::query_scalar!(
            "SELECT id FROM managed_resources WHERE owner_id=$1 AND id=$2",
            owner.as_str(),
            resource.as_str()
        )
        .fetch_optional(&self.pool)
        .await?;
        if resource_exists.is_none() {
            return Err(crate::managed::ManagedError::Unavailable);
        }

        let rows = sqlx::query!(r#"SELECT id,digest,parent_id,rationale,created_at::text AS "created_at!" FROM managed_revisions v WHERE owner_id=$1 AND resource_id=$2 ORDER BY v.created_at DESC,v.id DESC LIMIT 51 OFFSET $3"#,
            owner.as_str(), resource.as_str(), offset).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(RevisionSummary {
                    id: ResourceRevisionId::new(row.id),
                    digest: AssetDigest::try_from(row.digest)?,
                    parent_id: row.parent_id.map(ResourceRevisionId::new),
                    rationale: row.rationale,
                    created_at: row.created_at,
                })
            })
            .collect()
    }
}

fn validate_offset(offset: i64) -> Result<()> {
    if !(0..=500_000).contains(&offset) {
        return Err(invalid("Invalid listing offset"));
    }
    Ok(())
}
