//! Device-authenticated consumer evidence and correctable attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{InstallationSessionBindingId, ManagedResourceId, UserId};
use systemprompt_models::feedback::receipts::SessionBindingRequest;

use super::{attribution, credentials, host_key};
use crate::managed::{ManagedError, ManagedRepository, Result};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConsumerSessionBinding {
    pub id: InstallationSessionBindingId,
    pub bound_at: DateTime<Utc>,
}

impl ManagedRepository {
    pub async fn bind_consumer_session(
        &self,
        credential: &str,
        request: &SessionBindingRequest,
    ) -> Result<ConsumerSessionBinding> {
        if request.session_id.as_str().is_empty() || request.session_id.as_str().len() > 512 {
            return Err(ManagedError::Invalid("Invalid native session".to_owned()));
        }
        let mut tx = self.pool.begin().await?;
        let identity = credentials::authenticate(&mut tx, credential).await?;
        let host = host_key(request.host);
        let receipt = sqlx::query!("SELECT owner_id,resource_id FROM managed_installation_receipts WHERE id=$1 AND consumer_id=$2 AND device_id=$3 AND host=$4 AND fully_verified=true AND jsonb_array_length(COALESCE(consumer_evidence->'runtime_files','[]'::jsonb))>0", request.receipt_id.as_str(), identity.consumer_id.as_str(), identity.device_id.as_str(), host)
            .fetch_optional(&mut *tx).await?.ok_or(ManagedError::Unavailable)?;
        credentials::require_grant(
            &mut tx,
            &UserId::new(receipt.owner_id),
            &ManagedResourceId::new(&receipt.resource_id),
            &identity.consumer_id,
        )
        .await?;
        attribution::lock_session(&mut tx, &identity, host, request.session_id.as_str()).await?;
        if let Some(row) = sqlx::query!("SELECT id,bound_at FROM managed_consumer_session_bindings WHERE receipt_id=$1 AND consumer_id=$2 AND device_id=$3 AND host=$4 AND native_session_id=$5", request.receipt_id.as_str(), identity.consumer_id.as_str(), identity.device_id.as_str(), host, request.session_id.as_str())
            .fetch_optional(&mut *tx).await? {
            attribution::correct_session(&mut tx, &identity, host, request.session_id.as_str()).await?;
            tx.commit().await?;
            return Ok(ConsumerSessionBinding {
                id: InstallationSessionBindingId::new(row.id),
                bound_at: row.bound_at,
            });
        }
        let conflicting = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM managed_consumer_session_bindings b JOIN managed_installation_receipts r ON r.id=b.receipt_id WHERE b.consumer_id=$1 AND b.device_id=$2 AND b.host=$3 AND b.native_session_id=$4 AND r.resource_id=$5 AND b.receipt_id<>$6)",
            identity.consumer_id.as_str(), identity.device_id.as_str(), host,
            request.session_id.as_str(), receipt.resource_id, request.receipt_id.as_str()
        ).fetch_one(&mut *tx).await?.unwrap_or(false);
        if conflicting {
            return Err(ManagedError::Conflict(
                "Native session already has a retained binding for this resource; start a new native session to use another installation".to_owned(),
            ));
        }
        let id = InstallationSessionBindingId::generate();
        sqlx::query!("INSERT INTO managed_consumer_session_bindings(id,receipt_id,consumer_id,device_id,host,native_session_id) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING", id.as_str(), request.receipt_id.as_str(), identity.consumer_id.as_str(), identity.device_id.as_str(), host, request.session_id.as_str())
            .execute(&mut *tx).await?;
        let row = sqlx::query!("SELECT id,bound_at FROM managed_consumer_session_bindings WHERE receipt_id=$1 AND consumer_id=$2 AND device_id=$3 AND host=$4 AND native_session_id=$5", request.receipt_id.as_str(), identity.consumer_id.as_str(), identity.device_id.as_str(), host, request.session_id.as_str())
            .fetch_one(&mut *tx).await?;
        attribution::correct_session(&mut tx, &identity, host, request.session_id.as_str()).await?;
        tx.commit().await?;
        Ok(ConsumerSessionBinding {
            id: InstallationSessionBindingId::new(row.id),
            bound_at: row.bound_at,
        })
    }
}
