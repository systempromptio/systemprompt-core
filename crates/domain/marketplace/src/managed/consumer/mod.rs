//! Device-authenticated consumer evidence and correctable attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod attribution;
mod credentials;
mod plan;
mod receipts;
mod sessions;
mod status;

pub use credentials::IssuedConsumerCredential;
pub use receipts::verify_readback;
pub use sessions::ConsumerSessionBinding;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ConsumerInstallationId, InstallationReceiptId, ManagedResourceId, NativeSessionId,
    ResourceInvocationId, ResourceRevisionId,
};
use systemprompt_models::feedback::EvaluatorClient;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerInvocationRequest {
    pub invocation_id: ResourceInvocationId,
    pub host: EvaluatorClient,
    pub session_id: NativeSessionId,
    pub resource_id: ManagedResourceId,
    pub installation_id: Option<ConsumerInstallationId>,
    pub revision_id: Option<ResourceRevisionId>,
    pub generation: Option<i64>,
    pub occurred_at: DateTime<Utc>,
    pub evidence: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConsumerAttribution {
    pub receipt_id: Option<InstallationReceiptId>,
    pub version: i64,
}

pub const fn host_key(host: EvaluatorClient) -> &'static str {
    match host {
        EvaluatorClient::ClaudeCode => "claude-code",
        EvaluatorClient::OpenCode => "opencode",
        EvaluatorClient::Codex => "codex",
        EvaluatorClient::Hermes => "hermes",
        EvaluatorClient::ClaudeDesktop => "claude-desktop",
    }
}
