//! AI-domain repositories owned by the gateway, constructed once at router
//! build and threaded through dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_ai::repository::{
    AiQuotaBucketRepository, AiRequestPayloadRepository, AiRequestRepository,
    AiSafetyFindingRepository,
};
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct GatewayRepositories {
    pub quota_buckets: AiQuotaBucketRepository,
    pub requests: Arc<AiRequestRepository>,
    pub payloads: Arc<AiRequestPayloadRepository>,
    pub safety_findings: AiSafetyFindingRepository,
}

impl GatewayRepositories {
    pub fn new(db: &DbPool) -> Result<Self, systemprompt_ai::error::RepositoryError> {
        Ok(Self {
            quota_buckets: AiQuotaBucketRepository::new(db)?,
            requests: Arc::new(AiRequestRepository::new(db)?),
            payloads: Arc::new(AiRequestPayloadRepository::new(db)?),
            safety_findings: AiSafetyFindingRepository::new(db)?,
        })
    }
}
