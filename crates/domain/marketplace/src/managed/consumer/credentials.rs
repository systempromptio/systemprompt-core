//! Device-authenticated consumer evidence and correctable attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Postgres, Transaction};
use systemprompt_identifiers::{DeviceCertId, DeviceId, InstallationReceiptId, UserId};
use systemprompt_models::feedback::ContentDigest;
use systemprompt_models::feedback::receipts::AuthenticatedConsumerDevice;

use crate::managed::error::integrity;
use crate::managed::{ManagedError, ManagedRepository, Result};

#[derive(Serialize, schemars::JsonSchema)]
pub struct IssuedConsumerCredential {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub credential: String,
}

impl std::fmt::Debug for IssuedConsumerCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedConsumerCredential")
            .field("device_id", &self.device_id)
            .field("consumer_id", &self.consumer_id)
            .finish_non_exhaustive()
    }
}

impl ManagedRepository {
    pub async fn issue_consumer_credential(
        &self,
        cert: &DeviceCertId,
    ) -> Result<IssuedConsumerCredential> {
        let mut tx = self.pool.begin().await?;
        let record = sqlx::query!(
            "SELECT consumer_id AS \"user_id!\" FROM public.active_device_identity($1)",
            cert.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ManagedError::Unavailable)?;
        let credential = format!(
            "sp_device_{}{}",
            InstallationReceiptId::generate(),
            InstallationReceiptId::generate()
        );
        let digest = ContentDigest::of(credential.as_bytes());
        sqlx::query!("INSERT INTO managed_consumer_credentials(device_id,credential_digest) VALUES($1,$2) ON CONFLICT(device_id) DO UPDATE SET credential_digest=EXCLUDED.credential_digest,revoked_at=NULL,created_at=now()", cert.as_str(), digest.as_str())
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(IssuedConsumerCredential {
            device_id: DeviceId::try_new(cert.as_str()).map_err(integrity)?,
            consumer_id: UserId::new(record.user_id),
            credential,
        })
    }

    pub async fn revoke_consumer_credential(&self, cert: &DeviceCertId) -> Result<()> {
        sqlx::query!("UPDATE managed_consumer_credentials SET revoked_at=COALESCE(revoked_at,now()) WHERE device_id=$1", cert.as_str())
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn set_consumer_grant(
        &self,
        owner: &UserId,
        resource: &systemprompt_identifiers::ManagedResourceId,
        consumer: &UserId,
        enabled: bool,
    ) -> Result<()> {
        let revoked_at: Option<DateTime<Utc>> = if enabled { None } else { Some(Utc::now()) };
        sqlx::query!("INSERT INTO managed_consumer_grants(owner_id,resource_id,consumer_id,revoked_at) VALUES($1,$2,$3,$4) ON CONFLICT(owner_id,resource_id,consumer_id) DO UPDATE SET revoked_at=EXCLUDED.revoked_at", owner.as_str(), resource.as_str(), consumer.as_str(), revoked_at)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn authenticate_consumer_device(
        &self,
        token: &str,
    ) -> Result<AuthenticatedConsumerDevice> {
        let mut tx = self.pool.begin().await?;
        let identity = authenticate(&mut tx, token).await?;
        tx.commit().await?;
        Ok(identity)
    }
}

pub(super) async fn authenticate(
    tx: &mut Transaction<'_, Postgres>,
    token: &str,
) -> Result<AuthenticatedConsumerDevice> {
    if !token.starts_with("sp_device_") || token.len() > 256 {
        return Err(ManagedError::Unavailable);
    }
    let digest = ContentDigest::of(token.as_bytes());
    let row = sqlx::query!("SELECT c.device_id,d.consumer_id AS \"user_id!\" FROM managed_consumer_credentials c CROSS JOIN LATERAL public.active_device_identity(c.device_id) d WHERE c.credential_digest=$1 AND c.revoked_at IS NULL FOR SHARE OF c", digest.as_str())
        .fetch_optional(&mut **tx).await?.ok_or(ManagedError::Unavailable)?;
    Ok(AuthenticatedConsumerDevice {
        consumer_id: UserId::new(row.user_id),
        device_id: DeviceId::try_new(row.device_id).map_err(integrity)?,
    })
}

pub(super) async fn require_grant(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserId,
    resource: &systemprompt_identifiers::ManagedResourceId,
    consumer: &UserId,
) -> Result<()> {
    sqlx::query!("SELECT consumer_id FROM managed_consumer_grants WHERE owner_id=$1 AND resource_id=$2 AND consumer_id=$3 AND revoked_at IS NULL FOR SHARE", owner.as_str(), resource.as_str(), consumer.as_str())
        .fetch_optional(&mut **tx).await?.ok_or(ManagedError::Unavailable)?;
    Ok(())
}

impl ManagedRepository {
    pub async fn retain_consumer_catalog_grant(
        &self,
        owner: &UserId,
        resource: &systemprompt_identifiers::ManagedResourceId,
        consumer: &UserId,
    ) -> Result<()> {
        sqlx::query!("INSERT INTO managed_consumer_grants(owner_id,resource_id,consumer_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", owner.as_str(), resource.as_str(), consumer.as_str()).execute(&self.pool).await?;
        Ok(())
    }
}

impl ManagedRepository {
    pub async fn consumer_resource_owner(
        &self,
        resource: &systemprompt_identifiers::ManagedResourceId,
    ) -> Result<UserId> {
        let owner = sqlx::query_scalar!(
            "SELECT owner_id FROM managed_resources WHERE id=$1",
            resource.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ManagedError::Unavailable)?;
        Ok(UserId::new(owner))
    }
}
