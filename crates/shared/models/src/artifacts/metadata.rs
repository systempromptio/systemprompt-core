//! Execution provenance carried on every artifact.
//!
//! [`ExecutionMetadata`] captures the full identity of the run that produced an
//! artifact — context, trace, session, user, agent, and the optional tool/skill
//! that emitted it — and is derived from a [`RequestContext`] via
//! [`ExecutionMetadataBuilder`]. [`ToolResponse`] wraps an artifact with this
//! metadata and its persisted ids; it is the storage envelope for
//! `mcp_artifacts.data` rows and never appears on the wire, where provenance
//! travels under the [`EXECUTION_META_KEY`] `_meta` key instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{
    AgentName, ArtifactId, ContextId, McpExecutionId, McpToolName, SessionId, SkillId, SkillName,
    TaskId, TraceId, UserId,
};

use crate::execution::context::RequestContext;

pub const EXECUTION_META_KEY: &str = "io.systemprompt/execution";

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
    #[schemars(with = "Option<String>")]
    pub tool_name: Option<McpToolName>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub skill_id: Option<SkillId>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub skill_name: Option<SkillName>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
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
    tool_name: Option<McpToolName>,
    skill_id: Option<SkillId>,
    skill_name: Option<SkillName>,
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

    pub fn with_tool(mut self, name: McpToolName) -> Self {
        self.tool_name = Some(name);
        self
    }

    pub fn with_skill(mut self, id: SkillId, name: SkillName) -> Self {
        self.skill_id = Some(id);
        self.skill_name = Some(name);
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

    pub fn with_tool(mut self, name: McpToolName) -> Self {
        self.tool_name = Some(name);
        self
    }

    pub fn with_skill(mut self, id: SkillId, name: SkillName) -> Self {
        self.skill_id = Some(id);
        self.skill_name = Some(name);
        self
    }

    pub fn with_execution(mut self, id: impl Into<String>) -> Self {
        self.execution_id = Some(id.into());
        self
    }

    // JSON: JSON Schema document describing the metadata for the model.
    pub fn schema() -> JsonValue {
        match serde_json::to_value(schemars::schema_for!(Self)) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "ExecutionMetadata schema serialization failed");
                JsonValue::Null
            },
        }
    }

    // JSON: A2A `Artifact.metadata` map.
    pub fn to_object(&self) -> Option<serde_json::Map<String, JsonValue>> {
        serde_json::to_value(self)
            .map_err(|e| {
                tracing::warn!(error = %e, "ExecutionMetadata serialization failed");
                e
            })
            .ok()
            .and_then(|v| v.as_object().cloned())
    }
}

/// Provenance an artifact accumulates while it is built: the request it ran
/// under (absent until `with_request` is called) and the execution and skill
/// that produced it.
#[derive(Debug, Clone, Default)]
pub struct ArtifactProvenance {
    request: Option<ExecutionMetadata>,
    execution_id: Option<String>,
    skill: Option<(SkillId, SkillName)>,
}

impl ArtifactProvenance {
    pub fn set_request(&mut self, ctx: &RequestContext) {
        self.request = Some(ExecutionMetadata::with_request(ctx));
    }

    pub fn set_metadata(&mut self, metadata: ExecutionMetadata) {
        self.request = Some(metadata);
    }

    pub fn set_execution_id(&mut self, id: impl Into<String>) {
        self.execution_id = Some(id.into());
    }

    pub fn set_skill(&mut self, id: SkillId, name: SkillName) {
        self.skill = Some((id, name));
    }

    pub fn execution_id(&self) -> Option<&str> {
        self.execution_id.as_deref().or_else(|| {
            self.request
                .as_ref()
                .and_then(|metadata| metadata.execution_id.as_deref())
        })
    }

    pub fn metadata(&self) -> Option<ExecutionMetadata> {
        let mut metadata = self.request.clone()?;
        if let Some(id) = &self.execution_id {
            metadata.execution_id = Some(id.clone());
        }
        if let Some((id, name)) = &self.skill {
            metadata.skill_id = Some(id.clone());
            metadata.skill_name = Some(name.clone());
        }
        Some(metadata)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolResponse<T> {
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

    // JSON: A2A `Artifact.metadata` map.
    pub fn to_json(&self) -> Result<JsonValue, serde_json::Error> {
        serde_json::to_value(self)
    }
}
