//! Device-authenticated consumer evidence and correctable attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{InstallationReceiptId, UserId};
use systemprompt_models::feedback::ContentDigest;
use systemprompt_models::feedback::receipts::{
    ConsumerReceiptRequest, ConsumerReceiptResponse, InstallationPlanFile, ReceiptAcknowledgement,
};

use super::{credentials, host_key};
use crate::managed::{
    AssetDigest, InstalledFile, ManagedError, ManagedRepository, Result, RevisionBundle,
};

pub fn verify_readback(bundle: &RevisionBundle, request: &ConsumerReceiptRequest) -> Result<()> {
    request
        .validate()
        .map_err(|error| ManagedError::Invalid(error.to_string()))?;
    bundle.verify()?;
    if bundle.root != request.revision_id
        || bundle.digest()?.as_str() != request.bundle_digest.as_str()
    {
        return Err(ManagedError::Integrity);
    }
    let expected_count: usize = bundle
        .revisions
        .values()
        .map(|revision| revision.files.len())
        .sum();
    if expected_count != request.files.len() {
        return Err(ManagedError::Integrity);
    }
    for file in &request.files {
        let expected = bundle
            .revisions
            .get(&file.revision_id)
            .and_then(|revision| revision.files.get(&file.path))
            .ok_or(ManagedError::Integrity)?;
        if file.digest.as_str() != expected.digest.as_str()
            || file.bytes != expected.bytes
            || file.executable != expected.executable
        {
            return Err(ManagedError::Integrity);
        }
    }
    Ok(())
}

fn verify_runtime_files(
    request: &ConsumerReceiptRequest,
    expected_runtime: &[InstallationPlanFile],
) -> Result<()> {
    if request.runtime_files.is_empty() {
        return Ok(());
    }
    if request.runtime_files.len() != expected_runtime.len() {
        return Err(ManagedError::Integrity);
    }
    for expected in expected_runtime {
        let actual = request
            .runtime_files
            .iter()
            .find(|file| file.path == expected.path)
            .ok_or(ManagedError::Integrity)?;
        if actual.digest != ContentDigest::of(&expected.bytes)
            || actual.bytes != expected.bytes.len() as u64
            || actual.executable != expected.executable
        {
            return Err(ManagedError::Integrity);
        }
    }
    Ok(())
}

fn installed_files(request: &ConsumerReceiptRequest) -> Result<Vec<InstalledFile>> {
    request
        .files
        .iter()
        .map(|file| {
            Ok(InstalledFile {
                revision_id: file.revision_id.clone(),
                path: file.path.clone(),
                digest: AssetDigest::try_from(file.digest.as_str().to_owned())?,
                bytes: file.bytes,
                executable: file.executable,
            })
        })
        .collect()
}

