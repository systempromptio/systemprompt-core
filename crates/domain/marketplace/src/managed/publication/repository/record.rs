//! The transactional half of a publication: generation admission, the review
//! and publication rows, the base or cohort selection, and the outbox event.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::gate::GateAdmission;
use super::{
    AssetDigest, EventOutboxId, ManagedError, PublicationAction, PublicationDecision,
    PublicationId, PublicationRequest, PublicationReviewId, PublicationRow, ResourceRevisionId,
    Result, UserId, decision_from_row,
};

pub(super) struct Publication<'a> {
    pub owner: &'a UserId,
    pub reviewer: &'a UserId,
    pub request: &'a PublicationRequest,
    pub request_digest: &'a str,
    pub bundle_digest: Option<AssetDigest>,
    pub generation: i64,
    pub gate: Option<GateAdmission>,
}

pub(super) async fn existing_decision(
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
            id: PublicationId::new(existing.id),
            review_id: PublicationReviewId::new(existing.review_id),
            generation: existing.generation,
            action: existing.action,
            revision_id: existing.revision_id.map(ResourceRevisionId::new),
            bundle_digest: existing.bundle_digest,
        },
    )
    .map(Some)
}

pub(super) async fn admit_generation(
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
    let current_generation = sqlx::query_scalar!(
        r#"SELECT coalesce(max(generation), 0) AS "generation!" FROM managed_publications WHERE owner_id=$1 AND resource_id=$2"#,
        owner.as_str(), request.resource_id.as_str()
    )
    .fetch_one(&mut **tx)
    .await?;
    if current_generation != request.expected_generation {
        return Err(ManagedError::Conflict(format!(
            "Expected publication generation {}, found {current_generation}",
            request.expected_generation
        )));
    }
    if let Some(cohort) = &request.cohort
        && request.action == PublicationAction::Withdraw
    {
        let selected = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM managed_publication_cohort_selections WHERE owner_id=$1 AND resource_id=$2 AND cohort_dimension=$3 AND cohort_value=$4)",
            owner.as_str(), request.resource_id.as_str(), &cohort.dimension, &cohort.value).fetch_one(&mut **tx).await?.unwrap_or(false);
        if !selected {
            return Err(ManagedError::Conflict(
                "Withdrawing a cohort requires a cohort selection to clear".to_owned(),
            ));
        }
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

pub(super) async fn record_publication(
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
        gate,
    } = publication;
    let generation = *generation;
    let review_id = PublicationReviewId::generate();
    let publication_id = PublicationId::generate();
    let outbox_id = EventOutboxId::generate();
    let action = request.action.as_str();
    let revision = request.revision_id.as_ref().map(ResourceRevisionId::as_str);
    let digest = bundle_digest.as_ref().map(AssetDigest::as_str);

    let gate_id = gate.as_ref().map(|admission| admission.gate_id.as_str());
    let gate_digest = gate
        .as_ref()
        .map(|admission| admission.evidence_digest.as_str());
    let cohort_dimension = request.cohort.as_ref().map(|c| c.dimension.as_str());
    let cohort_value = request.cohort.as_ref().map(|c| c.value.as_str());
    sqlx::query!("INSERT INTO managed_publication_reviews(id,owner_id,resource_id,revision_id,action,bundle_digest,comparison_evidence,limitations,reviewer_id,expected_generation,request_digest,gate_id,gate_evidence_digest,cohort_dimension,cohort_value) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
        review_id.as_str(), owner.as_str(), request.resource_id.as_str(), revision, action, digest,
        serde_json::to_value(&request.comparison_evidence)?, &request.limitations, reviewer.as_str(), request.expected_generation, request_digest,
        gate_id, gate_digest, cohort_dimension, cohort_value)
        .execute(&mut **tx)
        .await?;
    sqlx::query!("INSERT INTO managed_publications(id,owner_id,resource_id,review_id,generation,action,revision_id,bundle_digest,operation_key,request_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        publication_id.as_str(), owner.as_str(), request.resource_id.as_str(), review_id.as_str(), generation, action, revision, digest, &request.operation_key, request_digest)
        .execute(&mut **tx)
        .await?;
    write_selection(tx, publication, &publication_id).await?;
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

async fn write_selection(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    publication: &Publication<'_>,
    publication_id: &PublicationId,
) -> Result<()> {
    let Publication {
        owner,
        request,
        generation,
        ..
    } = publication;
    let generation = *generation;
    let digest = publication.bundle_digest.as_ref().map(AssetDigest::as_str);
    let revision = request.revision_id.as_ref().map(ResourceRevisionId::as_str);
    let withdraw = request.action == PublicationAction::Withdraw;
    match &request.cohort {
        Some(cohort) if withdraw => {
            sqlx::query!("DELETE FROM managed_publication_cohort_selections WHERE owner_id=$1 AND resource_id=$2 AND cohort_dimension=$3 AND cohort_value=$4",
                owner.as_str(), request.resource_id.as_str(), &cohort.dimension, &cohort.value)
                .execute(&mut **tx)
                .await?;
        },
        Some(cohort) => {
            let revision = revision.ok_or(ManagedError::Integrity)?;
            let digest = digest.ok_or(ManagedError::Integrity)?;
            sqlx::query!("INSERT INTO managed_publication_cohort_selections(owner_id,resource_id,cohort_dimension,cohort_value,generation,publication_id,revision_id,bundle_digest,state) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'published') ON CONFLICT(owner_id,resource_id,cohort_dimension,cohort_value) DO UPDATE SET generation=EXCLUDED.generation,publication_id=EXCLUDED.publication_id,revision_id=EXCLUDED.revision_id,bundle_digest=EXCLUDED.bundle_digest,updated_at=now()",
                owner.as_str(), request.resource_id.as_str(), &cohort.dimension, &cohort.value, generation, publication_id.as_str(), revision, digest)
                .execute(&mut **tx)
                .await?;
        },
        None => {
            let state = if withdraw { "withdrawn" } else { "published" };
            sqlx::query!("INSERT INTO managed_publication_selections(owner_id,resource_id,generation,state,publication_id,revision_id,bundle_digest) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(owner_id,resource_id) DO UPDATE SET generation=EXCLUDED.generation,state=EXCLUDED.state,publication_id=EXCLUDED.publication_id,revision_id=EXCLUDED.revision_id,bundle_digest=EXCLUDED.bundle_digest,updated_at=now()",
                owner.as_str(), request.resource_id.as_str(), generation, state, publication_id.as_str(), revision, digest)
                .execute(&mut **tx)
                .await?;
        },
    }
    Ok(())
}
