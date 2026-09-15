//! Managed-resource review, publication, history, and resolution queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    AssetDigest, EventOutboxId, ManagedError, ManagedRepository, ManagedResolution,
    ManagedResourceId, PublicationAction, PublicationAdmission, PublicationDecision, PublicationId,
    PublicationRequest, PublicationReviewId, PublicationRow, ResourceKind, ResourceRevisionId,
    Result, RevisionBundle, SelectionRow, UserId, decision_from_row, request_digest,
    resolution_from_row, validate_admission, validate_request,
};

impl ManagedRepository {
    pub async fn review_and_publish(
        &self,
        owner: &UserId,
        reviewer: &UserId,
        request: &PublicationRequest,
    ) -> Result<PublicationDecision> {
        self.publish_with_admission(owner, reviewer, request, PublicationAdmission::Attested)
            .await
    }

    #[doc(hidden)]
    pub async fn publish_with_admission(
        &self,
        owner: &UserId,
        reviewer: &UserId,
        request: &PublicationRequest,
        admission: PublicationAdmission,
    ) -> Result<PublicationDecision> {
        validate_request(request)?;
        validate_admission(request, admission)?;
        let bundle_digest = if let Some(revision) = &request.revision_id {
            Some(self.get_revision_bundle(owner, revision).await?.digest()?)
        } else {
            None
        };
        let request_digest = request_digest(request, reviewer, bundle_digest.as_ref())?;
        let mut tx = self.pool.begin().await?;
        sqlx::query_scalar!(
            "SELECT 1 FROM managed_resources WHERE id=$1 AND owner_id=$2 FOR UPDATE",
            request.resource_id.as_str(),
            owner.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ManagedError::Unavailable)?;

        if let Some(decision) =
            existing_decision(&mut tx, owner, request, request_digest.as_str()).await?
        {
            tx.commit().await?;
            return Ok(decision);
        }
        let generation = admit_generation(&mut tx, owner, request, bundle_digest.as_ref()).await?;
        if admission == PublicationAdmission::Attested {
            crate::managed::evaluation::admit_improvement(
                &mut tx,
                owner,
                request,
                bundle_digest.as_ref(),
            )
            .await?;
        }
        let decision = record_publication(
            &mut tx,
            &Publication {
                owner,
                reviewer,
                request,
                request_digest: request_digest.as_str(),
                bundle_digest,
                generation,
            },
        )
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
            owner.as_str(),
            kind.as_str(),
            resource_key
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
        resolution_from_row(
            resource_id,
            SelectionRow {
                generation: selection.generation,
                state: selection.state,
                publication_id: selection.publication_id,
                revision_id: selection.revision_id,
                bundle_digest: selection.bundle_digest,
            },
        )
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

struct Publication<'a> {
    owner: &'a UserId,
    reviewer: &'a UserId,
    request: &'a PublicationRequest,
    request_digest: &'a str,
    bundle_digest: Option<AssetDigest>,
    generation: i64,
}

async fn existing_decision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserId,
    request: &PublicationRequest,
    request_digest: &str,
) -> Result<Option<PublicationDecision>> {
    let Some(existing) = sqlx::query!(
        "SELECT id,review_id,generation,action,revision_id,bundle_digest,request_digest FROM managed_publications WHERE owner_id=$1 AND operation_key=$2",
        owner.as_str(), &request.operation_key
    )
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };
    if existing.request_digest != request_digest {
        return Err(ManagedError::Conflict(
            "Publication operation key was already used for another decision".to_owned(),
        ));
    }
    decision_from_row(
        &request.resource_id,
        PublicationRow {
            id: existing.id,
            review_id: existing.review_id,
            generation: existing.generation,
            action: existing.action,
            revision_id: existing.revision_id,
            bundle_digest: existing.bundle_digest,
        },
    )
    .map(Some)
}

