//! Reviewed publication and generation-pinned managed resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    EventOutboxId, ManagedResourceId, PublicationId, PublicationReviewId, ResourceRevisionId,
    UserId,
};

use super::error::invalid;
use super::{AssetDigest, ManagedError, ManagedRepository, ResourceKind, Result, RevisionBundle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationAction {
    InitialAdoption,
    PublishImprovement,
    Withdraw,
    Rollback,
}

impl PublicationAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InitialAdoption => "initial_adoption",
            Self::PublishImprovement => "publish_improvement",
            Self::Withdraw => "withdraw",
            Self::Rollback => "rollback",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "initial_adoption" => Ok(Self::InitialAdoption),
            "publish_improvement" => Ok(Self::PublishImprovement),
            "withdraw" => Ok(Self::Withdraw),
            "rollback" => Ok(Self::Rollback),
            _ => Err(ManagedError::Integrity),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationRequest {
    pub resource_id: ManagedResourceId,
    pub revision_id: Option<ResourceRevisionId>,
    pub action: PublicationAction,
    pub expected_generation: i64,
    pub operation_key: String,
    pub comparison_evidence: serde_json::Value,
    pub limitations: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationDecision {
    pub publication_id: PublicationId,
    pub review_id: PublicationReviewId,
    pub resource_id: ManagedResourceId,
    pub generation: i64,
    pub action: PublicationAction,
    pub revision_id: Option<ResourceRevisionId>,
    pub bundle_digest: Option<AssetDigest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ManagedResolution {
    NotManaged,
    NeverAdopted {
        resource_id: ManagedResourceId,
    },
    Published {
        publication_id: PublicationId,
        resource_id: ManagedResourceId,
        generation: i64,
        revision_id: ResourceRevisionId,
        bundle_digest: AssetDigest,
    },
    Withdrawn {
        publication_id: PublicationId,
        resource_id: ManagedResourceId,
        generation: i64,
    },
    IntegrityFailure {
        resource_id: ManagedResourceId,
        generation: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationHistoryEntry {
    pub decision: PublicationDecision,
    pub approved: bool,
    pub distributed: bool,
    pub installation_verified: bool,
    pub reviewer_id: UserId,
    pub comparison_evidence: serde_json::Value,
    pub limitations: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

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
        rows.into_iter().map(|row| {
            Ok(PublicationHistoryEntry {
                decision: decision_from_fields(resource_id, row.id, row.review_id, row.generation, &row.action, row.revision_id, row.bundle_digest)?,
                approved: true,
                distributed: row.distributed,
                installation_verified: row.installation_verified,
                reviewer_id: UserId::new(row.reviewer_id),
                comparison_evidence: row.comparison_evidence,
                limitations: row.limitations,
                created_at: row.created_at,
            })
        }).collect()
    }
    pub async fn review_and_publish(
        &self,
        owner: &UserId,
        reviewer: &UserId,
        request: &PublicationRequest,
    ) -> Result<PublicationDecision> {
        validate_request(request)?;
        let bundle_digest = if let Some(revision) = &request.revision_id {
            Some(self.get_revision_bundle(owner, revision).await?.digest()?)
        } else {
            None
        };
        let request_digest = request_digest(request, reviewer, bundle_digest.as_ref())?;
        let mut tx = self.pool.begin().await?;
        let resource =
            sqlx::query!("SELECT id FROM managed_resources WHERE id=$1 AND owner_id=$2 FOR UPDATE",
                request.resource_id.as_str(), owner.as_str())
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(ManagedError::Unavailable)?;
        let _ = resource.id;

        if let Some(existing) = sqlx::query!(
            "SELECT id,review_id,generation,action,revision_id,bundle_digest,request_digest FROM managed_publications WHERE owner_id=$1 AND operation_key=$2",
            owner.as_str(), &request.operation_key
        )
        .fetch_optional(&mut *tx)
        .await?
        {
            let stored_digest = existing.request_digest;
            if stored_digest != request_digest.as_str() {
                return Err(ManagedError::Conflict(
                    "Publication operation key was already used for another decision".to_owned(),
                ));
            }
            let decision = decision_from_fields(&request.resource_id, existing.id, existing.review_id, existing.generation, &existing.action, existing.revision_id, existing.bundle_digest)?;
            tx.commit().await?;
            return Ok(decision);
        }

        if let Some(revision) = &request.revision_id {
            let found = sqlx::query!(
                "SELECT id FROM managed_revisions WHERE owner_id=$1 AND resource_id=$2 AND id=$3",
                owner.as_str(), request.resource_id.as_str(), revision.as_str()
            )
            .fetch_optional(&mut *tx)
            .await?;
            if found.is_none() {
                return Err(ManagedError::Unavailable);
            }
        }

        let current = sqlx::query!(
            "SELECT generation FROM managed_publication_selections WHERE owner_id=$1 AND resource_id=$2",
            owner.as_str(), request.resource_id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?;
        let current_generation = current.map(|row| row.generation).unwrap_or(0);
        if current_generation != request.expected_generation {
            return Err(ManagedError::Conflict(format!(
                "Expected publication generation {}, found {current_generation}",
                request.expected_generation
            )));
        }
        if request.action == PublicationAction::Rollback {
            let revision = request.revision_id.as_ref().map(ResourceRevisionId::as_str);
            let digest = bundle_digest.as_ref().map(AssetDigest::as_str);
            let retained = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_publications WHERE owner_id=$1 AND resource_id=$2 AND revision_id=$3 AND bundle_digest=$4 AND generation<=$5)",
                owner.as_str(), request.resource_id.as_str(), revision, digest, current_generation).fetch_one(&mut *tx).await?.unwrap_or(false);
            if !retained { return Err(ManagedError::Conflict("Rollback must reference content retained by an earlier generation".to_owned())); }
        }
        let generation = current_generation
            .checked_add(1)
            .ok_or(ManagedError::Integrity)?;
        let review_id = PublicationReviewId::generate();
        let publication_id = PublicationId::generate();
        let outbox_id = EventOutboxId::generate();
        let action = request.action.as_str();
        let revision = request.revision_id.as_ref().map(ResourceRevisionId::as_str);
        let digest = bundle_digest.as_ref().map(AssetDigest::as_str);

        sqlx::query!("INSERT INTO managed_publication_reviews(id,owner_id,resource_id,revision_id,action,bundle_digest,comparison_evidence,limitations,reviewer_id,expected_generation,request_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
            review_id.as_str(), owner.as_str(), request.resource_id.as_str(), revision, action, digest,
            &request.comparison_evidence, &request.limitations, reviewer.as_str(), request.expected_generation, request_digest.as_str())
            .execute(&mut *tx)
            .await?;
        sqlx::query!("INSERT INTO managed_publications(id,owner_id,resource_id,review_id,generation,action,revision_id,bundle_digest,operation_key,request_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            publication_id.as_str(), owner.as_str(), request.resource_id.as_str(), review_id.as_str(), generation, action, revision, digest, &request.operation_key, request_digest.as_str())
            .execute(&mut *tx)
            .await?;
        let state = if request.action == PublicationAction::Withdraw {
            "withdrawn"
        } else {
            "published"
        };
        sqlx::query!("INSERT INTO managed_publication_selections(owner_id,resource_id,generation,state,publication_id,revision_id,bundle_digest) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(owner_id,resource_id) DO UPDATE SET generation=EXCLUDED.generation,state=EXCLUDED.state,publication_id=EXCLUDED.publication_id,revision_id=EXCLUDED.revision_id,bundle_digest=EXCLUDED.bundle_digest,updated_at=now()",
            owner.as_str(), request.resource_id.as_str(), generation, state, publication_id.as_str(), revision, digest)
            .execute(&mut *tx)
            .await?;
        let decision = PublicationDecision {
            publication_id,
            review_id,
            resource_id: request.resource_id.clone(),
            generation,
            action: request.action,
            revision_id: request.revision_id.clone(),
            bundle_digest,
        };
        let payload = serde_json::to_value(&decision)?;
        sqlx::query!("INSERT INTO managed_distribution_outbox(id,owner_id,publication_id,generation,payload) VALUES($1,$2,$3,$4,$5)",
            outbox_id.as_str(), owner.as_str(), decision.publication_id.as_str(), generation, payload)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(decision)
    }

    pub async fn resolve_managed(
        &self,
        owner: &UserId,
        kind: ResourceKind,
        resource_key: &str,
    ) -> Result<ManagedResolution> {
        let resource = sqlx::query!(
            "SELECT id FROM managed_resources WHERE owner_id=$1 AND kind=$2 AND resource_key=$3",
            owner.as_str(), kind.as_str(), resource_key
        )
        .fetch_optional(&self.pool)
        .await?;
        let Some(resource) = resource else {
            return Ok(ManagedResolution::NotManaged);
        };
        let resource_id = ManagedResourceId::new(resource.id);
        let selection = sqlx::query!("SELECT generation,state,publication_id,revision_id,bundle_digest FROM managed_publication_selections WHERE owner_id=$1 AND resource_id=$2",
            owner.as_str(), resource_id.as_str())
            .fetch_optional(&self.pool)
            .await?;
        let Some(selection) = selection else {
            return Ok(ManagedResolution::NeverAdopted { resource_id });
        };
        resolution_from_fields(resource_id, selection.generation, &selection.state, selection.publication_id, selection.revision_id, selection.bundle_digest)
    }

    pub async fn get_publication_bundle(
        &self,
        owner: &UserId,
        resource_id: &ManagedResourceId,
        generation: i64,
        expected_digest: &AssetDigest,
    ) -> Result<RevisionBundle> {
        let row = sqlx::query!(r#"SELECT revision_id AS "revision_id!",bundle_digest AS "bundle_digest!" FROM managed_publications WHERE owner_id=$1 AND resource_id=$2 AND generation=$3 AND revision_id IS NOT NULL"#,
            owner.as_str(), resource_id.as_str(), generation)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ManagedError::Unavailable)?;
        let stored_digest = AssetDigest::try_from(row.bundle_digest)?;
        if &stored_digest != expected_digest {
            return Err(ManagedError::Integrity);
        }
        let revision = ResourceRevisionId::new(row.revision_id);
        let bundle = self.get_revision_bundle(owner, &revision).await?;
        if bundle.digest()? != stored_digest {
            return Err(ManagedError::Integrity);
        }
        Ok(bundle)
    }
}

fn validate_request(request: &PublicationRequest) -> Result<()> {
    if request.expected_generation < 0
        || request.operation_key.trim().is_empty()
        || request.operation_key.len() > 200
        || request.limitations.len() > 4000
        || serde_jcs::to_vec(&request.comparison_evidence)?.len() > 65_536
        || (request.action == PublicationAction::PublishImprovement
            && request.comparison_evidence.as_object().is_none_or(serde_json::Map::is_empty))
    {
        return Err(invalid("Invalid publication review input"));
    }
    let revision_required = request.action != PublicationAction::Withdraw;
    if revision_required != request.revision_id.is_some()
        || (request.action == PublicationAction::InitialAdoption
            && request.expected_generation != 0)
        || (request.action != PublicationAction::InitialAdoption
            && request.expected_generation == 0)
    {
        return Err(invalid(
            "Publication action does not match its generation or revision",
        ));
    }
    Ok(())
}

fn request_digest(
    request: &PublicationRequest,
    reviewer: &UserId,
    bundle_digest: Option<&AssetDigest>,
) -> Result<AssetDigest> {
    Ok(AssetDigest::of(&serde_jcs::to_vec(&serde_json::json!({
        "request": request,
        "reviewer_id": reviewer,
        "bundle_digest": bundle_digest,
    }))?))
}

fn decision_from_fields(
    resource_id: &ManagedResourceId,
    id: String,
    review_id: String,
    generation: i64,
    action: &str,
    revision_id: Option<String>,
    bundle_digest: Option<String>,
) -> Result<PublicationDecision> {
    let digest = bundle_digest.map(AssetDigest::try_from).transpose()?;
    Ok(PublicationDecision {
        publication_id: PublicationId::new(id),
        review_id: PublicationReviewId::new(review_id),
        resource_id: resource_id.clone(),
        generation,
        action: PublicationAction::parse(action)?,
        revision_id: revision_id.map(ResourceRevisionId::new),
        bundle_digest: digest,
    })
}

fn resolution_from_fields(
    resource_id: ManagedResourceId,
    generation: i64,
    state: &str,
    publication_id: String,
    revision_id: Option<String>,
    bundle_digest: Option<String>,
) -> Result<ManagedResolution> {
    let publication_id = PublicationId::new(publication_id);
    match state {
        "withdrawn" => Ok(ManagedResolution::Withdrawn {
            publication_id,
            resource_id,
            generation,
        }),
        "published" => Ok(ManagedResolution::Published {
            publication_id,
            resource_id,
            generation,
            revision_id: ResourceRevisionId::new(revision_id.ok_or(ManagedError::Integrity)?),
            bundle_digest: AssetDigest::try_from(bundle_digest.ok_or(ManagedError::Integrity)?)?,
        }),
        _ => Err(ManagedError::Integrity),
    }
}
