//! Errors of an in-process gateway call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::StatusCode;
use systemprompt_extension::gateway_guard::{GatewayDenyKind, GatewayDenyReason};

use crate::service::{DispatchError, GatewayError, upstream_status};

/// Why an in-process gateway call produced no completion.
#[derive(Debug, thiserror::Error)]
pub enum GatewayClientError {
    #[error("gateway denied the request: {}", reason.message)]
    Denied {
        reason: GatewayDenyReason,
        #[source]
        source: DispatchError,
    },
    #[error("gateway quota exhausted for the service principal")]
    Quota {
        retry_after_seconds: i32,
        #[source]
        source: DispatchError,
    },
    #[error("upstream answered {status}")]
    Upstream { status: StatusCode, body: String },
    #[error("gateway response could not be decoded")]
    Decode(#[source] DecodeError),
    #[error("gateway call timed out")]
    Timeout,
    #[error("model '{model}' has no gateway route")]
    Route {
        model: String,
        #[source]
        source: systemprompt_manifest::services::catalog::ResolveError,
    },
    #[error("model '{model}' resolves to no servable deployment")]
    NoRoute { model: String },
    #[error("service principal could not be resolved")]
    Principal(#[source] PrincipalError),
    #[error(transparent)]
    Dispatch(#[from] DispatchError),
}

/// Why a gateway response body could not become a completion.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("response body could not be read")]
    Body(#[source] axum::Error),
    #[error("response body is not JSON")]
    Json(#[source] serde_json::Error),
    #[error("response body is not an Anthropic Messages response")]
    Wire(#[source] systemprompt_wire::error::WireParseError),
    #[error("structured output is not valid JSON")]
    Structured(#[source] serde_json::Error),
}

/// Why the service principal's identity or session could not be minted.
#[derive(Debug, thiserror::Error)]
pub enum PrincipalError {
    #[error("service account could not be ensured")]
    Account(#[source] systemprompt_users::UserError),
    #[error("service account email belongs to a non-service account")]
    NotService,
    #[error("service principal identifier is invalid")]
    Identity(#[source] systemprompt_identifiers::error::IdValidationError),
    #[error("gateway session could not be minted")]
    Session(#[source] systemprompt_traits::AnalyticsProviderError),
}

impl GatewayClientError {
    pub fn from_dispatch(error: DispatchError) -> Self {
        let gateway = match &error {
            DispatchError::PreAudit(e) | DispatchError::Recorded(e) => e,
        };
        match gateway {
            GatewayError::Quota(quota) => Self::Quota {
                retry_after_seconds: quota.retry_after_seconds,
                source: error,
            },
            GatewayError::Upstream(upstream) => Self::Upstream {
                status: upstream_status(upstream),
                body: upstream.to_string(),
            },
            other => match deny_reason(other) {
                Some(reason) => Self::Denied {
                    reason,
                    source: error,
                },
                None => Self::Dispatch(error),
            },
        }
    }
}

fn deny_reason(error: &GatewayError) -> Option<GatewayDenyReason> {
    let (message, kind, retry_after_seconds) = match error {
        GatewayError::GuardForbidden(f) => (f.message.clone(), GatewayDenyKind::Forbidden, 0),
        GatewayError::GuardUnavailable(u) => (
            u.message.clone(),
            GatewayDenyKind::Unavailable,
            u.retry_after_seconds,
        ),
        GatewayError::Governance(g) => (g.message.clone(), GatewayDenyKind::Forbidden, 0),
        GatewayError::Safety(s) => (s.message.clone(), GatewayDenyKind::Forbidden, 0),
        GatewayError::PolicyDenied(p) => (p.0.clone(), GatewayDenyKind::Forbidden, 0),
        GatewayError::PromptRepair(r) => (r.message.clone(), GatewayDenyKind::Forbidden, 0),
        _ => return None,
    };
    Some(GatewayDenyReason {
        message,
        retry_after_seconds,
        kind,
    })
}
