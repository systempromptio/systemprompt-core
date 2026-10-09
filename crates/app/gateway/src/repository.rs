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
use systemprompt_manifest::profile::AuditConfig;
use systemprompt_security::authz::{AuthzHookContext, NullAuditSink, SubjectProviderSet};
use systemprompt_traits::{BackgroundTasks, DynContextMaterializer};

use crate::audit::journal::{GatewayJournal, Settlement};
use crate::policies::PolicyResolver;
use crate::signature_cache::{TTL, ThoughtSignatureCache};

#[derive(Clone)]
pub struct GatewayRepositories {
    pub journal: Arc<GatewayJournal>,
    pub quota_buckets: AiQuotaBucketRepository,
    pub requests: Arc<AiRequestRepository>,
    pub payloads: Arc<AiRequestPayloadRepository>,
    pub client_evidence: Arc<AiRequestClientEvidenceRepository>,
    pub safety_findings: AiSafetyFindingRepository,
    pub gateway_policies: AiGatewayPolicyRepository,
    pub policy_resolver: PolicyResolver,
    pub thought_signatures: Arc<ThoughtSignatureCache>,
    pub context_materializer: DynContextMaterializer,
    pub artifact_ingest: Option<Arc<systemprompt_mcp::ArtifactIngest>>,
    pub sessions: Option<systemprompt_traits::DynSessionStore>,
    pub payload_cap_bytes: usize,
    pub background: BackgroundTasks,
    pub subject_providers: SubjectProviderSet,
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
        background: BackgroundTasks,
    ) -> Self {
        let requests = Arc::new(AiRequestRepository::new(db));
        Self {
            journal: Arc::new(journal),
            quota_buckets: AiQuotaBucketRepository::new(db),
            requests,
            payloads: Arc::new(AiRequestPayloadRepository::new(db)),
            client_evidence: Arc::new(AiRequestClientEvidenceRepository::new(db)),
            safety_findings: AiSafetyFindingRepository::new(db),
            gateway_policies: AiGatewayPolicyRepository::new(db),
            policy_resolver: PolicyResolver::from_repository(AiGatewayPolicyRepository::new(db)),
            thought_signatures: Arc::new(ThoughtSignatureCache::new(
                TTL,
                Arc::new(AiThoughtSignatureRepository::new(db)),
            )),
            context_materializer,
            artifact_ingest: None,
            sessions: None,
            payload_cap_bytes: AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES,
            background,
            subject_providers: SubjectProviderSet::discover(&AuthzHookContext {
                pool: db.pool(),
                sink: Arc::new(NullAuditSink),
            }),
        }
    }

    #[must_use]
    pub fn with_subject_providers(mut self, providers: SubjectProviderSet) -> Self {
        self.subject_providers = providers;
        self
    }

    #[must_use]
    pub const fn with_payload_cap(mut self, payload_cap_bytes: usize) -> Self {
        self.payload_cap_bytes = payload_cap_bytes;
        self
    }

    #[must_use]
    pub fn with_artifact_ingest(mut self, ingest: Arc<systemprompt_mcp::ArtifactIngest>) -> Self {
        self.artifact_ingest = Some(ingest);
        self
    }

    #[must_use]
    pub fn with_session_store(mut self, sessions: systemprompt_traits::DynSessionStore) -> Self {
        self.sessions = Some(sessions);
        self
    }

    pub fn settlement(&self) -> Settlement {
        Settlement {
            journal: Arc::clone(&self.journal),
            requests: Arc::clone(&self.requests),
            sessions: self.sessions.clone(),
        }
    }
}
