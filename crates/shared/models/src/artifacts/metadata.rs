#![allow(clippy::trait_duplication_in_bounds)]

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{
    AgentName, ArtifactId, ContextId, McpExecutionId, SessionId, SkillId, TaskId, TraceId, UserId,
};

use crate::execution::context::RequestContext;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecutionMetadata {
    #[schemars(with = "String")]
    pub context_id: ContextId,

    #[schemars(with = "String")]
    pub trace_id: TraceId,

    #[schemars(with = "String")]
    pub session_id: SessionId,

    #[schemars(with = "String")]
    pub user_id: UserId,

    #[schemars(with = "String")]
    pub agent_name: AgentName,

    #[schemars(with = "String")]
    pub timestamp: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub task_id: Option<TaskId>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub skill_id: Option<SkillId>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
}

impl Default for ExecutionMetadata {
    fn default() -> Self {
        Self {
            context_id: ContextId::new("default"),
            trace_id: TraceId::new("default"),
            session_id: SessionId::new("default"),
            user_id: UserId::new("default"),
            agent_name: AgentName::new("default"),
            timestamp: Utc::now(),
            task_id: None,
            tool_name: None,
            skill_id: None,
            skill_name: None,
            execution_id: None,
        }
    }
}

#[derive(Debug)]
pub struct ExecutionMetadataBuilder {
    context_id: ContextId,
    trace_id: TraceId,
    session_id: SessionId,
    user_id: UserId,
    agent_name: AgentName,
    timestamp: DateTime<Utc>,
    task_id: Option<TaskId>,
    tool_name: Option<String>,
    skill_id: Option<SkillId>,
    skill_name: Option<String>,
    execution_id: Option<String>,
}

impl ExecutionMetadataBuilder {
    pub fn new(ctx: &RequestContext) -> Self {
        Self {
            context_id: ctx.context_id().clone(),
            trace_id: ctx.trace_id().clone(),
            session_id: ctx.session_id().clone(),
            user_id: ctx.user_id().clone(),
            agent_name: ctx.agent_name().clone(),
            timestamp: Utc::now(),
            task_id: ctx.task_id().cloned(),
            tool_name: None,
            skill_id: None,
            skill_name: None,
            execution_id: None,
        }
    }

    pub fn with_tool(mut self, name: impl Into<String>) -> Self {
        self.tool_name = Some(name.into());
        self
    }

    pub fn with_skill(mut self, id: impl Into<SkillId>, name: impl Into<String>) -> Self {
        self.skill_id = Some(id.into());
        self.skill_name = Some(name.into());
        self
    }

    pub fn with_execution(mut self, id: impl Into<String>) -> Self {
        self.execution_id = Some(id.into());
        self
    }

    pub fn build(self) -> ExecutionMetadata {
        ExecutionMetadata {
            context_id: self.context_id,
            trace_id: self.trace_id,
            session_id: self.session_id,
            user_id: self.user_id,
            agent_name: self.agent_name,
            timestamp: self.timestamp,
            task_id: self.task_id,
            tool_name: self.tool_name,
            skill_id: self.skill_id,
            skill_name: self.skill_name,
            execution_id: self.execution_id,
        }
    }
}

impl ExecutionMetadata {
    pub fn builder(ctx: &RequestContext) -> ExecutionMetadataBuilder {
        ExecutionMetadataBuilder::new(ctx)
    }

    pub fn with_request(ctx: &RequestContext) -> Self {
        Self::builder(ctx).build()
    }

    pub fn with_tool(mut self, name: impl Into<String>) -> Self {
        self.tool_name = Some(name.into());
        self
    }

    pub fn with_skill(mut self, id: impl Into<String>, name: impl Into<String>) -> Self {
        self.skill_id = Some(SkillId::new(id));
        self.skill_name = Some(name.into());
        self
    }

    pub fn with_execution(mut self, id: impl Into<String>) -> Self {
        self.execution_id = Some(id.into());
        self
    }

    pub fn schema() -> JsonValue {
        match serde_json::to_value(schemars::schema_for!(Self)) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "ExecutionMetadata schema serialization failed");
                JsonValue::Null
            },
        }
    }

    pub fn to_meta(&self) -> Option<rmcp::model::Meta> {
        serde_json::to_value(self)
            .map_err(|e| {
                tracing::warn!(error = %e, "ExecutionMetadata serialization failed");
                e
            })
            .ok()
            .and_then(|v| v.as_object().cloned())
            .map(rmcp::model::Meta)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolResponse<T: JsonSchema> {
    pub artifact_id: ArtifactId,
    pub mcp_execution_id: McpExecutionId,
    pub artifact: T,
    #[serde(rename = "_metadata")]
    pub metadata: ExecutionMetadata,
}

impl<T: Serialize + JsonSchema> ToolResponse<T> {
    pub const fn new(
        artifact_id: ArtifactId,
        mcp_execution_id: McpExecutionId,
        artifact: T,
        metadata: ExecutionMetadata,
    ) -> Self {
        Self {
            artifact_id,
            mcp_execution_id,
            artifact,
            metadata,
        }
    }

    pub fn to_json(&self) -> JsonValue {
        match serde_json::to_value(self) {
            Ok(value) => value,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    artifact_id = %self.artifact_id,
                    "ToolResponse serialization failed"
                );
                JsonValue::Null
            },
        }
    }

    pub fn try_to_json(&self) -> Result<JsonValue, serde_json::Error> {
        serde_json::to_value(self)
    }
}

impl<T: JsonSchema> ToolResponse<T> {
    pub fn schema() -> JsonValue {
        match serde_json::to_value(schemars::schema_for!(Self)) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "ToolResponse schema serialization failed");
                JsonValue::Null
            },
        }
    }
}
