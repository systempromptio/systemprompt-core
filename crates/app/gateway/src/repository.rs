//! AI-domain repositories owned by the gateway, constructed once at router
//! build and threaded through dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_manifest::services::ServicesView;
use systemprompt_manifest::services::catalog::ResolvedCatalog;

use systemprompt_ai::repository::{
    AiGatewayPolicyRepository, AiQuotaBucketRepository, AiRequestClientEvidenceRepository,
    AiRequestPayloadRepository, AiRequestRepository, AiSafetyFindingRepository,
    AiThoughtSignatureRepository, UserProviderCredentialRepository,
};
use systemprompt_database::DbPool;
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_governance::policy::RateWindowRepository;
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::profile::{AuditConfig, QuotaConfig, UnappliedPlaneMode};
use systemprompt_security::at_rest::AtRestCipher;
use systemprompt_traits::{BackgroundTasks, CacheEpochs, DynContextMaterializer};

use crate::audit::journal::{GatewayJournal, Settlement};
use crate::audit::tool_validity::ToolValidityCache;
use crate::policies::PolicyResolver;
use crate::signature_cache::{TTL, ThoughtSignatureCache};

/// Process-scoped handles the gateway borrows from its composition root.
#[derive(Debug, Clone)]
pub struct GatewayHandles {
    pub background: BackgroundTasks,
    pub cache_epochs: CacheEpochs,
    pub logging: LoggingRuntime,
    pub services: ServicesView,
    pub subject_providers: SubjectProviderSet,
    pub at_rest: AtRestCipher,
    pub unapplied_plane: UnappliedPlaneMode,
}

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
    pub quota_sharding: QuotaConfig,
    pub background: BackgroundTasks,
    pub logging: LoggingRuntime,
    pub subject_providers: SubjectProviderSet,
    pub rate_windows: RateWindowRepository,
    pub user_provider_credentials: UserProviderCredentialRepository,
    pub tool_validity: Arc<ToolValidityCache>,
    pub catalog: Arc<ResolvedCatalog>,
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
        handles: GatewayHandles,
    ) -> Self {
        let GatewayHandles {
            background,
            cache_epochs,
            logging,
            services,
            subject_providers,
            at_rest,
            unapplied_plane,
        } = handles;
        let requests = Arc::new(AiRequestRepository::new(db));
        Self {
            journal: Arc::new(journal),
            quota_buckets: AiQuotaBucketRepository::new(db),
            requests,
            payloads: Arc::new(AiRequestPayloadRepository::new(db)),
            client_evidence: Arc::new(AiRequestClientEvidenceRepository::new(db)),
            safety_findings: AiSafetyFindingRepository::new(db),
            gateway_policies: AiGatewayPolicyRepository::new(db),
            policy_resolver: PolicyResolver::from_repository(
                AiGatewayPolicyRepository::new(db),
                cache_epochs,
                unapplied_plane,
            ),
            thought_signatures: Arc::new(ThoughtSignatureCache::new(
                TTL,
                Arc::new(AiThoughtSignatureRepository::new(db)),
            )),
            context_materializer,
            artifact_ingest: None,
            sessions: None,
            payload_cap_bytes: AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES,
            quota_sharding: QuotaConfig::default(),
            background,
            logging,
            subject_providers,
            rate_windows: RateWindowRepository::new(db.write_pool().as_ref().clone()),
            user_provider_credentials: UserProviderCredentialRepository::new(db, at_rest),
            tool_validity: Arc::new(ToolValidityCache::default()),
            catalog: Arc::clone(&services.catalog),
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
    pub const fn with_quota_sharding(mut self, quota_sharding: QuotaConfig) -> Self {
        self.quota_sharding = quota_sharding;
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