async fn admit_generation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserId,
    request: &PublicationRequest,
    bundle_digest: Option<&AssetDigest>,
) -> Result<i64> {
    if let Some(revision) = &request.revision_id {
        let found = sqlx::query!(
            "SELECT id FROM managed_revisions WHERE owner_id=$1 AND resource_id=$2 AND id=$3",
            owner.as_str(),
            request.resource_id.as_str(),
            revision.as_str()
        )
        .fetch_optional(&mut **tx)
        .await?;
        if found.is_none() {
            return Err(ManagedError::Unavailable);
        }
    }
    let current = sqlx::query!(
        "SELECT generation FROM managed_publication_selections WHERE owner_id=$1 AND resource_id=$2",
        owner.as_str(), request.resource_id.as_str()
    )
    .fetch_optional(&mut **tx)
    .await?;
    let current_generation = current.map_or(0, |row| row.generation);
    if current_generation != request.expected_generation {
        return Err(ManagedError::Conflict(format!(
            "Expected publication generation {}, found {current_generation}",
            request.expected_generation
        )));
    }
    if request.action == PublicationAction::Rollback {
        let revision = request.revision_id.as_ref().map(ResourceRevisionId::as_str);
        let digest = bundle_digest.map(AssetDigest::as_str);
        let retained = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_publications WHERE owner_id=$1 AND resource_id=$2 AND revision_id=$3 AND bundle_digest=$4 AND generation<=$5)",
            owner.as_str(), request.resource_id.as_str(), revision, digest, current_generation).fetch_one(&mut **tx).await?.unwrap_or(false);
        if !retained {
            return Err(ManagedError::Conflict(
                "Rollback must reference content retained by an earlier generation".to_owned(),
            ));
        }
    }
    current_generation
        .checked_add(1)
        .ok_or(ManagedError::Integrity)
}

async fn record_publication(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    publication: &Publication<'_>,
) -> Result<PublicationDecision> {
    let Publication {
        owner,
        reviewer,
        request,
        request_digest,
        bundle_digest,
        generation,
    } = publication;
    let generation = *generation;
    let review_id = PublicationReviewId::generate();
    let publication_id = PublicationId::generate();
    let outbox_id = EventOutboxId::generate();
    let action = request.action.as_str();
    let revision = request.revision_id.as_ref().map(ResourceRevisionId::as_str);
    let digest = bundle_digest.as_ref().map(AssetDigest::as_str);

    sqlx::query!("INSERT INTO managed_publication_reviews(id,owner_id,resource_id,revision_id,action,bundle_digest,comparison_evidence,limitations,reviewer_id,expected_generation,request_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        review_id.as_str(), owner.as_str(), request.resource_id.as_str(), revision, action, digest,
        serde_json::to_value(&request.comparison_evidence)?, &request.limitations, reviewer.as_str(), request.expected_generation, request_digest)
        .execute(&mut **tx)
        .await?;
    sqlx::query!("INSERT INTO managed_publications(id,owner_id,resource_id,review_id,generation,action,revision_id,bundle_digest,operation_key,request_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        publication_id.as_str(), owner.as_str(), request.resource_id.as_str(), review_id.as_str(), generation, action, revision, digest, &request.operation_key, request_digest)
        .execute(&mut **tx)
        .await?;
    let state = if request.action == PublicationAction::Withdraw {
        "withdrawn"
    } else {
        "published"
    };
    sqlx::query!("INSERT INTO managed_publication_selections(owner_id,resource_id,generation,state,publication_id,revision_id,bundle_digest) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(owner_id,resource_id) DO UPDATE SET generation=EXCLUDED.generation,state=EXCLUDED.state,publication_id=EXCLUDED.publication_id,revision_id=EXCLUDED.revision_id,bundle_digest=EXCLUDED.bundle_digest,updated_at=now()",
        owner.as_str(), request.resource_id.as_str(), generation, state, publication_id.as_str(), revision, digest)
        .execute(&mut **tx)
        .await?;
    let decision = PublicationDecision {
        publication_id,
        review_id,
        resource_id: request.resource_id.clone(),
        generation,
        action: request.action,
        revision_id: request.revision_id.clone(),
        bundle_digest: bundle_digest.clone(),
    };
    let payload = serde_json::to_value(&decision)?;
    sqlx::query!("INSERT INTO managed_distribution_outbox(id,owner_id,publication_id,generation,payload) VALUES($1,$2,$3,$4,$5)",
        outbox_id.as_str(), owner.as_str(), decision.publication_id.as_str(), generation, payload)
        .execute(&mut **tx)
        .await?;
    Ok(decision)
}
