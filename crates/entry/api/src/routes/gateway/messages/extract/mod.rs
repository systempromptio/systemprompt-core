//! Gateway request extraction and pre-dispatch authorization.
//!
//! Turns an inbound HTTP request into a validated `PreparedRequest`:
//! extracts the credential and required headers (see [`headers`]),
//! authenticates the principal and binds its session, parses the canonical
//! body, classifies the client (see [`attribution`]), resolves the gateway
//! route, and runs the pre-dispatch authz check (see [`authz`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod attribution;
pub mod authz;
pub mod headers;
mod partial;
pub mod scope;
pub mod signatures;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use std::sync::Arc;
use systemprompt_gateway::{RouteResolution, RouteSource};
use systemprompt_identifiers::{
    ClientSessionId, ContextId, GatewayConversationId, SessionId, UserId,
};
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_models::origin::{ClientEvidence, RequestOrigin};

use super::RequestContext;
use super::auth::{AuthInputs, AuthedPrincipal, authenticate};
use super::error::RejectionError;
use authz::{DispatchTarget, enforce_authz_pre_dispatch};
use headers::{
    classify_client_headers, optional_gateway_conversation_id, optional_session_id,
    read_gateway_body,
};
use systemprompt_gateway::protocol::canonical::CanonicalRequest;
use systemprompt_gateway::protocol::inbound::InboundAdapter;

pub use attribution::AttributionHeaders;
use attribution::classify_client;
pub use authz::{GatewayAuthzRequestInput, build_gateway_authz_request};
pub(super) use headers::ClientHeaders;
pub use headers::extract_credential;
pub use partial::RejectionPartial;
use scope::{ScopeHeaders, attribute_scopes};

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

    let header_session = optional_session_id(request.headers());
    let header_gateway_conversation = optional_gateway_conversation_id(request.headers())?;

    let principal = authenticate_request(
        rc,
        &presented,
        header_session.as_ref(),
        request.headers(),
        partial,
    )
    .await?;
    let session_id = principal.session_id().clone();

    let attribution = AttributionHeaders::capture(request.headers());
    let scope_headers = ScopeHeaders::capture(request.headers())?;
    let (body_bytes, mut gateway_request) = read_gateway_body(inbound, request, partial).await?;
    let evidence = classify_client(&attribution, principal.is_bridge(), &body_bytes, partial)?;
    let attribution = attribute_scopes(rc, gateway_config, &principal, &scope_headers, partial)?;

    let (gateway_conversation_id, context_id, body_client_session) = derive_conversation(
        principal.user_id(),
        header_gateway_conversation,
        &gateway_request,
        partial,
    )?;
    let client_session_id = body_client_session.or_else(|| principal.client_session.clone());
    partial.client_session_id.clone_from(&client_session_id);
    let resolution = resolve_route(rc, &gateway_request, &attribution, partial)?;
    scope::enforce_key_model_allowlist(&principal, gateway_request.model.as_str())?;
    let signatures_required = signatures::route_needs_thought_signatures(&resolution);
    ensure_owned_context(rc, principal.user_id(), &context_id, &session_id).await?;
    rc.repos
        .thought_signatures
        .hydrate_request(
            principal.user_id(),
            &gateway_conversation_id,
            &mut gateway_request,
            signatures_required,
        )
        .await;

    let primary = resolution
        .primary()
        .ok_or_else(|| unservable(&gateway_request))?;
    let upstream_model = primary.upstream_model.clone();
    let provider = primary.provider.as_str().to_owned();

    let target = DispatchTarget::new(
        resolution.access_route_id(),
        gateway_request.model.as_str(),
        &primary.provider,
    );
    enforce_authz_pre_dispatch(&principal, target, &context_id, rc.ctx.authz_hook()).await?;

    Ok(PreparedRequest {
        origin: partial.origin,
        evidence,
        attribution,
        principal,
        body_bytes,
        client_headers,
        gateway_request,
        provider,
        upstream_model,
        session_id,
        context_id,
        gateway_conversation_id,
        client_session_id,
    })
}

async fn authenticate_request(
    rc: &RequestContext<'_>,
    presented: &str,
    header_session: Option<&SessionId>,
    headers: &HeaderMap,
    partial: &mut RejectionPartial,
) -> Result<AuthedPrincipal, RejectionError> {
    let principal = authenticate(
        presented,
        AuthInputs {
            header_session,
            headers,
            jwt_extractor: rc.jwt_extractor,
            providers: &rc.repos.subject_providers,
            ctx: rc.ctx,
        },
    )
    .await?;
    partial.user_id = Some(principal.user_id().clone());
    partial.trace_id = Some(principal.trace_id().clone());
    partial.session_id = Some(principal.session_id().clone());
    partial
        .client_session_id
        .clone_from(&principal.client_session);
    Ok(principal)
}

fn resolve_route(
    rc: &RequestContext<'_>,
    gateway_request: &CanonicalRequest,
    attribution: &RequestAttribution,
    partial: &mut RejectionPartial,
) -> Result<RouteResolution, RejectionError> {
    let resolution = systemprompt_gateway::resolve_route(
        RouteSource::Catalog(rc.catalog),
        gateway_request,
        attribution,
    )
    .map_err(|error| unservable(gateway_request).with_cause(error))?;
    partial.provider = resolution.provider().map(str::to_owned);
    Ok(resolution)
}

fn unservable(gateway_request: &CanonicalRequest) -> RejectionError {
    RejectionError::client(
        StatusCode::NOT_FOUND,
        format!("No gateway route matches model '{}'", gateway_request.model),
    )
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
