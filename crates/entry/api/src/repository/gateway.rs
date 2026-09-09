//! AI-domain repositories owned by the gateway, constructed once at router
//! build and threaded through dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_ai::repository::{
    AiGatewayPolicyRepository, AiQuotaBucketRepository, AiRequestPayloadRepository,
    AiRequestRepository, AiSafetyFindingRepository, AiThoughtSignatureRepository,
};
use systemprompt_database::DbPool;
use systemprompt_traits::DynContextMaterializer;

use crate::services::gateway::signature_cache::{TTL, ThoughtSignatureCache};

#[derive(Clone)]
pub struct GatewayRepositories {
    pub evaluations: systemprompt_evaluation::repository::experiments::GatewayEvaluationRepository,
    pub quota_buckets: AiQuotaBucketRepository,
    pub requests: Arc<AiRequestRepository>,
    pub payloads: Arc<AiRequestPayloadRepository>,
    pub safety_findings: AiSafetyFindingRepository,
    pub gateway_policies: AiGatewayPolicyRepository,
    pub thought_signatures: Arc<ThoughtSignatureCache>,
    pub context_materializer: DynContextMaterializer,
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
        context_materializer: DynContextMaterializer,
    ) -> Result<Self, systemprompt_ai::error::RepositoryError> {
        let pool = db.write_pool_arc().map_err(|error| {
            systemprompt_ai::error::RepositoryError::PoolInitialization(error.to_string())
        })?;
        Ok(Self {
            evaluations:
                systemprompt_evaluation::repository::experiments::GatewayEvaluationRepository::new(
                    (*pool).clone(),
                ),
            quota_buckets: AiQuotaBucketRepository::new(db)?,
            requests: Arc::new(AiRequestRepository::new(db)?),
            payloads: Arc::new(AiRequestPayloadRepository::new(db)?),
            safety_findings: AiSafetyFindingRepository::new(db)?,
            gateway_policies: AiGatewayPolicyRepository::new(db)?,
            thought_signatures: Arc::new(ThoughtSignatureCache::new(
                TTL,
                Arc::new(AiThoughtSignatureRepository::new(db)?),
            )),
            context_materializer,
        })
    }
}
