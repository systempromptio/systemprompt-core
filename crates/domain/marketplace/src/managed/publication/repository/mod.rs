//! Managed-resource review, publication, history, and resolution queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catalog_batch;
mod history;
mod record;

pub use catalog_batch::SkillResolutionRow;

use record::{Publication, admit_generation, existing_decision, record_publication};

use super::gate::{GateAdmission, GateContext};
use super::{
    AssetDigest, EventOutboxId, ManagedError, ManagedRepository, ManagedResolution,
    ManagedResourceId, PublicationAction, PublicationAdmission, PublicationDecision, PublicationId,
    PublicationRequest, PublicationReviewId, PublicationRow, ResourceKind, ResourceRevisionId,
    Result, RevisionBundle, SelectionRow, UserId, decision_from_row, invalid, request_digest,
    resolution_from_row, validate_admission, validate_request,
};

impl ManagedRepository {
    pub async fn review_and_publish(
        &self,
        owner: &UserId,
        reviewer: &UserId,
        request: &PublicationRequest,
    ) -> Result<PublicationDecision> {
        self.publish_with_admission(owner, reviewer, request, PublicationAdmission::Reviewed)
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
        let gate = self
            .admit_through_gates(owner, reviewer, request, bundle_digest.as_ref())
            .await?;
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
        let decision = record_publication(
            &mut tx,
            &Publication {
                owner,
                reviewer,
                request,
                request_digest: request_digest.as_str(),
                bundle_digest,
                generation,
                gate,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(decision)
    }

    async fn admit_through_gates(
        &self,
        owner: &UserId,
        reviewer: &UserId,
        request: &PublicationRequest,
        bundle_digest: Option<&AssetDigest>,
    ) -> Result<Option<GateAdmission>> {
        let policy = self.publication_policy();
        if let Some(cohort) = &request.cohort
            && policy
                .config
                .ring(&cohort.dimension, &cohort.value)
                .is_none()
        {
            return Err(invalid("Publication cohort is not a configured ring"));
        }
        let resource_kind = sqlx::query_scalar!(
            r#"SELECT kind AS "kind: ResourceKind" FROM managed_resources WHERE id=$1 AND owner_id=$2"#,
            request.resource_id.as_str(),
            owner.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ManagedError::Unavailable)?;
        policy
            .admit(&GateContext {
                owner,
                reviewer,
                action: request.action,
                resource_id: &request.resource_id,
                resource_kind,
                revision_id: request.revision_id.as_ref(),
                bundle_digest,
                cohort: request.cohort.as_ref(),
                comparison_evidence: &request.comparison_evidence,
            })
            .await
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
                publication_id: PublicationId::new(selection.publication_id),
                revision_id: selection.revision_id.map(ResourceRevisionId::new),
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
