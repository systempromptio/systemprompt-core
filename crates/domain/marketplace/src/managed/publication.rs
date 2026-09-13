//! Reviewed publication and generation-pinned managed resolution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use sqlx::Row;
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
}

impl ManagedRepository {
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
            sqlx::query("SELECT id FROM managed_resources WHERE id=$1 AND owner_id=$2 FOR UPDATE")
                .bind(request.resource_id.as_str())
                .bind(owner.as_str())
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(ManagedError::Unavailable)?;
        let _: String = resource.try_get("id")?;

        if let Some(existing) = sqlx::query(
            "SELECT id,review_id,generation,action,revision_id,bundle_digest,request_digest FROM managed_publications WHERE owner_id=$1 AND operation_key=$2",
        )
        .bind(owner.as_str())
        .bind(&request.operation_key)
        .fetch_optional(&mut *tx)
        .await?
        {
            let stored_digest: String = existing.try_get("request_digest")?;
            if stored_digest != request_digest.as_str() {
                return Err(ManagedError::Conflict(
                    "Publication operation key was already used for another decision".to_owned(),
                ));
            }
            let decision = decision_from_row(&request.resource_id, &existing)?;
            tx.commit().await?;
            return Ok(decision);
        }

        if let Some(revision) = &request.revision_id {
            let found = sqlx::query(
                "SELECT id FROM managed_revisions WHERE owner_id=$1 AND resource_id=$2 AND id=$3",
            )
            .bind(owner.as_str())
            .bind(request.resource_id.as_str())
            .bind(revision.as_str())
            .fetch_optional(&mut *tx)
            .await?;
            if found.is_none() {
                return Err(ManagedError::Unavailable);
            }
        }