impl ManagedRepository {
    pub async fn record_consumer_receipt(
        &self,
        credential: &str,
        request: &ConsumerReceiptRequest,
    ) -> Result<ConsumerReceiptResponse> {
        self.authenticate_consumer_device(credential).await?;
        let publication = sqlx::query!("SELECT owner_id,resource_id,revision_id,generation,bundle_digest FROM managed_publications WHERE id=$1", request.publication_id.as_str())
            .fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        if publication.resource_id != request.resource_id.as_str()
            || publication.revision_id.as_deref() != Some(request.revision_id.as_str())
            || publication.generation != request.generation
            || publication.bundle_digest.as_deref() != Some(request.bundle_digest.as_str())
        {
            return Err(ManagedError::Integrity);
        }
        let owner = UserId::new(publication.owner_id);
        let bundle = self
            .get_revision_bundle(&owner, &request.revision_id)
            .await?;
        verify_readback(&bundle, request)?;
        let key = sqlx::query_scalar!(
            "SELECT resource_key FROM managed_resources WHERE id=$1",
            request.resource_id.as_str()
        )
        .fetch_one(&self.pool)
        .await?;
        let expected_runtime = super::plan::runtime_files(&bundle, request.host, &key)?;
        verify_runtime_files(request, &expected_runtime)?;
        let mut tx = self.pool.begin().await?;
        let identity = credentials::authenticate(&mut tx, credential).await?;
        credentials::require_grant(&mut tx, &owner, &request.resource_id, &identity.consumer_id)
            .await?;
        let mut normalized = request.clone();
        normalized.runtime_files.sort_by(|a, b| a.path.cmp(&b.path));
        normalized
            .files
            .sort_by(|a, b| (&a.revision_id, &a.path).cmp(&(&b.revision_id, &b.path)));
        let evidence = serde_json::to_value(&normalized)?;
        let manifest = serde_json::to_value(installed_files(&normalized)?)?;
        let id = InstallationReceiptId::generate();
        let host = host_key(request.host);
        let verified = request.fully_verified();
        let inserted = sqlx::query!("INSERT INTO managed_installation_receipts(id,owner_id,installation_id,publication_id,resource_id,generation,bundle_digest,installed_manifest,client_evidence,consumer_id,device_id,host,consumer_evidence,fully_verified) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'{}'::jsonb,$9,$10,$11,$12,$13) ON CONFLICT DO NOTHING", id.as_str(), owner.as_str(), request.installation_id.as_str(), request.publication_id.as_str(), request.resource_id.as_str(), request.generation, request.bundle_digest.as_str(), manifest, identity.consumer_id.as_str(), identity.device_id.as_str(), host, evidence, verified)
            .execute(&mut *tx).await?.rows_affected() == 1;
        let row = sqlx::query!("SELECT id,consumer_evidence,verified_at,fully_verified FROM managed_installation_receipts WHERE consumer_id=$1 AND device_id=$2 AND host=$3 AND installation_id=$4 AND publication_id=$5", identity.consumer_id.as_str(), identity.device_id.as_str(), host, request.installation_id.as_str(), request.publication_id.as_str())
            .fetch_optional(&mut *tx).await?.ok_or_else(|| ManagedError::Conflict("Installation identity belongs to different immutable evidence".to_owned()))?;
        if row.consumer_evidence.as_ref() != Some(&evidence) {
            return Err(ManagedError::Conflict(
                "Receipt retry conflicts with immutable readback evidence".to_owned(),
            ));
        }
        tx.commit().await?;
        Ok(ConsumerReceiptResponse {
            receipt_id: InstallationReceiptId::new(row.id),
            acknowledgement: if inserted {
                ReceiptAcknowledgement::Accepted
            } else {
                ReceiptAcknowledgement::IdenticalRetry
            },
            acknowledged_at: row.verified_at,
            fully_verified: row.fully_verified.unwrap_or(false),
        })
    }

    pub async fn consumer_receipt_resource(
        &self,
        credential: &str,
        receipt: &InstallationReceiptId,
    ) -> Result<systemprompt_identifiers::ManagedResourceId> {
        let identity = self.authenticate_consumer_device(credential).await?;
        let row = sqlx::query!("SELECT resource_id FROM managed_installation_receipts WHERE id=$1 AND consumer_id=$2 AND device_id=$3", receipt.as_str(), identity.consumer_id.as_str(), identity.device_id.as_str())
            .fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        Ok(systemprompt_identifiers::ManagedResourceId::new(
            row.resource_id,
        ))
    }
}

impl ManagedRepository {
    pub async fn consumer_receipt_status(
        &self,
        credential: &str,
        receipt: &InstallationReceiptId,
    ) -> Result<ConsumerReceiptResponse> {
        let identity = self.authenticate_consumer_device(credential).await?;
        let row = sqlx::query!("SELECT id,verified_at,(fully_verified AND jsonb_array_length(COALESCE(consumer_evidence->'runtime_files','[]'::jsonb))>0) AS fully_verified FROM managed_installation_receipts WHERE id=$1 AND consumer_id=$2 AND device_id=$3", receipt.as_str(), identity.consumer_id.as_str(), identity.device_id.as_str())
            .fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        Ok(ConsumerReceiptResponse {
            receipt_id: InstallationReceiptId::new(row.id),
            acknowledgement: ReceiptAcknowledgement::IdenticalRetry,
            acknowledged_at: row.verified_at,
            fully_verified: row.fully_verified.unwrap_or(false),
        })
    }
}
