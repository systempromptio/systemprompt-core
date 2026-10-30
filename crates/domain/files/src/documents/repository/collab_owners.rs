//! The session-owner lease of a live document: which replica applies its
//! collab updates, and the address the others forward them to.
//!
//! [`DocumentCollabRepository::claim_owner`] takes or renews the lease when it
//! is free, lapsed or already held by the claimant, and otherwise reports the
//! holder's advertised address. A lease is only ever a routing hint: every
//! update is still logged through the shared, sequence-checked update log, so
//! two replicas briefly believing they own a document cost contention, never
//! correctness.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::DocumentId;

use super::collab_updates::DocumentCollabRepository;
use super::rows::RepoResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerClaim {
    Owned,
    Held { address: String },
    Lapsed,
}

impl DocumentCollabRepository {
    pub async fn claim_owner(
        &self,
        document: &DocumentId,
        owner: uuid::Uuid,
        address: &str,
        hold: Duration,
    ) -> RepoResult<OwnerClaim> {
        let claimed = sqlx::query_scalar!(
            "INSERT INTO document_collab_owners (document_id, owner_id, address, lease_until)
             VALUES ($1::text::uuid, $2, $3, now() + make_interval(secs => $4))
             ON CONFLICT (document_id) DO UPDATE
                 SET owner_id = EXCLUDED.owner_id, address = EXCLUDED.address,
                     lease_until = EXCLUDED.lease_until, updated_at = now()
                 WHERE document_collab_owners.lease_until < now()
                    OR document_collab_owners.owner_id = EXCLUDED.owner_id
             RETURNING owner_id",
            document.as_str(),
            owner,
            address,
            hold.as_secs_f64(),
        )
        .fetch_optional(&*self.pools.write)
        .await?;
        if claimed.is_some() {
            return Ok(OwnerClaim::Owned);
        }
        let holder = sqlx::query_scalar!(
            "SELECT address FROM document_collab_owners
             WHERE document_id = $1::text::uuid AND lease_until >= now()",
            document.as_str(),
        )
        .fetch_optional(&*self.pools.write)
        .await?;
        Ok(holder.map_or(OwnerClaim::Lapsed, |address| OwnerClaim::Held { address }))
    }

    pub async fn release_owner(
        &self,
        document: &DocumentId,
        owner: uuid::Uuid,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "DELETE FROM document_collab_owners
             WHERE document_id = $1::text::uuid AND owner_id = $2",
            document.as_str(),
            owner,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
