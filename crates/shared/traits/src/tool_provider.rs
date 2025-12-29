//! Tool provider traits for abstracting tool discovery and execution.
//!
//! This module defines the core traits for tool providers, allowing the AI
//! module to use tools without directly depending on specific implementations
//! like MCP.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Definition of a tool that can be called by an AI model.
///
/// This is a simplified view of tool metadata used by the AI module
/// for tool discovery and invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ToolDefinition {
    /// Unique name of the tool
    pub name: String,
    /// Human-readable description of what the tool does
    pub description: Option<String>,
    /// JSON schema defining the tool's input parameters
    pub input_schema: Option<JsonValue>,
    /// JSON schema defining the tool's output format
    pub output_schema: Option<JsonValue>,
    /// Identifier of the service/server that provides this tool
    pub service_id: String,
    /// Whether successful execution should terminate the agent loop
    #[serde(default)]
    pub terminal_on_success: bool,
    /// Optional model configuration for this specific tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_config: Option<JsonValue>,
}

impl ToolDefinition {
    /// Create a new tool definition with required fields.
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

    /// Add a description to the tool definition.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add an input schema to the tool definition.
    pub fn with_input_schema(mut self, schema: JsonValue) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Add an output schema to the tool definition.
    pub fn with_output_schema(mut self, schema: JsonValue) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Set whether the tool terminates on success.
    pub const fn with_terminal_on_success(mut self, terminal: bool) -> Self {
        self.terminal_on_success = terminal;
        self
    }

    /// Add model configuration for this tool.
    pub fn with_model_config(mut self, config: JsonValue) -> Self {
        self.model_config = Some(config);
        self
    }
}

/// A request to call a specific tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    /// The AI-assigned ID for this tool call
    pub tool_call_id: String,
    /// Name of the tool to call
    pub name: String,
    /// Arguments to pass to the tool
    pub arguments: JsonValue,
}

/// Result of a tool call execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Text content returned by the tool
    pub content: Vec<ToolContent>,
    /// Structured content for rich UI rendering
    pub structured_content: Option<JsonValue>,
    /// Whether the execution resulted in an error
    pub is_error: Option<bool>,
    /// Additional metadata
    pub meta: Option<JsonValue>,
}

/// Content item from a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Text content
    Text { text: String },
    /// Image content
    Image { data: String, mime_type: String },
    /// Resource reference
    Resource {
        uri: String,
        mime_type: Option<String>,
    },
}

impl ToolContent {
    /// Create a text content item.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}

impl ToolCallResult {
    /// Create a successful result with text content.
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        }
    }

    /// Create an error result.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(message)],
            structured_content: None,
            is_error: Some(true),
            meta: None,
        }
    }

    /// Add structured content to the result.
    pub fn with_structured_content(mut self, content: JsonValue) -> Self {
        self.structured_content = Some(content);
        self
    }
}

/// Error type for tool provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ToolProviderError {
    /// Tool not found
    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    /// Service not found
    #[error("Service '{0}' not found")]
    ServiceNotFound(String),

    /// Connection failed
    #[error("Failed to connect to service '{service}': {message}")]
    ConnectionFailed { service: String, message: String },

    /// Tool execution failed
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    /// Authorization error
    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for ToolProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Context for tool provider operations.
///
/// This is a simplified context type that tool providers can use
/// without depending on the full system context.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Authentication token for the request
    pub auth_token: String,
    /// Optional session ID
    pub session_id: Option<String>,
    /// Optional trace ID for distributed tracing
    pub trace_id: Option<String>,
    /// Optional AI tool call ID for linking executions
    pub ai_tool_call_id: Option<String>,
    /// Additional context headers
    pub headers: HashMap<String, String>,
}

impl ToolContext {
    /// Create a new tool context with an auth token.
    pub fn new(auth_token: impl Into<String>) -> Self {
        Self {
            auth_token: auth_token.into(),
            session_id: None,
            trace_id: None,
            ai_tool_call_id: None,
            headers: HashMap::new(),
        }
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set the trace ID.
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// Set the AI tool call ID.
    pub fn with_ai_tool_call_id(mut self, id: impl Into<String>) -> Self {
        self.ai_tool_call_id = Some(id.into());
        self
    }

    /// Add a header.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

/// Result type for tool provider operations.
pub type ToolProviderResult<T> = Result<T, ToolProviderError>;

/// Trait for discovering and executing tools.
///
/// This trait abstracts the tool discovery and execution mechanism,
/// allowing the AI module to work with tools without knowing the
/// underlying implementation (e.g., MCP, local functions, etc.).
#[async_trait]
pub trait ToolProvider: Send + Sync {
    /// List all available tools for a specific agent.
    ///
    /// # Arguments
    /// * `agent_name` - The name/identifier of the agent requesting tools
    /// * `context` - The tool context containing auth and session info
    ///
    /// # Returns
    /// A list of tool definitions available to the agent
    async fn list_tools(
        &self,
        agent_name: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<Vec<ToolDefinition>>;

    /// Execute a tool call.
    ///
    /// # Arguments
    /// * `request` - The tool call request
    /// * `service_id` - The ID of the service providing the tool
    /// * `context` - The tool context containing auth and session info
    ///
    /// # Returns
    /// The result of the tool execution
    async fn call_tool(
        &self,
        request: &ToolCallRequest,
        service_id: &str,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult>;

    /// Refresh connections for an agent's tools.
    ///
    /// This is called before tool discovery to ensure connections are ready.
    ///
    /// # Arguments
    /// * `agent_name` - The name/identifier of the agent
    async fn refresh_connections(&self, agent_name: &str) -> ToolProviderResult<()>;

    /// Check health of all connected tool services.
    ///
    /// # Returns
    /// A map of service names to their health status (true = healthy)
    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>>;

    /// Find a specific tool by name for an agent.
    ///
    /// Default implementation filters the `list_tools` result.
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
