//! The failover decision, kept free of I/O so it can be tested as a table:
//! which upstream errors leave a deployment, and the attempt order the
//! deployments' circuit breakers dictate: healthy deployments in chain
//! order, or every deployment in chain order when none is healthy, so a
//! request is never left unsent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::protocol::outbound::UpstreamError;
use crate::service::GatewayError;

/// Why a request left a deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverReason {
    CircuitOpen,
    Status(u16),
    Transport,
}

impl FailoverReason {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::CircuitOpen => "circuit_open".to_owned(),
            Self::Status(status) => format!("status_{status}"),
            Self::Transport => "transport".to_owned(),
        }
    }
}

#[must_use]
pub const fn is_failover_status(status: u16) -> bool {
    matches!(status, 429 | 500..=599)
}

#[must_use]
pub fn failover_reason(error: &GatewayError) -> Option<FailoverReason> {
    match error.upstream()? {
        UpstreamError::Status { status, .. } if is_failover_status(*status) => {
            Some(FailoverReason::Status(*status))
        },
        UpstreamError::Status { .. } => None,
        UpstreamError::Transport { .. } => Some(FailoverReason::Transport),
    }
}

#[must_use]
pub fn plan_attempts(tripped: &[bool]) -> Vec<usize> {
    let healthy: Vec<usize> = (0..tripped.len()).filter(|&i| !tripped[i]).collect();
    if healthy.is_empty() {
        (0..tripped.len()).collect()
    } else {
        healthy
    }
}
