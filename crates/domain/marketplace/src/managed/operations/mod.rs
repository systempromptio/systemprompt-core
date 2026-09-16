//! Durable fenced administrative operations retain typed input checkpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
mod capture;
mod credentials;
use super::{AssetDigest, ManagedError, ManagedRepository, Result};
use chrono::{DateTime, Utc};
pub use credentials::CredentialIssueStatus;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{TaskId, UserId};
/// Durable mutation status; result schemas are selected by the operation kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiOperation {
    pub id: TaskId,
    pub kind: String,
    pub state: String,
    pub fence: i64,
    pub lease_until: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // JSON: operation storage preserves the typed endpoint response for exact replay.
    pub result: Option<serde_json::Value>,
    pub problem: Option<String>,
}
/// A claim is either the retained response or a fenced right to finish its
/// input.
#[derive(Debug, Clone)]
pub enum ApiOperationClaim {
    Acquired(ApiOperation),
    Retained(ApiOperation),
}
impl ManagedRepository {
    pub async fn begin_api_operation<T: Serialize + Sync>(
        &self,
        owner: &UserId,
        id: &TaskId,
        kind: &str,
        input: &T,
    ) -> Result<ApiOperationClaim> {
        if id.as_str().is_empty()
            || id.as_str().len() > 200
            || !id
                .as_str()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
            || kind.is_empty()
            || kind.len() > 64
        {
            return Err(ManagedError::Invalid(
                "Operation identity exceeds bounds".to_owned(),
            ));
        }
        let digest = AssetDigest::of(&serde_jcs::to_vec(input)?);
        let mut tx = self.pool.begin().await?;
        let inserted=sqlx::query!("INSERT INTO managed_api_operations(owner_id,id,kind,request_digest) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING RETURNING id",owner.as_str(),id.as_str(),kind,digest.as_str()).fetch_optional(&mut *tx).await?.is_some();
        let row=sqlx::query!("SELECT kind,request_digest,state,lease_until FROM managed_api_operations WHERE owner_id=$1 AND id=$2 FOR UPDATE",owner.as_str(),id.as_str()).fetch_one(&mut *tx).await?;
        if row.kind != kind || row.request_digest != digest.as_str() {
            return Err(ManagedError::Conflict(
                "Operation key conflicts with retained input".to_owned(),
            ));
        }
        let acquired = inserted || (row.state == "pending" && row.lease_until < Utc::now());
        if acquired && !inserted {
            sqlx::query!("UPDATE managed_api_operations SET fence=fence+1,lease_until=clock_timestamp()+interval '180 seconds',updated_at=clock_timestamp() WHERE owner_id=$1 AND id=$2",owner.as_str(),id.as_str()).execute(&mut *tx).await?;
        }
        let record=sqlx::query_scalar!(r#"SELECT to_jsonb(o) AS "record!: sqlx::types::Json<ApiOperation>" FROM managed_api_operations o WHERE owner_id=$1 AND id=$2"#,owner.as_str(),id.as_str()).fetch_one(&mut *tx).await?.0;
        tx.commit().await?;
        Ok(if acquired {
            ApiOperationClaim::Acquired(record)
        } else {
            ApiOperationClaim::Retained(record)
        })
    }
    pub async fn api_operation(&self, owner: &UserId, id: &TaskId) -> Result<ApiOperation> {
        Ok(sqlx::query_scalar!(r#"SELECT to_jsonb(o) AS "record!: sqlx::types::Json<ApiOperation>" FROM managed_api_operations o WHERE owner_id=$1 AND id=$2"#,owner.as_str(),id.as_str()).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?.0)
    }
    pub async fn checkpoint_api_input<T: Serialize + serde::de::DeserializeOwned + Sync>(
        &self,
        owner: &UserId,
        operation: &ApiOperation,
        input: &T,
    ) -> Result<T> {
        let value=sqlx::query_scalar!("UPDATE managed_api_operations SET input_checkpoint=COALESCE(input_checkpoint,$4) WHERE owner_id=$1 AND id=$2 AND fence=$3 AND state='pending' RETURNING input_checkpoint",owner.as_str(),operation.id.as_str(),operation.fence,sqlx::types::Json(input) as _).fetch_optional(&self.pool).await?.flatten().ok_or_else(||ManagedError::Conflict("Operation lease was superseded".to_owned()))?;
        Ok(serde_json::from_value(value)?)
    }
    pub async fn api_input<T: serde::de::DeserializeOwned>(
        &self,
        owner: &UserId,
        operation: &ApiOperation,
    ) -> Result<Option<T>> {
        let value=sqlx::query_scalar!("SELECT input_checkpoint FROM managed_api_operations WHERE owner_id=$1 AND id=$2 AND fence=$3",owner.as_str(),operation.id.as_str(),operation.fence).fetch_optional(&self.pool).await?.flatten();
        value
            .map(serde_json::from_value)
            .transpose()
            .map_err(Into::into)
    }
    pub async fn finish_api_operation<T: Serialize + Sync>(
        &self,
        owner: &UserId,
        operation: &ApiOperation,
        result: &T,
    ) -> Result<()> {
        let row=sqlx::query!("UPDATE managed_api_operations SET state='completed',result=$4,input_checkpoint=NULL,problem=NULL,updated_at=clock_timestamp() WHERE owner_id=$1 AND id=$2 AND fence=$3 AND state='pending' RETURNING id",owner.as_str(),operation.id.as_str(),operation.fence,sqlx::types::Json(result) as _).fetch_optional(&self.pool).await?;
        if row.is_none() {
            return Err(ManagedError::Conflict(
                "Operation lease was superseded".to_owned(),
            ));
        }
        Ok(())
    }
    pub async fn fail_api_operation(&self, owner: &UserId, operation: &ApiOperation) -> Result<()> {
        sqlx::query!("UPDATE managed_api_operations SET state='failed',problem='Review retained inputs and use a new operation key after correcting the failure',updated_at=clock_timestamp() WHERE owner_id=$1 AND id=$2 AND fence=$3 AND state='pending'",owner.as_str(),operation.id.as_str(),operation.fence).execute(&self.pool).await?;
        Ok(())
    }
}
