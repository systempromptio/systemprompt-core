//! HTTP-header propagation of [`RequestContext`] across service hops.
//!
//! Implements [`InjectContextHeaders`] and [`ContextPropagation`] for
//! [`RequestContext`]: serializing trace and execution fields (and the bearer
//! the next hop may relay) into outbound headers and reconstructing them
//! inbound. Headers carry correlation, never identity: a context rebuilt from
//! headers is always anonymous, and a receiver that needs the caller's
//! identity takes it from a credential it verifies itself.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{CallSource, RequestContext};
use http::{HeaderMap, HeaderValue};
use std::str::FromStr;
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ContextId, JwtToken, SessionId, TaskId, TraceId, UserId,
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

        match &self.auth.auth_token {
            Some(auth_token) => {
                let auth_value = format!("Bearer {}", auth_token.as_str());
                insert_header(hdrs, headers::AUTHORIZATION, &auth_value);
                tracing::trace!("Injected Authorization header for the next hop");
            },
            None => {
                tracing::trace!("No auth_token to inject - Authorization header not added");
            },
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

fn invalid_header(
    name: &'static str,
    source: impl Into<systemprompt_traits::BoxedSource>,
) -> ContextPropagationError {
    ContextPropagationError::InvalidHeader {
        name: name.to_owned(),
        source: source.into(),
    }
}

fn apply_optional_execution_fields(
    mut ctx: RequestContext,
    hdrs: &HeaderMap,
) -> ContextPropagationResult<RequestContext> {
    if let Some(s) = header_str(hdrs, headers::TASK_ID) {
        ctx = ctx.with_task_id(TaskId::new(s.to_owned()));
    }
    if let Some(s) = header_str(hdrs, headers::AI_TOOL_CALL_ID) {
        ctx = ctx.with_ai_tool_call_id(AiToolCallId::new(s.to_owned()));
    }
    if let Some(s) = header_str(hdrs, headers::CALL_SOURCE) {
        let cs = CallSource::from_str(s).map_err(|e| invalid_header(headers::CALL_SOURCE, e))?;
        ctx = ctx.with_call_source(cs);
    }
    let auth_token = header_str(hdrs, headers::AUTHORIZATION)
        .and_then(|s| s.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty());
    if let Some(token) = auth_token {
        ctx = ctx.with_auth_token(JwtToken::new(token));
    }
    Ok(ctx)
}

impl ContextPropagation for RequestContext {
    fn from_headers(hdrs: &HeaderMap) -> ContextPropagationResult<Self> {
        let session_id = required_header(hdrs, headers::SESSION_ID)?;
        let trace_id = required_header(hdrs, headers::TRACE_ID)?;
        let agent_name = required_header(hdrs, headers::AGENT_NAME)?;

        let session_id = SessionId::new(session_id.to_owned());
        let context_id = match header_str(hdrs, headers::CONTEXT_ID).filter(|s| !s.is_empty()) {
            Some(s) => ContextId::try_new(s).map_err(|e| invalid_header(headers::CONTEXT_ID, e))?,
            None => ContextId::derived_from_session(&session_id),
        };

        let agent_name = AgentName::try_new(agent_name.to_owned())
            .map_err(|e| invalid_header(headers::AGENT_NAME, e))?;

        let ctx = Self::new(
            session_id,
            TraceId::new(trace_id.to_owned()),
            context_id,
            agent_name,
            Actor::anonymous(UserId::generate()),
        );

        apply_optional_execution_fields(ctx, hdrs)
    }

    fn to_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        self.inject_headers(&mut headers);
        headers
    }
}
