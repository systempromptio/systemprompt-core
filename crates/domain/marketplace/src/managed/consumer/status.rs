//! Device-scoped status resources preserve late attribution visibility.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::{ConsumerAttribution, ConsumerSessionBinding, host_key};
use crate::managed::{ManagedError, ManagedRepository, Result};
use systemprompt_identifiers::{InstallationSessionBindingId, ResourceInvocationId};
use systemprompt_models::feedback::EvaluatorClient;
impl ManagedRepository {
    pub async fn consumer_session_status(
        &self,
        credential: &str,
        id: &InstallationSessionBindingId,
    ) -> Result<ConsumerSessionBinding> {
        let identity = self.authenticate_consumer_device(credential).await?;
        let row=sqlx::query!("SELECT id,bound_at FROM managed_consumer_session_bindings WHERE id=$1 AND consumer_id=$2 AND device_id=$3",id.as_str(),identity.consumer_id.as_str(),identity.device_id.as_str()).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        Ok(ConsumerSessionBinding {
            id: InstallationSessionBindingId::new(row.id),
            bound_at: row.bound_at,
        })
    }
    pub async fn consumer_invocation_status(
        &self,
        credential: &str,
        id: &ResourceInvocationId,
        host: EvaluatorClient,
    ) -> Result<ConsumerAttribution> {
        let identity = self.authenticate_consumer_device(credential).await?;
        let row=sqlx::query!("SELECT p.receipt_id,p.version FROM managed_consumer_invocation_evidence e JOIN managed_consumer_attribution_projection p ON p.evidence_id=e.id WHERE e.invocation_id=$1 AND e.consumer_id=$2 AND e.device_id=$3 AND e.host=$4",id.as_str(),identity.consumer_id.as_str(),identity.device_id.as_str(),host_key(host)).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        Ok(ConsumerAttribution {
            receipt_id: row
                .receipt_id
                .map(systemprompt_identifiers::InstallationReceiptId::new),
            version: row.version,
        })
    }
}
