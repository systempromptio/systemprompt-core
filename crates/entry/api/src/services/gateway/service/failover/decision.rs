//! The failover decision, kept free of I/O so it can be tested as a table:
//! which upstream errors leave the primary, and the attempt order the two
//! circuit breakers dictate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::gateway::protocol::outbound::UpstreamError;

/// Why a request left its primary provider.
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
pub fn failover_reason(error: &anyhow::Error) -> Option<FailoverReason> {
    match error.downcast_ref::<UpstreamError>()? {
        UpstreamError::Status { status, .. } if is_failover_status(*status) => {
            Some(FailoverReason::Status(*status))
        },
        UpstreamError::Status { .. } => None,
        UpstreamError::Transport { .. } => Some(FailoverReason::Transport),
    }
}

/// The order in which a request tries its upstreams, decided from the two
/// breakers before anything is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptPlan {
    PrimaryOnly,
    PrimaryThenFallback,
    FallbackOnly,
}

#[must_use]
pub const fn plan_attempts(
    has_fallback: bool,
    primary_tripped: bool,
    fallback_tripped: bool,
) -> AttemptPlan {
    match (has_fallback, primary_tripped, fallback_tripped) {
        (true, true, false) => AttemptPlan::FallbackOnly,
        (false, _, _) | (true, false, true) => AttemptPlan::PrimaryOnly,
        (true, false, false) | (true, true, true) => AttemptPlan::PrimaryThenFallback,
    }
}