        let current = sqlx::query(
            "SELECT generation FROM managed_publication_selections WHERE owner_id=$1 AND resource_id=$2",
        )
        .bind(owner.as_str())
        .bind(request.resource_id.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        let current_generation = current
            .as_ref()
            .map(|row| row.try_get::<i64, _>("generation"))
            .transpose()?
            .unwrap_or(0);
        if current_generation != request.expected_generation {
            return Err(ManagedError::Conflict(format!(
                "Expected publication generation {}, found {current_generation}",
                request.expected_generation
            )));
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

        sqlx::query("INSERT INTO managed_publication_reviews(id,owner_id,resource_id,revision_id,action,bundle_digest,comparison_evidence,limitations,reviewer_id,expected_generation,request_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
            .bind(review_id.as_str())
            .bind(owner.as_str())
            .bind(request.resource_id.as_str())
            .bind(revision)
            .bind(action)
            .bind(digest)
            .bind(&request.comparison_evidence)
            .bind(&request.limitations)
            .bind(reviewer.as_str())
            .bind(request.expected_generation)
            .bind(request_digest.as_str())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO managed_publications(id,owner_id,resource_id,review_id,generation,action,revision_id,bundle_digest,operation_key,request_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(publication_id.as_str())
            .bind(owner.as_str())
            .bind(request.resource_id.as_str())
            .bind(review_id.as_str())
            .bind(generation)
            .bind(action)
            .bind(revision)
            .bind(digest)
            .bind(&request.operation_key)
            .bind(request_digest.as_str())
            .execute(&mut *tx)
            .await?;
        let state = if request.action == PublicationAction::Withdraw {
            "withdrawn"
        } else {
            "published"
        };
        sqlx::query("INSERT INTO managed_publication_selections(owner_id,resource_id,generation,state,publication_id,revision_id,bundle_digest) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(owner_id,resource_id) DO UPDATE SET generation=EXCLUDED.generation,state=EXCLUDED.state,publication_id=EXCLUDED.publication_id,revision_id=EXCLUDED.revision_id,bundle_digest=EXCLUDED.bundle_digest,updated_at=now()")
            .bind(owner.as_str())
            .bind(request.resource_id.as_str())
            .bind(generation)
            .bind(state)
            .bind(publication_id.as_str())
            .bind(revision)
            .bind(digest)
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
        sqlx::query("INSERT INTO managed_distribution_outbox(id,owner_id,publication_id,generation,payload) VALUES($1,$2,$3,$4,$5)")
            .bind(outbox_id.as_str())
            .bind(owner.as_str())
            .bind(decision.publication_id.as_str())
            .bind(generation)
            .bind(payload)
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
        let resource = sqlx::query(
            "SELECT id FROM managed_resources WHERE owner_id=$1 AND kind=$2 AND resource_key=$3",
        )
        .bind(owner.as_str())
        .bind(kind.as_str())
        .bind(resource_key)
        .fetch_optional(&self.pool)
        .await?;
        let Some(resource) = resource else {
            return Ok(ManagedResolution::NotManaged);
        };
        let resource_id = ManagedResourceId::new(resource.try_get::<String, _>("id")?);
        let selection = sqlx::query("SELECT generation,state,publication_id,revision_id,bundle_digest FROM managed_publication_selections WHERE owner_id=$1 AND resource_id=$2")
            .bind(owner.as_str())
            .bind(resource_id.as_str())
            .fetch_optional(&self.pool)
            .await?;
        let Some(selection) = selection else {
            return Ok(ManagedResolution::NeverAdopted { resource_id });
        };
        resolution_from_row(resource_id, &selection)
    }

    pub async fn get_publication_bundle(
        &self,
        owner: &UserId,
        resource_id: &ManagedResourceId,
        generation: i64,
        expected_digest: &AssetDigest,
    ) -> Result<RevisionBundle> {
        let row = sqlx::query("SELECT revision_id,bundle_digest FROM managed_publications WHERE owner_id=$1 AND resource_id=$2 AND generation=$3 AND revision_id IS NOT NULL")
            .bind(owner.as_str())
            .bind(resource_id.as_str())
            .bind(generation)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ManagedError::Unavailable)?;
        let stored_digest = AssetDigest::try_from(row.try_get::<String, _>("bundle_digest")?)?;
        if &stored_digest != expected_digest {
            return Err(ManagedError::Integrity);
        }
        let revision = ResourceRevisionId::new(row.try_get::<String, _>("revision_id")?);
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

fn decision_from_row(
    resource_id: &ManagedResourceId,
    row: &sqlx::postgres::PgRow,
) -> Result<PublicationDecision> {
    let digest = row
        .try_get::<Option<String>, _>("bundle_digest")?
        .map(AssetDigest::try_from)
        .transpose()?;
    Ok(PublicationDecision {
        publication_id: PublicationId::new(row.try_get::<String, _>("id")?),
        review_id: PublicationReviewId::new(row.try_get::<String, _>("review_id")?),
        resource_id: resource_id.clone(),
        generation: row.try_get("generation")?,
        action: PublicationAction::parse(row.try_get("action")?)?,
        revision_id: row
            .try_get::<Option<String>, _>("revision_id")?
            .map(ResourceRevisionId::new),
        bundle_digest: digest,
    })
}

fn resolution_from_row(
    resource_id: ManagedResourceId,
    row: &sqlx::postgres::PgRow,
) -> Result<ManagedResolution> {
    let publication_id = PublicationId::new(row.try_get::<String, _>("publication_id")?);
    let generation = row.try_get("generation")?;
    match row.try_get::<&str, _>("state")? {
        "withdrawn" => Ok(ManagedResolution::Withdrawn {
            publication_id,
            resource_id,
            generation,
        }),
        "published" => Ok(ManagedResolution::Published {
            publication_id,
            resource_id,
            generation,
            revision_id: ResourceRevisionId::new(
                row.try_get::<Option<String>, _>("revision_id")?
                    .ok_or(ManagedError::Integrity)?,
            ),
            bundle_digest: AssetDigest::try_from(
                row.try_get::<Option<String>, _>("bundle_digest")?
                    .ok_or(ManagedError::Integrity)?,
            )?,
        }),
        _ => Err(ManagedError::Integrity),
    }
}
