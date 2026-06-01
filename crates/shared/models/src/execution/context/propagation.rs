//! HTTP-header propagation of [`RequestContext`] across service hops.
//!
//! Implements [`InjectContextHeaders`] and [`ContextPropagation`] for
//! [`RequestContext`]: serializing identity, trace, and execution fields into
//! outbound headers and reconstructing them inbound. The proxy-verified path
//! reconstructs the [`AuthenticatedUser`](crate::auth::AuthenticatedUser) only
//! when an upstream proxy has asserted trust via the `proxy-verified` header.

use super::{CallSource, RequestContext};
use http::{HeaderMap, HeaderValue};
use std::str::FromStr;
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ClientId, ContextId, SessionId, TaskId, TraceId, UserId,
    headers,
};
use systemprompt_traits::{
    ContextPropagation, ContextPropagationError, ContextPropagationResult, InjectContextHeaders,
};

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    match HeaderValue::from_str(value) {
        Ok(val) => {
            headers.insert(name, val);
        },
        Err(e) => {
            tracing::warn!(
                header = %name,
                value = %value,
                error = %e,
                "Invalid header value - header not inserted"
            );
        },
    }
}

fn insert_header_if_present(headers: &mut HeaderMap, name: &'static str, value: Option<&str>) {
    if let Some(v) = value {
        insert_header(headers, name, v);
    }
}

impl InjectContextHeaders for RequestContext {
    fn inject_headers(&self, hdrs: &mut HeaderMap) {
        insert_header(hdrs, headers::SESSION_ID, self.request.session_id.as_str());
        insert_header(hdrs, headers::TRACE_ID, self.execution.trace_id.as_str());
        insert_header(hdrs, headers::USER_ID, self.auth.actor.user_id.as_str());
        insert_header(hdrs, headers::USER_TYPE, self.auth.user_type.as_str());
        insert_header(
            hdrs,
            headers::AGENT_NAME,
            self.execution.agent_name.as_str(),
        );

        insert_header(
            hdrs,
            headers::CONTEXT_ID,
            self.execution.context_id.as_str(),
        );

        insert_header_if_present(
            hdrs,
            headers::TASK_ID,
            self.execution.task_id.as_ref().map(TaskId::as_str),
        );
        insert_header_if_present(
            hdrs,
            headers::AI_TOOL_CALL_ID,
            self.execution.ai_tool_call_id.as_ref().map(AsRef::as_ref),
        );
        insert_header_if_present(
            hdrs,
            headers::CALL_SOURCE,
            self.execution.call_source.as_ref().map(CallSource::as_str),
        );
        insert_header_if_present(
            hdrs,
            headers::CLIENT_ID,
            self.request.client_id.as_ref().map(ClientId::as_str),
        );

        let auth_token = self.auth.auth_token.as_str();
        if auth_token.is_empty() {
            tracing::trace!(user_id = %self.auth.actor.user_id, "No auth_token to inject - Authorization header not added");
        } else {
            let auth_value = format!("Bearer {}", auth_token);
            insert_header(hdrs, headers::AUTHORIZATION, &auth_value);
            tracing::trace!(user_id = %self.auth.actor.user_id, "Injected Authorization header for proxy");
        }

        if let Some(user) = &self.user {
            insert_header(hdrs, headers::PROXY_VERIFIED, "true");
            let perms = crate::auth::permissions_to_string(&user.permissions);
            insert_header(hdrs, headers::USER_PERMISSIONS, &perms);
        }
    }
}

fn header_str<'h>(hdrs: &'h HeaderMap, name: &'static str) -> Option<&'h str> {
    hdrs.get(name).and_then(|v| v.to_str().ok())
}

fn required_header<'h>(
    hdrs: &'h HeaderMap,
    name: &'static str,
) -> ContextPropagationResult<&'h str> {
    header_str(hdrs, name).ok_or_else(|| ContextPropagationError::MissingHeader(name.to_owned()))
}

fn apply_optional_execution_fields(mut ctx: RequestContext, hdrs: &HeaderMap) -> RequestContext {
    if let Some(s) = header_str(hdrs, headers::TASK_ID) {
        ctx = ctx.with_task_id(TaskId::new(s.to_owned()));
    }
    if let Some(s) = header_str(hdrs, headers::AI_TOOL_CALL_ID) {
        ctx = ctx.with_ai_tool_call_id(AiToolCallId::new(s.to_owned()));
    }
    let call_source =
        header_str(hdrs, headers::CALL_SOURCE).and_then(|s| CallSource::from_str(s).ok());
    if let Some(cs) = call_source {
        ctx = ctx.with_call_source(cs);
    }
    if let Some(s) = header_str(hdrs, headers::CLIENT_ID) {
        ctx = ctx.with_client_id(ClientId::new(s.to_owned()));
    }
    let auth_token =
        header_str(hdrs, headers::AUTHORIZATION).and_then(|s| s.strip_prefix("Bearer "));
    if let Some(token) = auth_token {
        ctx = ctx.with_auth_token(token.to_owned());
    }
    ctx
}

fn apply_proxy_verified_user(
    mut ctx: RequestContext,
    hdrs: &HeaderMap,
    user_id: &str,
) -> ContextPropagationResult<RequestContext> {
    let proxy_verified = header_str(hdrs, headers::PROXY_VERIFIED).is_some_and(|v| v == "true");
    if !proxy_verified {
        return Ok(ctx);
    }

    let Some(permissions) = header_str(hdrs, headers::USER_PERMISSIONS)
        .and_then(|s| crate::auth::parse_permissions(s).ok())
    else {
        return Ok(ctx);
    };

    let user_id_uuid =
        user_id
            .parse::<uuid::Uuid>()
            .map_err(|e| ContextPropagationError::InvalidHeader {
                name: headers::USER_ID.to_owned(),
                message: format!("invalid UUID: {e}"),
            })?;
    let user = crate::auth::AuthenticatedUser::new(
        user_id_uuid,
        String::new(),
        String::new(),
        permissions,
    );
    ctx = ctx.with_user(user);
    Ok(ctx)
}

impl ContextPropagation for RequestContext {
    fn from_headers(hdrs: &HeaderMap) -> ContextPropagationResult<Self> {
        let session_id = required_header(hdrs, headers::SESSION_ID)?;
        let trace_id = required_header(hdrs, headers::TRACE_ID)?;
        let user_id = required_header(hdrs, headers::USER_ID)?;
        let agent_name = required_header(hdrs, headers::AGENT_NAME)?;

        let context_id = header_str(hdrs, headers::CONTEXT_ID)
            .filter(|s| !s.is_empty())
            .and_then(|s| ContextId::try_new(s).ok())
            .unwrap_or_else(ContextId::generate);

        let ctx = Self::new(
            SessionId::new(session_id.to_owned()),
            TraceId::new(trace_id.to_owned()),
            context_id,
            AgentName::new(agent_name.to_owned()),
        )
        .with_actor(Actor::user(UserId::new(user_id.to_owned())));

        let ctx = apply_optional_execution_fields(ctx, hdrs);
        apply_proxy_verified_user(ctx, hdrs, user_id)
    }

    fn to_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        self.inject_headers(&mut headers);
        headers
    }
}
