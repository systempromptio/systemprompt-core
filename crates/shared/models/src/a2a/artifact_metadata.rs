use chrono::Utc;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_traits::validation::{
    MetadataValidation, Validate, ValidationError, ValidationResult,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactMetadata {
    pub artifact_type: String,
    pub context_id: ContextId,
    pub created_at: String,
    pub task_id: TaskId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendering_hints: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_execution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_internal: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
}

impl ArtifactMetadata {
    pub fn new(artifact_type: String, context_id: ContextId, task_id: TaskId) -> Self {
        Self {
            artifact_type,
            context_id,
            task_id,
            created_at: Utc::now().to_rfc3339(),
            rendering_hints: None,
            source: Some("mcp_tool".to_string()),
            mcp_execution_id: None,
            mcp_schema: None,
            is_internal: None,
            fingerprint: None,
            tool_name: None,
            execution_index: None,
            skill_id: None,
            skill_name: None,
        }
    }

    pub fn with_rendering_hints(mut self, hints: serde_json::Value) -> Self {
        self.rendering_hints = Some(hints);
        self
    }

    pub fn with_source(mut self, source: String) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_mcp_execution_id(mut self, id: String) -> Self {
        self.mcp_execution_id = Some(id);
        self
    }

    pub fn with_mcp_schema(mut self, schema: serde_json::Value) -> Self {
        self.mcp_schema = Some(schema);
        self
    }

    pub const fn with_is_internal(mut self, is_internal: bool) -> Self {
        self.is_internal = Some(is_internal);
        self
    }

    pub fn with_fingerprint(mut self, fingerprint: String) -> Self {
        self.fingerprint = Some(fingerprint);
        self
    }

    pub fn with_tool_name(mut self, tool_name: String) -> Self {
        self.tool_name = Some(tool_name);
        self
    }

    pub const fn with_execution_index(mut self, index: usize) -> Self {
        self.execution_index = Some(index);
        self
    }

    pub fn with_skill_id(mut self, skill_id: String) -> Self {
        self.skill_id = Some(skill_id);
        self
    }

    pub fn with_skill_name(mut self, skill_name: String) -> Self {
        self.skill_name = Some(skill_name);
        self
    }

    pub fn with_skill(mut self, skill_id: String, skill_name: String) -> Self {
        self.skill_id = Some(skill_id);
        self.skill_name = Some(skill_name);
        self
    }

    pub fn new_validated(
        artifact_type: String,
        context_id: ContextId,
        task_id: TaskId,
    ) -> ValidationResult<Self> {
        if artifact_type.is_empty() {
            return Err(ValidationError::new(
                "artifact_type",
                "Cannot create ArtifactMetadata: artifact_type is empty",
            )
            .with_context(format!(
                "artifact_type={artifact_type:?}, context_id={context_id:?}, task_id={task_id:?}"
            )));
        }

        let metadata = Self {
            artifact_type,
            context_id,
            task_id,
            created_at: Utc::now().to_rfc3339(),
            rendering_hints: None,
            source: Some("mcp_tool".to_string()),
            mcp_execution_id: None,
            mcp_schema: None,
            is_internal: None,
            fingerprint: None,
            tool_name: None,
            execution_index: None,
            skill_id: None,
            skill_name: None,
        };

        metadata.validate()?;
        Ok(metadata)
    }
}

impl Validate for ArtifactMetadata {
    fn validate(&self) -> ValidationResult<()> {
        self.validate_required_fields()?;
        Ok(())
    }
}

impl MetadataValidation for ArtifactMetadata {
    fn required_string_fields(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("artifact_type", &self.artifact_type),
            ("context_id", self.context_id.as_str()),
            ("task_id", self.task_id.as_str()),
            ("created_at", &self.created_at),
        ]
    }
}
