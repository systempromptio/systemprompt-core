//! MCP (Model Context Protocol) identifier types.

use crate::error::IdValidationError;
use crate::{DbValue, ToDbValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// AI Provider's tool call identifier (from Anthropic/OpenAI API response)
/// Example: `toolu_01D7XQ2V9K3J8N5M4P6R7T8W9Y`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct AiToolCallId(String);

impl AiToolCallId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AiToolCallId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for AiToolCallId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for AiToolCallId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for AiToolCallId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for AiToolCallId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &AiToolCallId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

/// MCP execution identifier (internal tracking in `mcp_tool_executions` table)
/// Example: `550e8400-e29b-41d4-a716-446655440000`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct McpExecutionId(String);

impl McpExecutionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for McpExecutionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for McpExecutionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for McpExecutionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for McpExecutionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for McpExecutionId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &McpExecutionId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

/// MCP Server identifier - the canonical name of an MCP server.
///
/// This identifies which MCP server provides a tool, used for routing tool
/// calls to the correct server endpoint. The value MUST match the key in
/// `mcp_servers` in the YAML configuration.
///
/// # Format
/// - Lowercase alphanumeric with hyphens
/// - Examples: "content-manager", "systemprompt-admin", "tyingshoelaces"
///
/// # Flow
/// 1. YAML config defines `mcp_servers.{name}` - this is the canonical ID
/// 2. Spawner passes `MCP_SERVICE_ID={name}` to the server process
/// 3. Server reads env and validates it matches expected value
/// 4. Tools use this ID so the system knows where to route calls
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct McpServerId(String);

impl McpServerId {
    /// Create a new validated MCP server ID.
    ///
    /// # Errors
    /// Returns `IdValidationError::Empty` if the ID is empty.
    pub fn try_new(id: impl Into<String>) -> Result<Self, IdValidationError> {
        let id = id.into();
        if id.is_empty() {
            return Err(IdValidationError::empty("McpServerId"));
        }
        Ok(Self(id))
    }

    /// Create a new MCP server ID, panicking if validation fails.
    ///
    /// # Panics
    /// Panics if the ID is empty.
    #[allow(clippy::expect_used)]
    pub fn new(id: impl Into<String>) -> Self {
        Self::try_new(id).expect("MCP server ID cannot be empty")
    }

    /// Load MCP server ID from the `MCP_SERVICE_ID` environment variable.
    ///
    /// # Errors
    /// Returns an error if the environment variable is not set or is empty.
    pub fn from_env() -> Result<Self, IdValidationError> {
        let id = std::env::var("MCP_SERVICE_ID").map_err(|_| {
            IdValidationError::invalid(
                "McpServerId",
                "MCP_SERVICE_ID environment variable not set",
            )
        })?;
        Self::try_new(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for McpServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for McpServerId {
    type Error = IdValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl TryFrom<&str> for McpServerId {
    type Error = IdValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl std::str::FromStr for McpServerId {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl AsRef<str> for McpServerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for McpServerId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &McpServerId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
