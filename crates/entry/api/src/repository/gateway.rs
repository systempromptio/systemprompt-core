//! AI-domain repositories owned by the gateway, constructed once at router
//! build and threaded through dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_ai::repository::{
    AiGatewayPolicyRepository, AiQuotaBucketRepository, AiRequestClientEvidenceRepository,
    AiRequestPayloadRepository, AiRequestRepository, AiSafetyFindingRepository,
    AiThoughtSignatureRepository,
};
use systemprompt_database::DbPool;
use systemprompt_models::profile::AuditConfig;
use systemprompt_traits::DynContextMaterializer;

use crate::services::gateway::audit::journal::{GatewayJournal, Settlement};
use crate::services::gateway::signature_cache::{TTL, ThoughtSignatureCache};

#[derive(Clone)]
pub struct GatewayRepositories {
    pub journal: Arc<GatewayJournal>,
    pub quota_buckets: AiQuotaBucketRepository,
    pub requests: Arc<AiRequestRepository>,
    pub payloads: Arc<AiRequestPayloadRepository>,
    pub client_evidence: Arc<AiRequestClientEvidenceRepository>,
    pub safety_findings: AiSafetyFindingRepository,
    pub gateway_policies: AiGatewayPolicyRepository,
    pub thought_signatures: Arc<ThoughtSignatureCache>,
    pub context_materializer: DynContextMaterializer,
    pub artifact_ingest: Option<Arc<systemprompt_mcp::ArtifactIngest>>,
    /// `governance.audit.payload_cap_bytes`: the largest body stored whole.
    pub payload_cap_bytes: usize,
}

impl std::fmt::Debug for GatewayRepositories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayRepositories")
            .finish_non_exhaustive()
    }
}

impl GatewayRepositories {
    pub fn new(
        db: &DbPool,
        journal: GatewayJournal,
        context_materializer: DynContextMaterializer,
    ) -> Result<Self, systemprompt_ai::error::RepositoryError> {
        let requests = Arc::new(AiRequestRepository::new(db)?);
        Ok(Self {
            journal: Arc::new(journal),
            quota_buckets: AiQuotaBucketRepository::new(db)?,
            requests,
            payloads: Arc::new(AiRequestPayloadRepository::new(db)?),
            client_evidence: Arc::new(AiRequestClientEvidenceRepository::new(db)?),
            safety_findings: AiSafetyFindingRepository::new(db)?,
            gateway_policies: AiGatewayPolicyRepository::new(db)?,
            thought_signatures: Arc::new(ThoughtSignatureCache::new(
                TTL,
                Arc::new(AiThoughtSignatureRepository::new(db)?),
            )),
            context_materializer,
            artifact_ingest: None,
            payload_cap_bytes: AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES,
        })
    }

    /// Sets the payload cap from the profile's `governance.audit` block.
    #[must_use]
    pub const fn with_payload_cap(mut self, payload_cap_bytes: usize) -> Self {
        self.payload_cap_bytes = payload_cap_bytes;
        self
    }

    /// Attaches the artifact ingest so replayed `tool_result` blocks become
    /// linked artifacts.
    #[must_use]
    pub fn with_artifact_ingest(mut self, ingest: Arc<systemprompt_mcp::ArtifactIngest>) -> Self {
        self.artifact_ingest = Some(ingest);
        self
    }

    pub fn settlement(&self) -> Settlement {
        Settlement {
            journal: Arc::clone(&self.journal),
            requests: Arc::clone(&self.requests),
        }
    }
}
