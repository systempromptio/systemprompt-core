//! Device-authenticated installation plans for a published revision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::credentials;
use crate::managed::consumer::plan::{PlanIdentity, build_plan};
use crate::managed::{ManagedError, ManagedRepository, Result};
use systemprompt_identifiers::{ManagedResourceId, PublicationId, UserId};
use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_models::feedback::receipts::ConsumerInstallationPlan;

impl ManagedRepository {
    pub async fn consumer_installation_plan(
        &self,
        credential: &str,
        resource: &ManagedResourceId,
        publication: &PublicationId,
        host: EvaluatorClient,
    ) -> Result<ConsumerInstallationPlan> {
        self.authenticate_consumer_device(credential).await?;
        let row = sqlx::query!("SELECT owner_id,revision_id,generation,bundle_digest FROM managed_publications WHERE id=$1 AND resource_id=$2 AND revision_id IS NOT NULL", publication.as_str(), resource.as_str()).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        let owner = UserId::new(row.owner_id);
        let revision = systemprompt_identifiers::ResourceRevisionId::new(
            row.revision_id.ok_or(ManagedError::Integrity)?,
        );
        let bundle = self.get_revision_bundle(&owner, &revision).await?;
        let key = sqlx::query_scalar!(
            "SELECT resource_key FROM managed_resources WHERE id=$1",
            resource.as_str()
        )
        .fetch_one(&self.pool)
        .await?;
        let plan = build_plan(
            &bundle,
            PlanIdentity {
                publication_id: publication.clone(),
                resource_id: resource.clone(),
                generation: row.generation,
                host,
            },
            &key,
        )?;
        if Some(plan.bundle_digest.as_str()) != row.bundle_digest.as_deref() {
            return Err(ManagedError::Integrity);
        }
        let mut tx = self.pool.begin().await?;
        let identity = credentials::authenticate(&mut tx, credential).await?;
        credentials::require_grant(&mut tx, &owner, resource, &identity.consumer_id).await?;
        tx.commit().await?;
        Ok(plan)
    }
}
