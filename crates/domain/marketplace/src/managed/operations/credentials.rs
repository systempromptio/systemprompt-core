//! Atomic credential issuance records delivery status without retaining tokens.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::ApiOperation;
use crate::managed::error::integrity;
use crate::managed::{AssetDigest, ManagedError, ManagedRepository, Result};
use systemprompt_identifiers::{DeviceCertId, DeviceId, InstallationReceiptId, UserId};
/// Credential issue status is replayable; plaintext is delivered once only.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CredentialIssueStatus {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub token_retrievable: bool,
    pub retry_action: String,
}
impl ManagedRepository {
    pub async fn issue_api_consumer_credential(
        &self,
        owner: &UserId,
        operation: &ApiOperation,
        cert: &DeviceCertId,
    ) -> Result<(CredentialIssueStatus, Option<String>)> {
        let mut tx = self.pool.begin().await?;
        let current=sqlx::query!("SELECT fence,state,result FROM managed_api_operations WHERE owner_id=$1 AND id=$2 FOR UPDATE",owner.as_str(),operation.id.as_str()).fetch_one(&mut *tx).await?;
        if current.state == "completed" {
            return Ok((
                serde_json::from_value(current.result.ok_or(ManagedError::Unavailable)?)?,
                None,
            ));
        }
        if current.fence != operation.fence || current.state != "pending" {
            return Err(ManagedError::Conflict(
                "Operation lease was superseded".to_owned(),
            ));
        }
        let consumer = sqlx::query_scalar!(
            "SELECT consumer_id AS \"consumer_id!\" FROM public.active_device_identity($1)",
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
        let digest = AssetDigest::of(credential.as_bytes());
        sqlx::query!("INSERT INTO managed_consumer_credentials(device_id,credential_digest,issuance_operation) VALUES($1,$2,$3) ON CONFLICT(device_id) DO UPDATE SET credential_digest=EXCLUDED.credential_digest,issuance_operation=EXCLUDED.issuance_operation,revoked_at=NULL,created_at=clock_timestamp()",cert.as_str(),digest.as_str(),operation.id.as_str()).execute(&mut *tx).await?;
        let status=CredentialIssueStatus{device_id:DeviceId::try_new(cert.as_str()).map_err(integrity)?,consumer_id:UserId::new(consumer),token_retrievable:false,retry_action:"Token is delivered once. If its response was lost, deliberately rotate with a new operation key.".to_owned()};
        sqlx::query!("UPDATE managed_api_operations SET state='completed',result=$3,updated_at=clock_timestamp() WHERE owner_id=$1 AND id=$2",owner.as_str(),operation.id.as_str(),sqlx::types::Json(&status) as _).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok((status, Some(credential)))
    }
}
