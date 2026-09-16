//! Retained publication review history and distribution evidence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ManagedRepository, ManagedResourceId, PublicationHistoryEntry, PublicationRow, Result, UserId,
    decision_from_row,
};

impl ManagedRepository {
    pub async fn list_publication_history(
        &self,
        owner: &UserId,
        resource_id: &ManagedResourceId,
    ) -> Result<Vec<PublicationHistoryEntry>> {
        let rows = sqlx::query!(r#"SELECT p.id,p.review_id,p.generation,p.action,p.revision_id,p.bundle_digest,p.created_at,r.reviewer_id,r.comparison_evidence,r.limitations,
            EXISTS(SELECT 1 FROM managed_distribution_deliveries d WHERE d.owner_id=p.owner_id AND d.publication_id=p.id AND d.status='distributed') AS "distributed!",
            EXISTS(SELECT 1 FROM managed_installation_receipts i WHERE i.owner_id=p.owner_id AND i.publication_id=p.id) AS "installation_verified!"
            FROM managed_publications p JOIN managed_publication_reviews r ON r.id=p.review_id AND r.owner_id=p.owner_id WHERE p.owner_id=$1 AND p.resource_id=$2 ORDER BY p.generation DESC"#,
            owner.as_str(), resource_id.as_str())
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(PublicationHistoryEntry {
                    decision: decision_from_row(
                        resource_id,
                        PublicationRow {
                            id: row.id,
                            review_id: row.review_id,
                            generation: row.generation,
                            action: row.action,
                            revision_id: row.revision_id,
                            bundle_digest: row.bundle_digest,
                        },
                    )?,
                    distributed: row.distributed,
                    installation_verified: row.installation_verified,
                    reviewer_id: UserId::new(row.reviewer_id),
                    comparison_evidence: serde_json::from_value(row.comparison_evidence)?,
                    limitations: row.limitations,
                    created_at: row.created_at,
                })
            })
            .collect()
    }
    pub async fn publication_history_page(
        &self,
        owner: &UserId,
        resource_id: &ManagedResourceId,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<PublicationHistoryEntry>> {
        if !(1..=100).contains(&limit) {
            return Err(super::invalid("Publication page limit must be 1–100"));
        }
        let rows = sqlx::query!(r#"SELECT p.id,p.review_id,p.generation,p.action,p.revision_id,p.bundle_digest,p.created_at,r.reviewer_id,r.comparison_evidence,r.limitations,
            EXISTS(SELECT 1 FROM managed_distribution_deliveries d WHERE d.owner_id=p.owner_id AND d.publication_id=p.id AND d.status='distributed') AS "distributed!",
            EXISTS(SELECT 1 FROM managed_installation_receipts i WHERE i.owner_id=p.owner_id AND i.publication_id=p.id) AS "installation_verified!"
            FROM managed_publications p JOIN managed_publication_reviews r ON r.id=p.review_id AND r.owner_id=p.owner_id WHERE p.owner_id=$1 AND p.resource_id=$2 AND ($3::bigint IS NULL OR p.generation<$3) ORDER BY p.generation DESC LIMIT $4"#,
            owner.as_str(), resource_id.as_str(),before,i64::from(limit))
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok(PublicationHistoryEntry {
                    decision: decision_from_row(
                        resource_id,
                        PublicationRow {
                            id: row.id,
                            review_id: row.review_id,
                            generation: row.generation,
                            action: row.action,
                            revision_id: row.revision_id,
                            bundle_digest: row.bundle_digest,
                        },
                    )?,
                    distributed: row.distributed,
                    installation_verified: row.installation_verified,
                    reviewer_id: UserId::new(row.reviewer_id),
                    comparison_evidence: serde_json::from_value(row.comparison_evidence)?,
                    limitations: row.limitations,
                    created_at: row.created_at,
                })
            })
            .collect()
    }
}
