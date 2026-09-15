//! Device-authenticated consumer evidence and correctable attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{Postgres, Transaction};
use systemprompt_identifiers::{
    ConsumerInstallationId, InstallationReceiptId, InvocationAttributionId, ResourceRevisionId,
    UserId,
};
use systemprompt_models::feedback::receipts::AuthenticatedConsumerDevice;

use super::{ConsumerAttribution, ConsumerInvocationRequest, credentials, host_key};
use crate::managed::{ManagedError, ManagedRepository, Result};

impl ManagedRepository {
    pub async fn record_consumer_invocation(
        &self,
        credential: &str,
        request: &ConsumerInvocationRequest,
    ) -> Result<ConsumerAttribution> {
        if request.invocation_id.as_str().is_empty()
            || request.invocation_id.as_str().len() > 200
            || request.session_id.as_str().is_empty()
            || request.session_id.as_str().len() > 512
            || request.generation.is_some_and(|value| value < 1)
            || serde_jcs::to_vec(&request.evidence)?.len() > 65_536
        {
            return Err(ManagedError::Invalid(
                "Invalid invocation evidence".to_owned(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let identity = credentials::authenticate(&mut tx, credential).await?;
        let owner = sqlx::query_scalar!(
            "SELECT owner_id FROM managed_resources WHERE id=$1",
            request.resource_id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ManagedError::Unavailable)?;
        credentials::require_grant(
            &mut tx,
            &UserId::new(owner),
            &request.resource_id,
            &identity.consumer_id,
        )
        .await?;
        let host = host_key(request.host);
        lock_session(&mut tx, &identity, host, request.session_id.as_str()).await?;
        let evidence = serde_json::to_value(request)?;
        let id = InvocationAttributionId::generate();
        sqlx::query!("INSERT INTO managed_consumer_invocation_evidence(id,consumer_id,device_id,host,native_session_id,invocation_id,resource_id,installation_id,revision_id,generation,evidence,occurred_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT DO NOTHING", id.as_str(), identity.consumer_id.as_str(), identity.device_id.as_str(), host, request.session_id.as_str(), request.invocation_id.as_str(), request.resource_id.as_str(), request.installation_id.as_ref().map(ConsumerInstallationId::as_str), request.revision_id.as_ref().map(ResourceRevisionId::as_str), request.generation, evidence, request.occurred_at)
            .execute(&mut *tx).await?;
        let stored = sqlx::query!(r#"SELECT id AS "id!: InvocationAttributionId",evidence FROM managed_consumer_invocation_evidence WHERE consumer_id=$1 AND device_id=$2 AND host=$3 AND invocation_id=$4"#, identity.consumer_id.as_str(), identity.device_id.as_str(), host, request.invocation_id.as_str())
            .fetch_one(&mut *tx).await?;
        if stored.evidence != evidence {
            return Err(ManagedError::Conflict(
                "Invocation retry conflicts with immutable evidence".to_owned(),
            ));
        }
        correct_one(&mut tx, &stored.id).await?;
        let projection = sqlx::query!("SELECT receipt_id,version FROM managed_consumer_attribution_projection WHERE evidence_id=$1", stored.id.as_str())
            .fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(ConsumerAttribution {
            receipt_id: projection.receipt_id.map(InstallationReceiptId::new),
            version: projection.version,
        })
    }
}

pub(super) async fn lock_session(
    tx: &mut Transaction<'_, Postgres>,
    identity: &AuthenticatedConsumerDevice,
    host: &str,
    session: &str,
) -> Result<()> {
    let key = serde_json::to_string(&(
        identity.consumer_id.as_str(),
        identity.device_id.as_str(),
        host,
        session,
    ))?;
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 771239)) IS NULL AS locked",
        key
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn correct_session(
    tx: &mut Transaction<'_, Postgres>,
    identity: &AuthenticatedConsumerDevice,
    host: &str,
    session: &str,
) -> Result<()> {
    let mut after = String::new();
    loop {
        let rows = sqlx::query!(r#"SELECT id AS "id!: InvocationAttributionId" FROM managed_consumer_invocation_evidence WHERE consumer_id=$1 AND device_id=$2 AND host=$3 AND native_session_id=$4 AND id>$5 ORDER BY id LIMIT 128"#, identity.consumer_id.as_str(), identity.device_id.as_str(), host, session, after)
            .fetch_all(&mut **tx).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            correct_one(tx, &row.id).await?;
            after = row.id.as_str().to_owned();
        }
    }
    Ok(())
}

async fn correct_one(
    tx: &mut Transaction<'_, Postgres>,
    evidence_id: &InvocationAttributionId,
) -> Result<()> {
    let matches = sqlx::query!("SELECT DISTINCT r.id FROM managed_consumer_invocation_evidence e JOIN managed_consumer_session_bindings b ON b.consumer_id=e.consumer_id AND b.device_id=e.device_id AND b.host=e.host AND b.native_session_id=e.native_session_id JOIN managed_installation_receipts r ON r.id=b.receipt_id AND r.consumer_id=e.consumer_id AND r.device_id=e.device_id AND r.host=e.host AND r.installation_id=e.installation_id AND r.resource_id=e.resource_id AND r.generation=e.generation JOIN managed_publications p ON p.id=r.publication_id AND p.revision_id=e.revision_id WHERE e.id=$1 AND r.fully_verified=true AND jsonb_array_length(COALESCE(r.consumer_evidence->'runtime_files','[]'::jsonb))>0 LIMIT 2", evidence_id.as_str())
        .fetch_all(&mut **tx).await?;
    let receipt = if matches.len() == 1 {
        Some(matches[0].id.as_str())
    } else {
        None
    };
    let changed = sqlx::query!("INSERT INTO managed_consumer_attribution_projection(evidence_id,receipt_id) VALUES($1,$2) ON CONFLICT(evidence_id) DO UPDATE SET receipt_id=EXCLUDED.receipt_id,version=managed_consumer_attribution_projection.version+1,updated_at=now() WHERE managed_consumer_attribution_projection.receipt_id IS DISTINCT FROM EXCLUDED.receipt_id RETURNING version", evidence_id.as_str(), receipt)
        .fetch_optional(&mut **tx).await?;
    if let Some(changed) = changed {
        let reason = if receipt.is_some() {
            "authenticated_session_receipt"
        } else {
            "revision_unknown"
        };
        sqlx::query!("INSERT INTO managed_consumer_attribution_history(evidence_id,version,receipt_id,reason) VALUES($1,$2,$3,$4)", evidence_id.as_str(), changed.version, receipt, reason)
            .execute(&mut **tx).await?;
    }
    Ok(())
}
