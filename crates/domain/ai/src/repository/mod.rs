//! Repository layer for AI domain persistence.
//!
//! Every type here owns `SQLx` queries against the AI domain tables
//! (`ai_requests`, `ai_request_messages`, `ai_tool_calls`,
//! `ai_request_payloads`, `ai_quota_buckets`, `ai_safety_findings`,
//! `ai_gateway_policies`).
//!
//! All repositories return [`crate::error::RepositoryError`]. Services are
//! the only callers — repositories never execute application logic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod ai_gateway_policies;
pub mod ai_quota_buckets;
pub mod ai_request_payloads;
pub mod ai_requests;
pub mod ai_safety_findings;

use crate::error::RepositoryError;
use systemprompt_database::DbPool;

pub use ai_gateway_policies::{AiGatewayPolicyRepository, GatewayPolicyRow};
pub use ai_quota_buckets::{
    AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta, QuotaBucketState,
};
pub use ai_request_payloads::{AiRequestPayload, AiRequestPayloadRepository, UpsertPayloadParams};
pub use ai_requests::{AiRequestRepository, InsertToolCallParams};
pub use ai_safety_findings::{AiSafetyFindingRepository, InsertSafetyFinding};

/// Bundle of the AI-domain repositories, constructed once at a composition
/// root and cloned by consumers.
#[derive(Debug, Clone)]
pub struct AiRepositories {
    pub requests: AiRequestRepository,
    pub payloads: AiRequestPayloadRepository,
    pub gateway_policies: AiGatewayPolicyRepository,
    pub quota_buckets: AiQuotaBucketRepository,
    pub safety_findings: AiSafetyFindingRepository,
}

impl AiRepositories {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        Ok(Self {
            requests: AiRequestRepository::new(db)?,
            payloads: AiRequestPayloadRepository::new(db)?,
            gateway_policies: AiGatewayPolicyRepository::new(db)?,
            quota_buckets: AiQuotaBucketRepository::new(db)?,
            safety_findings: AiSafetyFindingRepository::new(db)?,
        })
    }
}
