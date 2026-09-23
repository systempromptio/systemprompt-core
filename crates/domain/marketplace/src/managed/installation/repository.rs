//! Distribution and installation receipt persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    AssetDigest, BTreeMap, ConsumerInstallationId, DeviceId, DistributionClaim, DistributionId,
    DistributionState, DistributionStatus, EventOutboxId, InstallationReceipt,
    InstallationReceiptId, InstallationReceiptRequest, ManagedError, ManagedRepository,
    ManagedResourceId, PublicationId, Result, UserId,
};

impl ManagedRepository {
    pub async fn list_distribution_status(
        &self,
        owner: &UserId,
    ) -> Result<Vec<DistributionStatus>> {
        Ok(sqlx::query_as!(DistributionStatus, r#"SELECT id AS "id: DistributionId",publication_id AS "publication_id: PublicationId",generation,status AS "status: DistributionState",claimed_at,delivered_at,error FROM managed_distribution_deliveries WHERE owner_id=$1 ORDER BY claimed_at DESC LIMIT 100"#,
            owner.as_str()).fetch_all(&self.pool).await?)
    }

    pub async fn list_installation_receipts(
        &self,
        owner: &UserId,
        resource: Option<&ManagedResourceId>,
    ) -> Result<Vec<InstallationReceipt>> {
        let rows = sqlx::query!("SELECT id,installation_id,publication_id,resource_id,generation,bundle_digest,installed_manifest,client_evidence,consumer_id,device_id,host,consumer_evidence,fully_verified,verified_at FROM managed_installation_receipts WHERE owner_id=$1 AND ($2::TEXT IS NULL OR resource_id=$2) ORDER BY verified_at DESC LIMIT 100",
            owner.as_str(), resource.map(ManagedResourceId::as_str)).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                let receipt_id = InstallationReceiptId::new(row.id);
                Ok(InstallationReceipt {
                    id: receipt_id.clone(),
                    installation_id: ConsumerInstallationId::new(row.installation_id),
                    publication_id: PublicationId::new(row.publication_id),
                    resource_id: ManagedResourceId::new(row.resource_id),
                    generation: row.generation,
                    bundle_digest: AssetDigest::try_from(row.bundle_digest)?,
                    installed_manifest: serde_json::from_value(row.installed_manifest)?,
                    client_evidence: serde_json::from_value(row.client_evidence)
                        .inspect_err(|error| {
                            tracing::warn!(receipt = %receipt_id, %error, "Installation receipt client_evidence does not decode");
                        })
                        .ok(),
                    consumer_id: row.consumer_id.map(UserId::new),
                    device_id: row.device_id.and_then(|id| {
                        DeviceId::try_new(id)
                            .inspect_err(|error| {
                                tracing::warn!(receipt = %receipt_id, %error, "Installation receipt device_id is not a device id");
                            })
                            .ok()
                    }),
                    host: row.host,
                    consumer_evidence: row.consumer_evidence.and_then(|evidence| {
                        serde_json::from_value(evidence)
                            .inspect_err(|error| {
                                tracing::warn!(receipt = %receipt_id, %error, "Installation receipt consumer_evidence does not decode");
                            })
                            .ok()
                    }),
                    fully_verified: row.fully_verified.unwrap_or(false),
                    verified_at: row.verified_at,
                })
            })
            .collect()
    }

    pub async fn claim_distribution(
        &self,
        owner: &UserId,
        claim_token: &str,
    ) -> Result<Option<DistributionClaim>> {
        if claim_token.trim().is_empty() || claim_token.len() > 200 {
            return Err(super::super::error::invalid(
                "Invalid distribution claim token",
            ));
        }
        let mut tx = self.pool.begin().await?;
        if let Some(row) = sqlx::query!("SELECT d.id,d.outbox_id,d.publication_id,d.generation,o.payload,d.claim_token FROM managed_distribution_deliveries d JOIN managed_distribution_outbox o ON o.id=d.outbox_id WHERE d.owner_id=$1 AND d.claim_token=$2",
            owner.as_str(), claim_token).fetch_optional(&mut *tx).await? {
            let claim = DistributionClaim { id: DistributionId::new(row.id), outbox_id: EventOutboxId::new(row.outbox_id), publication_id: PublicationId::new(row.publication_id), generation: row.generation, payload: serde_json::from_value(row.payload)?, claim_token: row.claim_token }; tx.commit().await?; return Ok(Some(claim));
        }
        let row = sqlx::query!("SELECT id,publication_id,generation,payload FROM managed_distribution_outbox WHERE owner_id=$1 AND delivered_at IS NULL ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1",
            owner.as_str()).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let id = DistributionId::generate();
        let outbox_id = row.id;
        let publication_id = row.publication_id;
        let generation = row.generation;
        sqlx::query!("INSERT INTO managed_distribution_deliveries(id,owner_id,outbox_id,publication_id,generation,status,claim_token) VALUES($1,$2,$3,$4,$5,'claimed',$6)",
            id.as_str(), owner.as_str(), &outbox_id, &publication_id, generation, claim_token).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(DistributionClaim {
            id,
            outbox_id: EventOutboxId::new(outbox_id),
            publication_id: PublicationId::new(publication_id),
            generation,
            payload: serde_json::from_value(row.payload)?,
            claim_token: claim_token.to_owned(),
        }))
    }

    pub async fn complete_distribution(
        &self,
        owner: &UserId,
        claim: &DistributionClaim,
        delivered: bool,
        error: Option<&str>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let status = if delivered {
            DistributionState::Distributed
        } else {
            DistributionState::Failed
        };
        let existing = sqlx::query!(r#"SELECT status AS "status: DistributionState",error,outbox_id,publication_id,generation FROM managed_distribution_deliveries WHERE id=$1 AND owner_id=$2 AND claim_token=$3 FOR UPDATE"#,
            claim.id.as_str(), owner.as_str(), &claim.claim_token).fetch_optional(&mut *tx).await?.ok_or_else(|| ManagedError::Conflict("Distribution claim is stale or unavailable".to_owned()))?;
        if existing.outbox_id != claim.outbox_id.as_str()
            || existing.publication_id != claim.publication_id.as_str()
            || existing.generation != claim.generation
        {
            return Err(ManagedError::Conflict(
                "Distribution claim does not match retained delivery".to_owned(),
            ));
        }
        if existing.status == status && existing.error.as_deref() == error {
            tx.commit().await?;
            return Ok(());
        }
        let changed = sqlx::query!("UPDATE managed_distribution_deliveries SET status=$4,delivered_at=CASE WHEN $5 THEN NOW() ELSE NULL END,error=$6 WHERE id=$1 AND owner_id=$2 AND claim_token=$3 AND status IN ('claimed','failed')",
            claim.id.as_str(), owner.as_str(), &claim.claim_token, status.as_str(), delivered, error).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(ManagedError::Conflict(
                "Distribution claim is stale or already completed".to_owned(),
            ));
        }
        if delivered {
            sqlx::query!("UPDATE managed_distribution_outbox SET delivered_at=NOW() WHERE id=$1 AND owner_id=$2 AND delivered_at IS NULL",
                claim.outbox_id.as_str(), owner.as_str()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn record_installation(
        &self,
        owner: &UserId,
        request: &InstallationReceiptRequest,
    ) -> Result<InstallationReceipt> {
        let client_evidence = serde_json::to_value(&request.client_evidence)?;
        if request.installation_id.as_str().trim().is_empty()
            || request.installation_id.as_str().len() > 200
            || request.generation < 1
            || request.files.len() > 256
            || request.client_evidence.session_id.as_str().is_empty()
            || request.client_evidence.owner_id != *owner
            || serde_jcs::to_vec(&client_evidence)?.len() > 65_536
        {
            return Err(super::super::error::invalid(
                "Invalid authenticated installation receipt",
            ));
        }
        let bundle = self
            .get_publication_bundle(
                owner,
                &request.resource_id,
                request.generation,
                &request.bundle_digest,
            )
            .await?;
        let publication = sqlx::query_scalar!("SELECT p.id FROM managed_publications p JOIN managed_distribution_outbox o ON o.publication_id=p.id AND o.owner_id=p.owner_id JOIN managed_distribution_deliveries d ON d.outbox_id=o.id AND d.owner_id=p.owner_id AND d.status='distributed' WHERE p.owner_id=$1 AND p.resource_id=$2 AND p.generation=$3",
            owner.as_str(), request.resource_id.as_str(), request.generation).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        if publication != request.publication_id.as_str() {
            return Err(ManagedError::Integrity);
        }
        let mut expected = BTreeMap::new();
        for (revision_id, manifest) in &bundle.revisions {
            for (path, file) in &manifest.files {
                expected.insert(
                    (revision_id.as_str(), path.as_str()),
                    (file.digest.as_str(), file.bytes, file.executable),
                );
            }
        }
        let supplied: BTreeMap<_, _> = request
            .files
            .iter()
            .map(|file| {
                (
                    (file.revision_id.as_str(), file.path.as_str()),
                    (file.digest.as_str(), file.bytes, file.executable),
                )
            })
            .collect();
        if supplied != expected || supplied.len() != request.files.len() {
            return Err(ManagedError::Integrity);
        }
        let id = InstallationReceiptId::generate();
        sqlx::query!("INSERT INTO managed_installation_receipts(id,owner_id,installation_id,publication_id,resource_id,generation,bundle_digest,installed_manifest,client_evidence) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(owner_id,installation_id,resource_id,generation) DO NOTHING",
            id.as_str(), owner.as_str(), request.installation_id.as_str(), request.publication_id.as_str(), request.resource_id.as_str(), request.generation, request.bundle_digest.as_str(), serde_json::to_value(&request.files)?, &client_evidence).execute(&self.pool).await?;
        let row = sqlx::query!("SELECT id,verified_at FROM managed_installation_receipts WHERE owner_id=$1 AND installation_id=$2 AND resource_id=$3 AND generation=$4 AND publication_id=$5 AND bundle_digest=$6 AND installed_manifest=$7 AND client_evidence=$8",
            owner.as_str(), request.installation_id.as_str(), request.resource_id.as_str(), request.generation, request.publication_id.as_str(), request.bundle_digest.as_str(), serde_json::to_value(&request.files)?, &client_evidence).fetch_optional(&self.pool).await?.ok_or_else(|| ManagedError::Conflict("Installation receipt conflicts with existing immutable evidence".to_owned()))?;
        Ok(InstallationReceipt {
            id: InstallationReceiptId::new(row.id),
            installation_id: request.installation_id.clone(),
            publication_id: request.publication_id.clone(),
            resource_id: request.resource_id.clone(),
            generation: request.generation,
            bundle_digest: request.bundle_digest.clone(),
            installed_manifest: request.files.clone(),
            client_evidence: Some(request.client_evidence.clone()),
            consumer_id: None,
            device_id: None,
            host: None,
            consumer_evidence: None,
            fully_verified: false,
            verified_at: row.verified_at,
        })
    }
}
