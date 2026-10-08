//! Gateway request extraction and pre-dispatch authorization.
//!
//! Turns an inbound HTTP request into a validated `PreparedRequest`:
//! extracts the credential and required headers (see [`headers`]),
//! authenticates the principal, enforces session binding, parses the canonical
//! body, classifies the client (see [`attribution`]), resolves the gateway
//! route, and runs the pre-dispatch authz check (see [`authz`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod attribution;
pub mod authz;
pub mod headers;
pub mod scope;

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use bytes::Bytes;
use std::borrow::Cow;
use std::sync::Arc;
use systemprompt_identifiers::{
    ClientSessionId, ContextId, GatewayConversationId, SessionId, TraceId, UserId,
};
use systemprompt_manifest::services::gateway::{GatewayConfig, GatewayRoute};
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_models::origin::{ClientEvidence, RequestOrigin};

use super::RequestContext;
use super::auth::{AuthedPrincipal, authenticate};
use super::error::RejectionError;
use authz::enforce_authz_pre_dispatch;
use headers::{
    classify_client_headers, optional_gateway_conversation_id, read_gateway_body,
    require_session_id,
};
use systemprompt_gateway::protocol::canonical::CanonicalRequest;
use systemprompt_gateway::protocol::inbound::InboundAdapter;

pub use attribution::AttributionHeaders;
use attribution::classify_client;
pub use authz::{GatewayAuthzRequestInput, build_gateway_authz_request};
pub(super) use headers::ClientHeaders;
pub use headers::extract_credential;
use scope::{ScopeHeaders, attribute_scopes};

/// What is known about a request at the moment it is rejected.
///
/// `origin` is fixed at entry from the route and `User-Agent`, so a rejection
/// row is never persisted without its client and wire protocol; `evidence` is
/// filled once the attribution headers and body have been classified.
#[derive(Debug)]
pub struct RejectionPartial {
    pub origin: RequestOrigin,
    pub evidence: Option<ClientEvidence>,
    pub attribution: RequestAttribution,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub context_id: Option<ContextId>,
    pub gateway_conversation_id: Option<GatewayConversationId>,
    pub client_session_id: Option<ClientSessionId>,
    pub trace_id: Option<TraceId>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub is_streaming: bool,
    pub body: Option<Bytes>,
}

impl RejectionPartial {
    pub const fn new(origin: RequestOrigin) -> Self {
        Self {
            origin,
            evidence: None,
            attribution: RequestAttribution {
                entries: Vec::new(),
                api_key_id: None,
            },
            user_id: None,
            session_id: None,
            context_id: None,
            gateway_conversation_id: None,
            client_session_id: None,
            trace_id: None,
            provider: None,
            model: None,
            max_tokens: None,
            is_streaming: false,
            body: None,
        }
    }
}

pub(super) struct PreparedRequest {
    pub origin: RequestOrigin,
    pub evidence: ClientEvidence,
    pub attribution: RequestAttribution,
    pub principal: AuthedPrincipal,
    pub body_bytes: Bytes,
    pub client_headers: ClientHeaders,
    pub gateway_request: CanonicalRequest,
    pub provider: String,
    pub upstream_model: String,
    pub session_id: SessionId,
    pub context_id: ContextId,
    pub gateway_conversation_id: GatewayConversationId,
    pub client_session_id: Option<ClientSessionId>,
}

