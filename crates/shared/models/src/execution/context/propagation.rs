use super::{CallSource, RequestContext};
use anyhow::anyhow;
use http::{HeaderMap, HeaderValue};
use std::str::FromStr;
use systemprompt_identifiers::{
    headers, AgentName, AiToolCallId, ClientId, ContextId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_traits::{ContextPropagation, InjectContextHeaders};

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
        insert_header(hdrs, headers::USER_ID, self.auth.user_id.as_str());
        insert_header(hdrs, headers::USER_TYPE, self.auth.user_type.as_str());
        insert_header(
            hdrs,
            headers::AGENT_NAME,
            self.execution.agent_name.as_str(),
        );

        let context_id = self.execution.context_id.as_str();
        if !context_id.is_empty() {
            insert_header(hdrs, headers::CONTEXT_ID, context_id);
        }

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
            tracing::trace!(user_id = %self.auth.user_id, "No auth_token to inject - Authorization header not added");
        } else {
            let auth_value = format!("Bearer {}", auth_token);
            insert_header(hdrs, headers::AUTHORIZATION, &auth_value);
            tracing::trace!(user_id = %self.auth.user_id, "Injected Authorization header for proxy");
        }
    }
}

impl ContextPropagation for RequestContext {
    fn from_headers(hdrs: &HeaderMap) -> anyhow::Result<Self> {
        let session_id = hdrs
            .get(headers::SESSION_ID)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("Missing {} header", headers::SESSION_ID))?;

        let trace_id = hdrs
            .get(headers::TRACE_ID)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("Missing {} header", headers::TRACE_ID))?;

        let user_id = hdrs
            .get(headers::USER_ID)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("Missing {} header", headers::USER_ID))?;

        let context_id = hdrs
            .get(headers::CONTEXT_ID)
            .and_then(|v| v.to_str().ok())
            .map_or_else(
                || ContextId::new(String::new()),
                |s| ContextId::new(s.to_string()),
            );

        let agent_name = hdrs
            .get(headers::AGENT_NAME)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                anyhow!(
                    "Missing {} header - all requests must have agent context",
                    headers::AGENT_NAME
                )
            })?;

        let task_id = hdrs
            .get(headers::TASK_ID)
            .and_then(|v| v.to_str().ok())
            .map(|s| TaskId::new(s.to_string()));

        let ai_tool_call_id = hdrs
            .get(headers::AI_TOOL_CALL_ID)
            .and_then(|v| v.to_str().ok())
            .map(|s| AiToolCallId::from(s.to_string()));

        let call_source = hdrs
            .get(headers::CALL_SOURCE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| CallSource::from_str(s).ok());

        let client_id = hdrs
            .get(headers::CLIENT_ID)
            .and_then(|v| v.to_str().ok())
            .map(|s| ClientId::new(s.to_string()));

        let mut ctx = Self::new(
            SessionId::new(session_id.to_string()),
            TraceId::new(trace_id.to_string()),
            context_id,
            AgentName::new(agent_name.to_string()),
        )
        .with_user_id(UserId::new(user_id.to_string()));

        if let Some(tid) = task_id {
            ctx = ctx.with_task_id(tid);
        }

        if let Some(ai_id) = ai_tool_call_id {
            ctx = ctx.with_ai_tool_call_id(ai_id);
        }

        if let Some(cs) = call_source {
            ctx = ctx.with_call_source(cs);
        }

        if let Some(cid) = client_id {
            ctx = ctx.with_client_id(cid);
        }

        Ok(ctx)
    }

    fn to_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        self.inject_headers(&mut headers);
        headers
    }
}
