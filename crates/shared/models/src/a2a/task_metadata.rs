use chrono::Utc;
use serde::{Deserialize, Serialize};
use systemprompt_traits::validation::{
    MetadataValidation, Validate, ValidationError, ValidationResult,
};

use crate::execution::ExecutionStep;

pub mod agent_names {
    pub const SYSTEM: &str = "system";
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    McpExecution,
    AgentMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskMetadata {
    pub task_type: TaskType,
    pub agent_name: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(rename = "executionSteps", skip_serializing_if = "Option::is_none")]
    pub execution_steps: Option<Vec<ExecutionStep>>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Map<String, serde_json::Value>>,
}

impl TaskMetadata {
    pub fn new_mcp_execution(
        agent_name: String,
        tool_name: String,
        mcp_server_name: String,
    ) -> Self {
        Self {
            task_type: TaskType::McpExecution,
            agent_name,
            tool_name: Some(tool_name),
            mcp_server_name: Some(mcp_server_name),
            created_at: Utc::now().to_rfc3339(),
            updated_at: None,
            started_at: None,
            completed_at: None,
            execution_time_ms: None,
            input_tokens: None,
            output_tokens: None,
            model: None,
            execution_steps: None,
            extensions: None,
        }
    }

    pub fn new_agent_message(agent_name: String) -> Self {
        Self {
            task_type: TaskType::AgentMessage,
            agent_name,
            tool_name: None,
            mcp_server_name: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: None,
            started_at: None,
            completed_at: None,
            execution_time_ms: None,
            input_tokens: None,
            output_tokens: None,
            model: None,
            execution_steps: None,
            extensions: None,
        }
    }

    pub const fn with_token_usage(mut self, input_tokens: u32, output_tokens: u32) -> Self {
        self.input_tokens = Some(input_tokens);
        self.output_tokens = Some(output_tokens);
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_updated_at(mut self) -> Self {
        self.updated_at = Some(Utc::now().to_rfc3339());
        self
    }

    pub fn with_tool_name(mut self, tool_name: impl Into<String>) -> Self {
        self.tool_name = Some(tool_name.into());
        self
    }

    pub fn with_execution_steps(mut self, steps: Vec<ExecutionStep>) -> Self {
        self.execution_steps = Some(steps);
        self
    }

    pub fn with_extension(mut self, key: String, value: serde_json::Value) -> Self {
        self.extensions
            .get_or_insert_with(serde_json::Map::new)
            .insert(key, value);
        self
    }

    pub fn new_validated_agent_message(agent_name: String) -> ValidationResult<Self> {
        if agent_name.is_empty() {
            return Err(ValidationError::new(
                "agent_name",
                "Cannot create TaskMetadata: agent_name is empty",
            )
            .with_context(format!("agent_name={agent_name:?}")));
        }

        let metadata = Self::new_agent_message(agent_name);
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn new_validated_mcp_execution(
        agent_name: String,
        tool_name: String,
        mcp_server_name: String,
    ) -> ValidationResult<Self> {
        if agent_name.is_empty() {
            return Err(ValidationError::new(
                "agent_name",
                "Cannot create TaskMetadata: agent_name is empty",
            )
            .with_context(format!("agent_name={agent_name:?}")));
        }

        if tool_name.is_empty() {
            return Err(ValidationError::new(
                "tool_name",
                "Cannot create TaskMetadata: tool_name is empty for MCP execution",
            )
            .with_context(format!("tool_name={tool_name:?}")));
        }

        let metadata = Self::new_mcp_execution(agent_name, tool_name, mcp_server_name);
        metadata.validate()?;
        Ok(metadata)
    }
}

impl Validate for TaskMetadata {
    fn validate(&self) -> ValidationResult<()> {
        self.validate_required_fields()?;
        Ok(())
    }
}

impl MetadataValidation for TaskMetadata {
    fn required_string_fields(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("agent_name", &self.agent_name),
            ("created_at", &self.created_at),
        ]
    }
}
