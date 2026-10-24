//! Error type for gateway policies: validating `services/gateway/policies.yaml`
//! before the `gateway_policies` plane applies it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayPolicyError {
    #[error("invalid gateway policy {field}: {reason}")]
    Invalid { field: String, reason: String },
}