pub(super) async fn extract_request_context(
    rc: &RequestContext<'_>,
    inbound: &Arc<dyn InboundAdapter>,
    request: Request<Body>,
    partial: &mut RejectionPartial,
) -> Result<PreparedRequest, RejectionError> {
    let gateway_config = rc
        .services
        .gateway_config()
        .filter(|g| g.enabled)
        .ok_or_else(|| RejectionError::client(StatusCode::NOT_FOUND, "Gateway not enabled"))?;

    let presented = headers::require_credential(request.headers())?;

    let client_headers = classify_client_headers(request.headers());

    let session_id = require_session_id(request.headers())?;
    partial.session_id = Some(session_id.clone());
    let header_gateway_conversation = optional_gateway_conversation_id(request.headers())?;

    let principal = authenticate(&presented, &session_id, rc.jwt_extractor, rc.ctx).await?;
    partial.user_id = Some(principal.user_id().clone());
    partial.trace_id = Some(principal.trace_id().clone());

    principal.enforce_session_binding(&session_id)?;

    let attribution = AttributionHeaders::capture(request.headers());
    let scope_headers = ScopeHeaders::capture(request.headers())?;
    let (body_bytes, mut gateway_request) = read_gateway_body(inbound, request, partial).await?;
    let evidence = classify_client(&attribution, principal.is_bridge(), &body_bytes, partial)?;
    let attribution =
        attribute_scopes(rc, gateway_config, &principal, &scope_headers, partial).await?;

    let (gateway_conversation_id, context_id, client_session_id) = derive_conversation(
        principal.user_id(),
        header_gateway_conversation,
        &gateway_request,
        partial,
    )?;
    let route = resolve_route(rc, gateway_config, &gateway_request, partial)?;
    scope::enforce_key_model_allowlist(&principal, gateway_request.model.as_str())?;
    let wire = rc
        .services
        .providers
        .find_provider(route.provider.as_str())
        .map(|p| p.wire);
    ensure_owned_context(rc, principal.user_id(), &context_id, &session_id).await?;
    rc.repos
        .thought_signatures
        .hydrate_request(
            principal.user_id(),
            &gateway_conversation_id,
            &mut gateway_request,
            wire,
        )
        .await;

    let upstream_model = upstream_model_for(
        &rc.services.providers,
        &route,
        gateway_request.model.as_str(),
    );

    enforce_authz_pre_dispatch(
        &principal,
        route.as_ref(),
        gateway_request.model.as_str(),
        &context_id,
        rc.ctx.authz_hook(),
    )
    .await?;

    Ok(PreparedRequest {
        origin: partial.origin,
        evidence,
        attribution,
        principal,
        body_bytes,
        client_headers,
        gateway_request,
        provider: route.provider.as_str().to_owned(),
        upstream_model,
        session_id,
        context_id,
        gateway_conversation_id,
        client_session_id,
    })
}

fn resolve_route<'a>(
    rc: &RequestContext<'_>,
    gateway_config: &'a GatewayConfig,
    gateway_request: &CanonicalRequest,
    partial: &mut RejectionPartial,
) -> Result<Cow<'a, GatewayRoute>, RejectionError> {
    let route = gateway_config
        .resolve_route(&rc.services.providers, gateway_request)
        .ok_or_else(|| {
            RejectionError::client(
                StatusCode::NOT_FOUND,
                format!("No gateway route matches model '{}'", gateway_request.model),
            )
        })?;
    partial.provider = Some(route.provider.as_str().to_owned());
    Ok(route)
}

// Why: the bridge supplies a thread hash even when the harness supplies its
// native session. Thread identity keys thought signatures; native session
// identity keeps compaction and helper requests in the same conversation.
pub fn derive_conversation(
    user_id: &UserId,
    header_gateway_conversation: Option<GatewayConversationId>,
    gateway_request: &CanonicalRequest,
    partial: &mut RejectionPartial,
) -> Result<(GatewayConversationId, ContextId, Option<ClientSessionId>), RejectionError> {
    let gateway_conversation_id = match header_gateway_conversation {
        Some(c) => c,
        None => gateway_request
            .derived_gateway_conversation_id()
            .ok_or_else(|| {
                RejectionError::client(
                    StatusCode::BAD_REQUEST,
                    "request body has no messages; cannot derive gateway conversation id",
                )
            })?,
    };
    let client_session_id = gateway_request
        .client_session_id()
        .map_err(|error| RejectionError::invalid(StatusCode::BAD_REQUEST, error))?;
    let context_id = client_session_id.as_ref().map_or_else(
        || ContextId::derived_from_gateway_conversation(user_id, &gateway_conversation_id),
        ContextId::derived_from_client_session,
    );
    partial.context_id = Some(context_id.clone());
    partial.gateway_conversation_id = Some(gateway_conversation_id.clone());
    partial.client_session_id.clone_from(&client_session_id);
    Ok((gateway_conversation_id, context_id, client_session_id))
}

fn upstream_model_for(
    providers: &systemprompt_manifest::services::ProviderRegistry,
    route: &GatewayRoute,
    requested: &str,
) -> String {
    providers
        .find_provider(route.provider.as_str())
        .map_or_else(
            || route.effective_upstream_model(requested).to_owned(),
            |provider| {
                provider
                    .upstream_model_for(route.upstream_model.as_deref(), requested)
                    .to_owned()
            },
        )
}

async fn ensure_owned_context(
    rc: &RequestContext<'_>,
    user_id: &UserId,
    context_id: &ContextId,
    session_id: &SessionId,
) -> Result<(), RejectionError> {
    rc.repos
        .context_materializer
        .ensure_context(systemprompt_traits::EnsureContextParams {
            context_id,
            user_id,
            session_id: Some(session_id),
            name: "Gateway conversation",
            kind: "derived",
        })
        .await
        .map_err(|error| {
            RejectionError::server(
                StatusCode::SERVICE_UNAVAILABLE,
                "conversation binding unavailable",
            )
            .with_cause(error)
        })
}
