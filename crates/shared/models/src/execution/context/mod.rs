//! Request context for execution tracking.

mod call_source;
mod context_error;
mod context_types;

pub use call_source::CallSource;
pub use context_error::{ContextExtractionError, ContextIdSource, TASK_BASED_CONTEXT_MARKER};
pub use context_types::{
    AuthContext, ExecutionContext, ExecutionSettings, RequestMetadata, UserInteractionMode,
};

use crate::ai::ToolModelConfig;
use crate::auth::{AuthenticatedUser, RateLimitTier, UserType};
use anyhow::anyhow;
use axum::http::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::time::{Duration, Instant};
use systemprompt_identifiers::{
    AgentName, AiToolCallId, ClientId, ContextId, JwtToken, McpExecutionId, SessionId, TaskId,
    TraceId, UserId,
};
use systemprompt_traits::{ContextPropagation, InjectContextHeaders};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    pub auth: AuthContext,
    pub request: RequestMetadata,
    pub execution: ExecutionContext,
    pub settings: ExecutionSettings,

    #[serde(skip)]
    pub user: Option<AuthenticatedUser>,

    #[serde(skip, default = "Instant::now")]
    pub start_time: Instant,
}

impl RequestContext {
    /// Creates a new `RequestContext` - the ONLY way to construct a context.
    ///
    /// This is the single constructor for `RequestContext`. All contexts must
    /// be created through this method, ensuring consistent initialization.
    ///
    /// # Required Fields
    /// - `session_id`: Identifies the user session
    /// - `trace_id`: For distributed tracing
    /// - `context_id`: Conversation/execution context (empty string for
    ///   user-level contexts)
    /// - `agent_name`: The agent handling this request (use
    ///   `AgentName::system()` for system operations)
    ///
    /// # Optional Fields
    /// Use builder methods to set optional fields:
    /// - `.with_user_id()` - Set the authenticated user
    /// - `.with_auth_token()` - Set the JWT token
    /// - `.with_user_type()` - Set user type (Admin, Standard, Anon)
    /// - `.with_task_id()` - Set task ID for AI operations
    /// - `.with_client_id()` - Set client ID
    /// - `.with_call_source()` - Set call source (Agentic, Direct, Ephemeral)
    ///
    /// # Example
    /// ```
    /// # use systemprompt_models::execution::context::RequestContext;
    /// # use systemprompt_identifiers::{SessionId, TraceId, ContextId, AgentName, UserId};
    /// # use systemprompt_models::auth::UserType;
    /// let ctx = RequestContext::new(
    ///     SessionId::new("sess_123".to_string()),
    ///     TraceId::new("trace_456".to_string()),
    ///     ContextId::new("ctx_789".to_string()),
    ///     AgentName::new("my-agent".to_string()),
    /// )
    /// .with_user_id(UserId::new("user_123".to_string()))
    /// .with_auth_token("jwt_token_here")
    /// .with_user_type(UserType::User);
    /// ```
    pub fn new(
        session_id: SessionId,
        trace_id: TraceId,
        context_id: ContextId,
        agent_name: AgentName,
    ) -> Self {
        Self {
            auth: AuthContext {
                auth_token: JwtToken::new(""),
                user_id: UserId::anonymous(),
                user_type: UserType::Anon,
            },
            request: RequestMetadata {
                session_id,
                timestamp: Instant::now(),
                client_id: None,
                is_tracked: true,
            },
            execution: ExecutionContext {
                trace_id,
                context_id,
                task_id: None,
                ai_tool_call_id: None,
                mcp_execution_id: None,
                call_source: None,
                agent_name,
                tool_model_config: None,
            },
            settings: ExecutionSettings::default(),
            user: None,
            start_time: Instant::now(),
        }
    }

    pub fn with_user(mut self, user: AuthenticatedUser) -> Self {
        self.auth.user_id = UserId::new(user.id.to_string());
        self.user = Some(user);
        self
    }

    pub fn with_user_id(mut self, user_id: UserId) -> Self {
        self.auth.user_id = user_id;
        self
    }

    pub fn with_agent_name(mut self, agent_name: AgentName) -> Self {
        self.execution.agent_name = agent_name;
        self
    }

    pub fn with_context_id(mut self, context_id: ContextId) -> Self {
        self.execution.context_id = context_id;
        self
    }

    pub fn with_task_id(mut self, task_id: TaskId) -> Self {
        self.execution.task_id = Some(task_id);
        self
    }

    pub fn with_task(mut self, task_id: TaskId, call_source: CallSource) -> Self {
        self.execution.task_id = Some(task_id);
        self.execution.call_source = Some(call_source);
        self
    }

    pub fn with_ai_tool_call_id(mut self, ai_tool_call_id: AiToolCallId) -> Self {
        self.execution.ai_tool_call_id = Some(ai_tool_call_id);
        self
    }

    pub fn with_mcp_execution_id(mut self, mcp_execution_id: McpExecutionId) -> Self {
        self.execution.mcp_execution_id = Some(mcp_execution_id);
        self
    }

    pub fn with_client_id(mut self, client_id: ClientId) -> Self {
        self.request.client_id = Some(client_id);
        self
    }

    pub const fn with_user_type(mut self, user_type: UserType) -> Self {
        self.auth.user_type = user_type;
        self
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth.auth_token = JwtToken::new(token.into());
        self
    }

    pub const fn with_call_source(mut self, call_source: CallSource) -> Self {
        self.execution.call_source = Some(call_source);
        self
    }

    pub const fn with_budget(mut self, cents: i32) -> Self {
        self.settings.max_budget_cents = Some(cents);
        self
    }

