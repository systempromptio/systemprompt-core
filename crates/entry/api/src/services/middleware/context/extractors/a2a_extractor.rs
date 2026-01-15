use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::execution::{ContextExtractionError, RequestContext};

use super::traits::ContextExtractor;
use crate::services::middleware::context::sources::{
    ContextIdSource, HeaderSource, PayloadSource, TASK_BASED_CONTEXT_MARKER,
};

#[derive(Debug, Clone, Copy)]
pub struct A2aContextExtractor;

impl A2aContextExtractor {
    pub const fn new() -> Self {
        Self
    }

    fn try_from_headers(headers: &HeaderMap) -> Result<RequestContext, ContextExtractionError> {
        let session_id = HeaderSource::extract_required(headers, "x-session-id")?;
        let trace_id = HeaderSource::extract_required(headers, "x-trace-id")?;
        let user_id = HeaderSource::extract_required(headers, "x-user-id")?;
        let context_id = HeaderSource::extract_required(headers, "x-context-id")?;
        let agent_name = HeaderSource::extract_required(headers, "x-agent-name")?;

        let mut context = RequestContext::new(
            SessionId::new(session_id),
            TraceId::new(trace_id),
            ContextId::new(context_id),
            AgentName::new(agent_name),
        )
        .with_user_id(UserId::new(user_id));

        if let Some(task_id_str) = HeaderSource::extract_optional(headers, "x-task-id") {
            context = context.with_task_id(TaskId::new(task_id_str));
        }

        Ok(context)
    }

    fn try_from_payload(
        body_bytes: &[u8],
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        let context_source = PayloadSource::extract_context_source(body_bytes)?;

        let session_id = HeaderSource::extract_required(headers, "x-session-id")?;
        let trace_id = HeaderSource::extract_required(headers, "x-trace-id")?;
        let user_id = HeaderSource::extract_required(headers, "x-user-id")?;
        let agent_name = HeaderSource::extract_required(headers, "x-agent-name")?;

        let (context_id, task_id) = match context_source {
            ContextIdSource::Direct(id) => {
                (id, HeaderSource::extract_optional(headers, "x-task-id"))
            },
            ContextIdSource::FromTask { task_id } => {
                (TASK_BASED_CONTEXT_MARKER.to_string(), Some(task_id))
            },
        };

        let mut context = RequestContext::new(
            SessionId::new(session_id),
            TraceId::new(trace_id),
            ContextId::new(context_id),
            AgentName::new(agent_name),
        )
        .with_user_id(UserId::new(user_id));

        if let Some(task_id_str) = task_id {
            context = context.with_task_id(TaskId::new(task_id_str));
        }

        Ok(context)
    }
}

impl Default for A2aContextExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContextExtractor for A2aContextExtractor {
    async fn extract_from_headers(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        Self::try_from_headers(headers)
    }

    async fn extract_from_request(
        &self,
        request: Request<Body>,
    ) -> Result<(RequestContext, Request<Body>), ContextExtractionError> {
        let headers = request.headers().clone();

        if let Ok(context) = Self::try_from_headers(&headers) {
            return Ok((context, request));
        }

        let (body_bytes, reconstructed_request) =
            PayloadSource::read_and_reconstruct(request).await?;

        let context = Self::try_from_payload(&body_bytes, &headers)?;

        Ok((context, reconstructed_request))
    }
}
