//! Error type for gateway-policy bootstrap: reading, parsing, validating and
//! projecting `services/gateway/policies.yaml` into `ai_gateway_policies`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_traits::RepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayPolicyError {
    #[error("failed to read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("invalid gateway policy {field}: {reason}")]
    Invalid { field: String, reason: String },

    #[error("failed to encode the spec of gateway policy {name}")]
    EncodeSpec {
        name: String,
        #[source]
        source: serde_json::Error,
    },

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
