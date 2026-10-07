//! Boot-time claim on this process's replica identity.
//!
//! Every `services` row is keyed by `(instance_id, name)`, and each replica
//! reconciles (and reaps) only its own rows. Two live processes sharing one
//! instance id would therefore evict each other's MCP registrations. A
//! session-scoped Postgres advisory lock keyed on the instance id makes the
//! identity exclusive for the process lifetime: a second process with the
//! same id is refused at boot, and the claim vanishes with the session on a
//! crash, so no stale claim ever needs reaping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Connection, PgConnection};
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::RepositoryError;
use tracing::warn;

use super::repo::ServiceRepository;

// Why: Postgres advisory locks share one key space per database; the
// two-key form with a dedicated class keeps instance claims disjoint from
// the single-key `hashtext(job_name)` job locks.
pub const INSTANCE_CLAIM_CLASS: i32 = 0x5350_0001;

/// Failure to claim this process's replica identity.
#[derive(Debug, thiserror::Error)]
pub enum InstanceClaimError {
    #[error(
        "instance id '{instance_id}' is already claimed by a live process; give each replica its \
         own server.instance_id or leave it unset so HOSTNAME is used"
    )]
    Claimed { instance_id: InstanceId },

    #[error("could not claim instance id: {0}")]
    Repository(#[from] RepositoryError),
}

impl From<sqlx::Error> for InstanceClaimError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(RepositoryError::from(err))
    }
}

/// Exclusive claim on an instance id, held on a dedicated session.
pub struct InstanceClaim {
    session: Option<PgConnection>,
    instance_id: InstanceId,
}

impl InstanceClaim {
    pub const fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    pub async fn release(mut self) {
        if let Some(mut session) = self.session.take() {
            if let Err(e) = sqlx::query_scalar!(
                "SELECT pg_advisory_unlock($1, hashtext($2))",
                INSTANCE_CLAIM_CLASS,
                self.instance_id.as_str()
            )
            .fetch_one(&mut session)
            .await
            {
                warn!(
                    instance_id = %self.instance_id,
                    error = %e,
                    "Failed to release instance claim; closing its session"
                );
            }
            if let Err(e) = session.close().await {
                warn!(
                    instance_id = %self.instance_id,
                    error = %e,
                    "Failed to close instance claim session"
                );
            }
        }
    }
}

impl std::fmt::Debug for InstanceClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceClaim")
            .field("instance_id", &self.instance_id)
            .field("held", &self.session.is_some())
            .finish()
    }
}

impl ServiceRepository {
    pub async fn claim_instance(&self) -> Result<InstanceClaim, InstanceClaimError> {
        let mut session = self.write_pool.acquire().await?.detach();
        let acquired = sqlx::query_scalar!(
            r#"SELECT pg_try_advisory_lock($1, hashtext($2)) AS "acquired!""#,
            INSTANCE_CLAIM_CLASS,
            self.instance_id.as_str()
        )
        .fetch_one(&mut session)
        .await?;

        if !acquired {
            if let Err(e) = session.close().await {
                warn!(
                    instance_id = %self.instance_id,
                    error = %e,
                    "Failed to close refused claim session"
                );
            }
            return Err(InstanceClaimError::Claimed {
                instance_id: self.instance_id.clone(),
            });
        }

        Ok(InstanceClaim {
            session: Some(session),
            instance_id: self.instance_id.clone(),
        })
    }
}
