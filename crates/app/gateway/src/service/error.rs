//! The typed failures a gateway dispatch can end in.
//!
//! [`GatewayError`] is the one error a dispatch returns: every failure the
//! pipeline knows how to answer is a variant, and anything else is
//! [`GatewayError::Internal`], which keeps its cause for the log and answers
//! with a fixed message. The HTTP layer classifies by `match`, so a new failure
//! cannot silently fall through to a generic answer carrying its own text.
//!
//! [`DispatchError`] separates a failure that happened before the audit row
//! was opened from one already recorded against it, so the route handler
//! knows whether it still owes an audit write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use systemprompt_ai::UpstreamTargetError;
use systemprompt_traits::BoxedSource;

use crate::image_fetch::ImageFetchFailed;
use crate::policies::PolicyUnavailable;
use crate::pricing::MissingPricing;
use crate::protocol::inbound::error_type_for_status;
use crate::protocol::outbound::{OutboundError, UpstreamError};

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error(transparent)]
    PreAudit(GatewayError),
    #[error(transparent)]
    Recorded(GatewayError),
}

impl DispatchError {
    pub fn pre_audit(error: impl Into<GatewayError>) -> Self {
        Self::PreAudit(error.into())
    }

    pub fn recorded(error: impl Into<GatewayError>) -> Self {
        Self::Recorded(error.into())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error(transparent)]
    PolicyDenied(#[from] PolicyDenied),
    #[error(transparent)]
    PolicyUnavailable(#[from] PolicyUnavailable),
    #[error(transparent)]
    Quota(#[from] QuotaExceeded),
    #[error(transparent)]
    GuardForbidden(#[from] GuardForbidden),
    #[error(transparent)]
    GuardUnavailable(#[from] GuardUnavailable),
    #[error(transparent)]
    Governance(#[from] GovernanceDenied),
    #[error(transparent)]
    PromptRepair(#[from] PromptRepairRequired),
    #[error(transparent)]
    Safety(#[from] SafetyBlocked),
    #[error(transparent)]
    ImageFetch(#[from] ImageFetchFailed),
    #[error(transparent)]
    Upstream(#[from] UpstreamError),
    #[error(transparent)]
    MissingPricing(#[from] MissingPricing),
    #[error(transparent)]
    UpstreamTarget(#[from] UpstreamTargetError),
    #[error("no gateway route matches model '{model}'")]
    NoRoute { model: String },
    #[error("gateway route '{route}' names provider '{provider}', which is not declared")]
    UndeclaredProvider { route: String, provider: String },
    #[error("gateway has no outbound adapter for wire protocol '{wire}'")]
    NoAdapter { wire: String },
    #[error("gateway dispatch has no authenticated session")]
    MissingSession,
    #[error("gateway {context}")]
    Internal {
        context: &'static str,
        #[source]
        source: BoxedSource,
    },
}

impl GatewayError {
    pub fn internal(context: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self::Internal {
            context,
            source: source.into(),
        }
    }

    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Self::PolicyDenied(_)
            | Self::Governance(_)
            | Self::PromptRepair(_)
            | Self::Safety(_) => StatusCode::BAD_REQUEST,
            Self::PolicyUnavailable(_) | Self::GuardUnavailable(_) => {
                StatusCode::SERVICE_UNAVAILABLE
            },
            Self::Quota(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::GuardForbidden(_) => StatusCode::FORBIDDEN,
            Self::ImageFetch(image) if image.caller_fault() => StatusCode::BAD_REQUEST,
            Self::Upstream(upstream) => upstream_status(upstream),
            Self::MissingPricing(_)
            | Self::NoRoute { .. }
            | Self::UndeclaredProvider { .. }
            | Self::NoAdapter { .. } => StatusCode::NOT_FOUND,
            Self::UpstreamTarget(target) if is_unservable_target(target) => StatusCode::NOT_FOUND,
            Self::ImageFetch(_)
            | Self::UpstreamTarget(_)
            | Self::MissingSession
            | Self::Internal { .. } => StatusCode::BAD_GATEWAY,
        }
    }

    #[must_use]
    pub fn error_type(&self) -> &'static str {
        error_type_for_status(self.status())
    }

    #[must_use]
    pub const fn upstream(&self) -> Option<&UpstreamError> {
        match self {
            Self::Upstream(upstream) => Some(upstream),
            _ => None,
        }
    }
}

impl From<OutboundError> for GatewayError {
    fn from(error: OutboundError) -> Self {
        match error {
            OutboundError::Upstream(upstream) => Self::Upstream(upstream),
            other @ (OutboundError::RenderBody { .. }
            | OutboundError::ReadBody { .. }
            | OutboundError::DecodeBody { .. }) => Self::internal("outbound request failed", other),
        }
    }
}

#[must_use]
pub fn upstream_status(error: &UpstreamError) -> StatusCode {
    match error {
        UpstreamError::Status { status, .. } => match *status {
            400 | 404 | 422 => StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_REQUEST),
            429 => StatusCode::TOO_MANY_REQUESTS,
            408 | 504 => StatusCode::GATEWAY_TIMEOUT,
            _ => StatusCode::BAD_GATEWAY,
        },
        UpstreamError::Transport { .. } => StatusCode::BAD_GATEWAY,
    }
}

// Why: a missing credential or price is a deployment that cannot serve this
// model, not an outage. Any 5xx makes the Anthropic and OpenAI SDKs retry a
// request that can never succeed; 404 is what both providers answer for a
// model they do not serve, so clients surface it once instead of looping.
const fn is_unservable_target(error: &UpstreamTargetError) -> bool {
    matches!(
        error,
        UpstreamTargetError::MissingSecret { .. }
            | UpstreamTargetError::ApiKeyOnVertex { .. }
            | UpstreamTargetError::MalformedCredential { .. }
            | UpstreamTargetError::Endpoint { .. }
    )
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PolicyDenied(pub String);

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct QuotaExceeded {
    pub message: String,
    pub retry_after_seconds: i32,
    pub detail: Option<crate::quota::QuotaDetail>,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GuardForbidden {
    pub message: String,
}

/// A denial from the typed four-stage governance chain — the same engine and
/// the same operator-configured policies that govern MCP tool calls.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GovernanceDenied {
    pub policy: String,
    pub message: String,
}

/// A secret-scan denial the gateway could not repair; `locations` names the
/// provider-payload fields the client must fix before retrying.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct PromptRepairRequired {
    pub message: String,
    pub locations: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct SafetyBlocked {
    pub category: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GuardUnavailable {
    pub message: String,
    pub retry_after_seconds: i32,
}
