//! Failures inside governance. None of them carries a credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_device::client::{ClientError, CredentialError};

/// Why governance could not apply an instance or reach it. Never carries a
/// credential.
#[derive(Debug, thiserror::Error)]
pub enum GovernanceError {
    #[error("the applied instance is not the instance its client talks to")]
    InstanceMismatch,
    #[error("the instance URL may not receive a session token")]
    InsecureInstance,
    #[error("no session token could be had")]
    Credential(#[source] CredentialError),
    #[error("the session token handed out has expired")]
    ExpiredToken,
    #[error("a governance call to the instance failed")]
    Hook(#[source] ClientError),
}
