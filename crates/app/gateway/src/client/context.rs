//! The request context of one in-process gateway call.
//!
//! The HTTP handler builds a [`GatewayRequestContext`] from a bearer
//! principal and request headers; an in-process call builds it from the
//! [`ServicePrincipal`]: its user, session, `sys_<service>` client id and
//! access tier, an internal origin with internally attested evidence, a
//! `purpose` attribution, a fresh request and trace id, and a conversation
//! derived from the request's first message.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AiRequestId, ContextId, SessionId, TraceId};
use systemprompt_manifest::services::catalog::ResolvedHop;
use systemprompt_models::origin::{ClientAttestation, ClientEvidence, RequestOrigin};

use super::principal::ServicePrincipal;
use crate::audit::GatewayRequestContext;
use crate::protocol::canonical::CanonicalRequest;

#[derive(Debug)]
pub(super) struct ContextInputs<'a> {
    pub principal: &'a ServicePrincipal,
    pub session_id: SessionId,
    pub request: &'a CanonicalRequest,
    pub hop: &'a ResolvedHop,
    pub purpose: &'static str,
}

pub(super) fn request_context(inputs: ContextInputs<'_>) -> GatewayRequestContext {
    let ContextInputs {
        principal,
        session_id,
        request,
        hop,
        purpose,
    } = inputs;
    let user_id = principal.user_id().clone();
    let gateway_conversation_id = request.derived_gateway_conversation_id();
    let context_id = gateway_conversation_id
        .as_ref()
        .map_or_else(ContextId::generate, |gw| {
            ContextId::derived_from_gateway_conversation(&user_id, gw)
        });
    GatewayRequestContext {
        ai_request_id: AiRequestId::generate(),
        user_id,
        session_id: Some(session_id),
        context_id,
        gateway_conversation_id,
        client_session_id: None,
        trace_id: Some(TraceId::generate()),
        access_scope: principal.access_scope(),
        client_id: Some(principal.client_id().clone()),
        provider: hop.provider.as_str().to_owned(),
        hop: None,
        model: hop.upstream_model.clone(),
        requested_model: Some(request.model.to_string()),
        max_tokens: Some(request.max_tokens),
        is_streaming: false,
        origin: RequestOrigin::INTERNAL,
        evidence: ClientEvidence {
            kind_source: ClientAttestation::Internal,
            ..ClientEvidence::none()
        },
        attribution: principal.attribution(purpose),
        api_key_windows: Vec::new(),
        access_log: None,
        context_bound: false,
    }
}