    pub const fn with_interaction_mode(mut self, mode: UserInteractionMode) -> Self {
        self.settings.user_interaction_mode = Some(mode);
        self
    }

    pub const fn with_tracked(mut self, is_tracked: bool) -> Self {
        self.request.is_tracked = is_tracked;
        self
    }

    pub fn with_tool_model_config(mut self, config: ToolModelConfig) -> Self {
        self.execution.tool_model_config = Some(config);
        self
    }

    pub const fn tool_model_config(&self) -> Option<&ToolModelConfig> {
        self.execution.tool_model_config.as_ref()
    }

    pub const fn session_id(&self) -> &SessionId {
        &self.request.session_id
    }

    pub const fn user_id(&self) -> &UserId {
        &self.auth.user_id
    }

    pub const fn trace_id(&self) -> &TraceId {
        &self.execution.trace_id
    }

    pub const fn context_id(&self) -> &ContextId {
        &self.execution.context_id
    }

    pub const fn agent_name(&self) -> &AgentName {
        &self.execution.agent_name
    }

    pub const fn auth_token(&self) -> &JwtToken {
        &self.auth.auth_token
    }

    pub const fn user_type(&self) -> UserType {
        self.auth.user_type
    }

    pub const fn rate_limit_tier(&self) -> RateLimitTier {
        self.auth.user_type.rate_tier()
    }

    pub const fn task_id(&self) -> Option<&TaskId> {
        self.execution.task_id.as_ref()
    }

    pub const fn client_id(&self) -> Option<&ClientId> {
        self.request.client_id.as_ref()
    }

    pub const fn ai_tool_call_id(&self) -> Option<&AiToolCallId> {
        self.execution.ai_tool_call_id.as_ref()
    }

    pub const fn mcp_execution_id(&self) -> Option<&McpExecutionId> {
        self.execution.mcp_execution_id.as_ref()
    }

    pub const fn call_source(&self) -> Option<CallSource> {
        self.execution.call_source
    }

    pub const fn is_authenticated(&self) -> bool {
        self.user.is_some()
    }

    pub fn is_system(&self) -> bool {
        self.auth.user_id.is_system() && self.execution.context_id.is_system()
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn validate_task_execution(&self) -> Result<(), String> {
        if self.execution.task_id.is_none() {
            return Err("Missing task_id for task execution".to_string());
        }
        if self.execution.context_id.as_str().is_empty() {
            return Err("Missing context_id for task execution".to_string());
        }
        Ok(())
    }

    pub fn validate_authenticated(&self) -> Result<(), String> {
        if self.auth.auth_token.as_str().is_empty() {
            return Err("Missing authentication token".to_string());
        }
        if self.auth.user_id.is_anonymous() {
            return Err("User is not authenticated".to_string());
        }
        Ok(())
    }
}

fn insert_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    if let Ok(val) = HeaderValue::from_str(value) {
        headers.insert(name, val);
    }
}

fn insert_header_if_present(headers: &mut HeaderMap, name: &'static str, value: Option<&str>) {
    if let Some(v) = value {
        insert_header(headers, name, v);
    }
}

impl InjectContextHeaders for RequestContext {
    fn inject_headers(&self, headers: &mut HeaderMap) {
        insert_header(headers, "x-session-id", self.request.session_id.as_str());
        insert_header(headers, "x-trace-id", self.execution.trace_id.as_str());
        insert_header(headers, "x-user-id", self.auth.user_id.as_str());
        insert_header(headers, "x-user-type", self.auth.user_type.as_str());
        insert_header(headers, "x-agent-name", self.execution.agent_name.as_str());

        let context_id = self.execution.context_id.as_str();
        if !context_id.is_empty() {
            insert_header(headers, "x-context-id", context_id);
        }

        insert_header_if_present(
            headers,
            "x-task-id",
            self.execution.task_id.as_ref().map(TaskId::as_str),
        );
        insert_header_if_present(
            headers,
            "x-ai-tool-call-id",
            self.execution.ai_tool_call_id.as_ref().map(AsRef::as_ref),
        );
        insert_header_if_present(
            headers,
            "x-call-source",
            self.execution.call_source.as_ref().map(CallSource::as_str),
        );
        insert_header_if_present(
            headers,
            "x-client-id",
            self.request.client_id.as_ref().map(ClientId::as_str),
        );

        let auth_token = self.auth.auth_token.as_str();
        if !auth_token.is_empty() {
            let auth_value = format!("Bearer {}", auth_token);
            insert_header(headers, "authorization", &auth_value);
        }
    }
}

impl ContextPropagation for RequestContext {
    fn from_headers(headers: &HeaderMap) -> anyhow::Result<Self> {
        let session_id = headers
            .get("x-session-id")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("Missing x-session-id header"))?;

        let trace_id = headers
            .get("x-trace-id")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("Missing x-trace-id header"))?;

        let user_id = headers
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("Missing x-user-id header"))?;

        let context_id = headers
            .get("x-context-id")
            .and_then(|v| v.to_str().ok())
            .map_or_else(
                || ContextId::new(String::new()),
                |s| ContextId::new(s.to_string()),
            );

        let agent_name = headers
            .get("x-agent-name")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                anyhow!("Missing x-agent-name header - all requests must have agent context")
            })?;

        let task_id = headers
            .get("x-task-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| TaskId::new(s.to_string()));

        let ai_tool_call_id = headers
            .get("x-ai-tool-call-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| AiToolCallId::from(s.to_string()));

        let call_source = headers
            .get("x-call-source")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| CallSource::from_str(s).ok());

        let client_id = headers
            .get("x-client-id")
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
