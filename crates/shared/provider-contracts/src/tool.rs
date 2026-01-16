use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<JsonValue>,
    pub output_schema: Option<JsonValue>,
    pub service_id: String,
    #[serde(default)]
    pub terminal_on_success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<JsonValue>,
}

impl ToolDefinition {
    pub fn new(name: impl Into<String>, service_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: None,
            output_schema: None,
            service_id: service_id.into(),
            terminal_on_success: false,
            model_config: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_input_schema(mut self, schema: JsonValue) -> Self {
        self.input_schema = Some(schema);
        self
    }

    pub fn with_output_schema(mut self, schema: JsonValue) -> Self {
        self.output_schema = Some(schema);
        self
    }

    pub const fn with_terminal_on_success(mut self, terminal: bool) -> Self {
        self.terminal_on_success = terminal;
        self
    }

    pub fn with_model_config(mut self, config: JsonValue) -> Self {
        self.model_config = Some(config);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub tool_call_id: String,
    pub name: String,
    pub arguments: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    pub structured_content: Option<JsonValue>,
    pub is_error: Option<bool>,
    pub meta: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    Text {
        text: String,
    },
    Image {
        data: String,
        mime_type: String,
    },
    Resource {
        uri: String,
        mime_type: Option<String>,
    },
}

impl ToolContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}

impl ToolCallResult {
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(message)],
            structured_content: None,
            is_error: Some(true),
            meta: None,
        }
    }

    pub fn with_structured_content(mut self, content: JsonValue) -> Self {
        self.structured_content = Some(content);
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ToolProviderError {
    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    #[error("Service '{0}' not found")]
    ServiceNotFound(String),

    #[error("Failed to connect to service '{service}': {message}")]
    ConnectionFailed { service: String, message: String },

    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for ToolProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub auth_token: String,
    pub session_id: Option<String>,
    pub trace_id: Option<String>,
    pub ai_tool_call_id: Option<String>,
    pub headers: HashMap<String, String>,
}

impl ToolContext {
    pub fn new(auth_token: impl Into<String>) -> Self {
        Self {
            auth_token: auth_token.into(),
            session_id: None,
            trace_id: None,
            ai_tool_call_id: None,
            headers: HashMap::new(),
        }
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_ai_tool_call_id(mut self, id: impl Into<String>) -> Self {
        self.ai_tool_call_id = Some(id.into());
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

pub type ToolProviderResult<T> = Result<T, ToolProviderError>;

#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn list_tools(
        &self,
        agent_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>>;

    async fn call_tool(
        &self,
        request: &ToolCallRequest,
        service_id: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult>;

    async fn refresh_connections(&self, agent_name: &str) -> ToolProviderResult<()>;

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>>;

    async fn find_tool(
        &self,
        agent_name: &str,
        tool_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Option<ToolDefinition>> {
        let tools = self.list_tools(agent_name, context).await?;
        Ok(tools.into_iter().find(|t| t.name == tool_name))
    }
}
