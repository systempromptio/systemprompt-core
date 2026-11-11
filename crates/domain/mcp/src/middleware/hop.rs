//! Verification of the proxy's hop token on every request a managed MCP
//! server receives.
//!
//! The API proxy is the only caller that can produce a token this layer
//! accepts: RS256 with the deployment key, `typ` `mcp-hop+jwt`, audience
//! `mcp:<this server>`, a one-minute expiry. A request that presents one has
//! its identity taken from the token's claims and from nothing else it
//! carries; a request that presents none is anonymous, and a request that
//! presents one that does not verify, or that names a different MCP session
//! than the one the token was minted for, is refused with 401 before the MCP
//! service sees it. The verified principal also scopes session restore: a
//! session evicted from memory is restored only for the server and user it is
//! bound to (see [`restore_principal`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use http::{HeaderMap, StatusCode};
use systemprompt_identifiers::{Actor, McpServerId, UserId, headers};
use systemprompt_models::RequestContext as SysRequestContext;
use systemprompt_models::auth::{McpHopClaims, TokenAuthority, TokenError, TokenInteraction};
use systemprompt_models::execution::context::UserInteractionMode;
use systemprompt_traits::{ContextPropagation, ContextPropagationError};

const SESSION_HEADER: &str = "mcp-session-id";

tokio::task_local! {
    static RESTORE_PRINCIPAL: RestorePrincipal;
}

/// The server and verified user a request runs as, for session restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePrincipal {
    pub server_id: McpServerId,
    pub user_id: Option<UserId>,
}

#[must_use]
pub fn restore_principal() -> Option<RestorePrincipal> {
    RESTORE_PRINCIPAL.try_with(Clone::clone).ok()
}

pub async fn run_as<F: Future>(principal: RestorePrincipal, future: F) -> F::Output {
    RESTORE_PRINCIPAL.scope(principal, future).await
}

#[derive(Debug, thiserror::Error)]
pub enum HopRejection {
    #[error("the hop token header is not valid UTF-8")]
    MalformedHeader(#[source] http::header::ToStrError),
    #[error("the hop token was refused: {0}")]
    Token(#[source] TokenError),
    #[error("this server requires authentication and the request carries no hop token")]
    Missing,
    #[error("the hop token was minted for a different MCP session")]
    SessionMismatch,
}

/// Verifies hop tokens for one MCP server.
#[derive(Clone)]
pub struct HopVerifier {
    tokens: Arc<dyn TokenAuthority>,
    server_id: McpServerId,
    required: bool,
}

impl std::fmt::Debug for HopVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HopVerifier")
            .field("server_id", &self.server_id)
            .finish_non_exhaustive()
    }
}

impl HopVerifier {
    pub fn new(tokens: Arc<dyn TokenAuthority>, server_id: McpServerId) -> Self {
        Self {
            tokens,
            server_id,
            required: false,
        }
    }

    #[must_use]
    pub const fn requiring_hop(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub const fn server_id(&self) -> &McpServerId {
        &self.server_id
    }

    pub fn verify(&self, headers: &HeaderMap) -> Result<Option<VerifiedHop>, HopRejection> {
        let Some(raw) = headers.get(headers::MCP_HOP) else {
            if self.required {
                return Err(HopRejection::Missing);
            }
            return Ok(None);
        };
        let token = raw.to_str().map_err(HopRejection::MalformedHeader)?;
        let claims = self
            .tokens
            .verify_mcp_hop(token, &self.server_id)
            .map_err(HopRejection::Token)?;
        let presented = headers
            .get(SESSION_HEADER)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty());
        if presented != claims.identity.mcp_session.as_deref() {
            return Err(HopRejection::SessionMismatch);
        }
        Ok(Some(VerifiedHop(claims)))
    }
}

/// The claims of a hop token this server verified.
#[derive(Debug, Clone)]
pub struct VerifiedHop(McpHopClaims);

impl VerifiedHop {
    pub const fn claims(&self) -> &McpHopClaims {
        &self.0
    }

    pub fn principal(&self) -> Option<&UserId> {
        self.0.identity.user.as_ref().map(|user| &user.id)
    }

    pub fn request_context(
        &self,
        headers: &HeaderMap,
    ) -> Result<SysRequestContext, ContextPropagationError> {
        let identity = &self.0.identity;
        let mut context = SysRequestContext::from_headers(headers)?;
        context.request.session_id = identity.session_id.clone();
        context.request.client_id.clone_from(&identity.client_id);
        let actor = identity.user.as_ref().map_or_else(
            || Actor::anonymous(identity.user_id.clone()),
            |user| Actor::user(user.id.clone()),
        );
        let mut context = context
            .with_actor(actor)
            .with_user_type(identity.user_type)
            .with_act_chain(identity.act_chain.clone());
        if let Some(user) = identity.user.clone() {
            context = context.with_user(user);
        }
        if let Some(purpose) = identity.purpose.clone() {
            context = context.with_caller_purpose(purpose);
        }
        if let Some(interaction) = identity.interaction {
            context = context.with_interaction_mode(match interaction {
                TokenInteraction::Interactive => UserInteractionMode::Interactive,
                TokenInteraction::NonInteractive => UserInteractionMode::NonInteractive,
            });
        }
        Ok(context)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HopAdmissionError {
    #[error(transparent)]
    Rejected(#[from] HopRejection),
    #[error("the request-context headers are invalid")]
    Context(#[source] ContextPropagationError),
}

pub fn admit_request(
    verifier: &HopVerifier,
    headers: &HeaderMap,
    extensions: &mut http::Extensions,
) -> Result<RestorePrincipal, HopAdmissionError> {
    extensions.remove::<VerifiedHop>();
    extensions.remove::<SysRequestContext>();
    let hop = verifier.verify(headers)?;
    let principal = RestorePrincipal {
        server_id: verifier.server_id.clone(),
        user_id: hop.as_ref().and_then(VerifiedHop::principal).cloned(),
    };
    if let Some(hop) = hop {
        let context = hop
            .request_context(headers)
            .map_err(HopAdmissionError::Context)?;
        extensions.insert(context);
        extensions.insert(hop);
    }
    Ok(principal)
}

pub async fn verify_hop(
    State(verifier): State<HopVerifier>,
    mut request: Request,
    next: Next,
) -> Response {
    let (mut parts, body) = request.into_parts();
    let admitted = admit_request(&verifier, &parts.headers, &mut parts.extensions);
    request = Request::from_parts(parts, body);
    match admitted {
        Ok(principal) => run_as(principal, next.run(request)).await,
        Err(HopAdmissionError::Rejected(error)) => {
            if matches!(error, HopRejection::Token(TokenError::UnknownKid(_))) {
                tracing::error!(server = %verifier.server_id, %error, "Refused an MCP request signed by a key this server does not know; restart it to load the deployment's current signing keys");
            } else {
                tracing::warn!(server = %verifier.server_id, %error, "Refused an MCP request whose hop token did not verify");
            }
            (StatusCode::UNAUTHORIZED, error.to_string()).into_response()
        },
        Err(error @ HopAdmissionError::Context(_)) => {
            tracing::warn!(server = %verifier.server_id, %error, "Refused an MCP request with invalid request-context headers");
            (StatusCode::BAD_REQUEST, error.to_string()).into_response()
        },
    }
}
