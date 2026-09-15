//! Distribution, installation receipt, and invocation attribution persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    AssetDigest, BTreeMap, DistributionClaim, DistributionId, DistributionStatus,
    InstallationReceipt, InstallationReceiptId, InstallationReceiptRequest, InvocationAttribution,
    InvocationAttributionId, InvocationAttributionRequest, ManagedError, ManagedRepository,
    ManagedResourceId, PublicationId, ResourceRevisionId, Result, UserId,
};

impl ManagedRepository {
    pub async fn list_distribution_status(
        &self,
        owner: &UserId,
    ) -> Result<Vec<DistributionStatus>> {
        Ok(sqlx::query_as!(DistributionStatus, "SELECT id,publication_id,generation,status,claimed_at,delivered_at,error FROM managed_distribution_deliveries WHERE owner_id=$1 ORDER BY claimed_at DESC LIMIT 100",
            owner.as_str()).fetch_all(&self.pool).await?)
    }

    pub async fn list_installation_receipts(
        &self,
        owner: &UserId,
        resource: Option<&ManagedResourceId>,
    ) -> Result<Vec<InstallationReceipt>> {
        let rows = sqlx::query!("SELECT id,installation_id,publication_id,resource_id,generation,bundle_digest,installed_manifest,client_evidence,verified_at FROM managed_installation_receipts WHERE owner_id=$1 AND ($2::TEXT IS NULL OR resource_id=$2) ORDER BY verified_at DESC LIMIT 100",
            owner.as_str(), resource.map(ManagedResourceId::as_str)).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(InstallationReceipt {
                    id: InstallationReceiptId::new(row.id),
                    installation_id: row.installation_id,
                    publication_id: PublicationId::new(row.publication_id),
                    resource_id: ManagedResourceId::new(row.resource_id),
                    generation: row.generation,
                    bundle_digest: AssetDigest::try_from(row.bundle_digest)?,
                    installed_manifest: serde_json::from_value(row.installed_manifest)?,
                    client_evidence: row.client_evidence,
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
            let claim = DistributionClaim { id: DistributionId::new(row.id), outbox_id: row.outbox_id, publication_id: PublicationId::new(row.publication_id), generation: row.generation, payload: row.payload, claim_token: row.claim_token }; tx.commit().await?; return Ok(Some(claim));
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
            outbox_id,
            publication_id: PublicationId::new(publication_id),
            generation,
            payload: row.payload,
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
        let status = if delivered { "distributed" } else { "failed" };
        let existing = sqlx::query!("SELECT status,error,outbox_id,publication_id,generation FROM managed_distribution_deliveries WHERE id=$1 AND owner_id=$2 AND claim_token=$3 FOR UPDATE",
            claim.id.as_str(), owner.as_str(), &claim.claim_token).fetch_optional(&mut *tx).await?.ok_or_else(|| ManagedError::Conflict("Distribution claim is stale or unavailable".to_owned()))?;
        if existing.outbox_id != claim.outbox_id
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
            claim.id.as_str(), owner.as_str(), &claim.claim_token, status, delivered, error).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(ManagedError::Conflict(
                "Distribution claim is stale or already completed".to_owned(),
            ));
        }
        if delivered {
            sqlx::query!("UPDATE managed_distribution_outbox SET delivered_at=NOW() WHERE id=$1 AND owner_id=$2 AND delivered_at IS NULL",
                &claim.outbox_id, owner.as_str()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn record_installation(
        &self,
        owner: &UserId,
        request: &InstallationReceiptRequest,
    ) -> Result<InstallationReceipt> {
        let evidence_session = request
            .client_evidence
            .get("session_id")
            .and_then(serde_json::Value::as_str);
        let evidence_owner = request
            .client_evidence
            .get("owner_id")
            .and_then(serde_json::Value::as_str);
        if request.installation_id.trim().is_empty()
            || request.installation_id.len() > 200
            || request.generation < 1
            || request.files.len() > 256
            || evidence_session.is_none_or(str::is_empty)
            || evidence_owner != Some(owner.as_str())
            || serde_jcs::to_vec(&request.client_evidence)?.len() > 65_536
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
            id.as_str(), owner.as_str(), &request.installation_id, request.publication_id.as_str(), request.resource_id.as_str(), request.generation, request.bundle_digest.as_str(), serde_json::to_value(&request.files)?, &request.client_evidence).execute(&self.pool).await?;
        let row = sqlx::query!("SELECT id,verified_at FROM managed_installation_receipts WHERE owner_id=$1 AND installation_id=$2 AND resource_id=$3 AND generation=$4 AND publication_id=$5 AND bundle_digest=$6 AND installed_manifest=$7 AND client_evidence=$8",
            owner.as_str(), &request.installation_id, request.resource_id.as_str(), request.generation, request.publication_id.as_str(), request.bundle_digest.as_str(), serde_json::to_value(&request.files)?, &request.client_evidence).fetch_optional(&self.pool).await?.ok_or_else(|| ManagedError::Conflict("Installation receipt conflicts with existing immutable evidence".to_owned()))?;
        Ok(InstallationReceipt {
            id: InstallationReceiptId::new(row.id),
            installation_id: request.installation_id.clone(),
            publication_id: request.publication_id.clone(),
            resource_id: request.resource_id.clone(),
            generation: request.generation,
            bundle_digest: request.bundle_digest.clone(),
            installed_manifest: request.files.clone(),
            client_evidence: request.client_evidence.clone(),
            verified_at: row.verified_at,
        })
    }

    pub async fn attribute_invocation(
        &self,
        owner: &UserId,
        request: &InvocationAttributionRequest,
    ) -> Result<InvocationAttribution> {
        let session = request
            .authenticated_evidence
            .get("session_id")
            .and_then(serde_json::Value::as_str);
        let attested_owner = request
            .authenticated_evidence
            .get("owner_id")
            .and_then(serde_json::Value::as_str);
        if request.invocation_id.trim().is_empty()
            || request.invocation_id.len() > 200
            || session.is_none_or(str::is_empty)
            || attested_owner.is_some_and(|value| value != owner.as_str())
            || serde_jcs::to_vec(&request.authenticated_evidence)?.len() > 65_536
        {
            return Err(super::super::error::invalid(
                "Invalid authenticated invocation attribution",
            ));
        }
        let verified = if let (Some(installation), Some(key), Some(revision), Some(generation)) = (
            request.installation_id.as_deref(),
            request.resource_key.as_deref(),
            request.resource_revision_id.as_ref(),
            request.publication_generation,
        ) {
            sqlx::query!("SELECT r.id AS receipt_id,r.resource_id,r.generation,p.revision_id FROM managed_installation_receipts r JOIN managed_resources m ON m.id=r.resource_id AND m.owner_id=r.owner_id JOIN managed_publications p ON p.id=r.publication_id AND p.owner_id=r.owner_id WHERE r.owner_id=$1 AND r.installation_id=$2 AND m.resource_key=$3 AND r.client_evidence->>'session_id'=$4 AND r.generation=$5 AND p.revision_id=$6 ORDER BY r.verified_at DESC LIMIT 1",
                owner.as_str(), installation, key, session, generation, revision.as_str()).fetch_optional(&self.pool).await?
        } else {
            None
        };
        let id = InvocationAttributionId::generate();
        let (status, receipt_id, resource_id, revision_id, generation) = if let Some(row) = verified
        {
            (
                "verified",
                Some(row.receipt_id),
                Some(row.resource_id),
                row.revision_id,
                Some(row.generation),
            )
        } else {
            ("revision_unknown", None, None, None, None)
        };
        sqlx::query!("INSERT INTO managed_invocation_attributions(id,owner_id,invocation_id,installation_id,resource_id,revision_id,publication_generation,traffic_class,status,receipt_id,authenticated_evidence) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT(owner_id,invocation_id) DO NOTHING",
            id.as_str(), owner.as_str(), &request.invocation_id, request.installation_id.as_deref(), resource_id.as_deref(), revision_id.as_deref(), generation, request.traffic_class.as_str(), status, receipt_id.as_deref(), &request.authenticated_evidence).execute(&self.pool).await?;
        let stored = sqlx::query!("SELECT id,installation_id,resource_id,revision_id,publication_generation,traffic_class,status,authenticated_evidence FROM managed_invocation_attributions WHERE owner_id=$1 AND invocation_id=$2",
            owner.as_str(), &request.invocation_id).fetch_one(&self.pool).await?;
        if stored.installation_id != request.installation_id
            || stored.resource_id != resource_id
            || stored.revision_id != revision_id
            || stored.publication_generation != generation
            || stored.traffic_class != request.traffic_class.as_str()
            || stored.status != status
            || stored.authenticated_evidence != request.authenticated_evidence
        {
            return Err(ManagedError::Conflict(
                "Invocation attribution replay conflicts with immutable evidence".to_owned(),
            ));
        }
        Ok(InvocationAttribution {
            id: InvocationAttributionId::new(stored.id),
            invocation_id: request.invocation_id.clone(),
            installation_id: stored.installation_id,
            resource_id: stored.resource_id.map(ManagedResourceId::new),
            revision_id: stored.revision_id.map(ResourceRevisionId::new),
            publication_generation: stored.publication_generation,
            traffic_class: request.traffic_class,
            status: stored.status,
        })
    }
}
