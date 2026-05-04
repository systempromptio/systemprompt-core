//! Per-kind step content payload and the [`PlannedTool`] descriptor.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::SkillId;

use super::enums::StepType;

/// Tool-call descriptor produced during the planning step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedTool {
    /// Name of the tool the planner intends to invoke.
    pub tool_name: String,
    /// Tool arguments as a JSON value.
    pub arguments: serde_json::Value,
}

/// Per-kind payload attached to an [`super::ExecutionStep`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepContent {
    /// Reasoning step with no payload.
    Understanding,
    /// Planning step.
    Planning {
        /// Optional planner reasoning trace.
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        /// Optional set of tool calls the planner intends to issue.
        #[serde(skip_serializing_if = "Option::is_none")]
        planned_tools: Option<Vec<PlannedTool>>,
    },
    /// Invocation of a declared skill.
    SkillUsage {
        /// Stable skill identifier.
        skill_id: SkillId,
        /// Human-readable skill name.
        skill_name: String,
    },
    /// Concrete tool execution.
    ToolExecution {
        /// Name of the tool being invoked.
        tool_name: String,
        /// Tool arguments as a JSON value.
        tool_arguments: serde_json::Value,
        /// Tool result payload, populated once the call returns.
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_result: Option<serde_json::Value>,
    },
    /// Terminal completion step.
    Completion,
}

impl StepContent {
    /// Build an [`Self::Understanding`] payload.
    #[must_use]
    pub const fn understanding() -> Self {
        Self::Understanding
    }

    /// Build a [`Self::Planning`] payload from optional reasoning and tool
    /// plans.
    #[must_use]
    pub const fn planning(
        reasoning: Option<String>,
        planned_tools: Option<Vec<PlannedTool>>,
    ) -> Self {
        Self::Planning {
            reasoning,
            planned_tools,
        }
    }

    /// Build a [`Self::SkillUsage`] payload.
    pub fn skill_usage(skill_id: SkillId, skill_name: impl Into<String>) -> Self {
        Self::SkillUsage {
            skill_id,
            skill_name: skill_name.into(),
        }
    }

    /// Build a [`Self::ToolExecution`] payload with no result yet.
    pub fn tool_execution(tool_name: impl Into<String>, tool_arguments: serde_json::Value) -> Self {
        Self::ToolExecution {
            tool_name: tool_name.into(),
            tool_arguments,
            tool_result: None,
        }
    }

    /// Build a [`Self::Completion`] payload.
    #[must_use]
    pub const fn completion() -> Self {
        Self::Completion
    }

    /// High-level kind matching this payload variant.
    #[must_use]
    pub const fn step_type(&self) -> StepType {
        match self {
            Self::Understanding => StepType::Understanding,
            Self::Planning { .. } => StepType::Planning,
            Self::SkillUsage { .. } => StepType::SkillUsage,
            Self::ToolExecution { .. } => StepType::ToolExecution,
            Self::Completion => StepType::Completion,
        }
    }

    /// Render a UI-friendly title for this step.
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::Understanding => "Analyzing request...".to_string(),
            Self::Planning { .. } => "Planning response...".to_string(),
            Self::SkillUsage { skill_name, .. } => format!("Using {skill_name} skill..."),
            Self::ToolExecution { tool_name, .. } => format!("Running {tool_name}..."),
            Self::Completion => "Complete".to_string(),
        }
    }

    /// True for steps that complete instantly (everything except tool
    /// execution).
    #[must_use]
    pub const fn is_instant(&self) -> bool {
        !matches!(self, Self::ToolExecution { .. })
    }

    /// Borrow the tool / skill name carried by this step, if any.
    #[must_use]
    pub fn tool_name(&self) -> Option<&str> {
        match self {
            Self::ToolExecution { tool_name, .. } => Some(tool_name),
            Self::SkillUsage { skill_name, .. } => Some(skill_name),
            Self::Understanding | Self::Planning { .. } | Self::Completion => None,
        }
    }

    /// Borrow the tool arguments carried by this step, if any.
    #[must_use]
    pub const fn tool_arguments(&self) -> Option<&serde_json::Value> {
        match self {
            Self::ToolExecution { tool_arguments, .. } => Some(tool_arguments),
            Self::Understanding
            | Self::Planning { .. }
            | Self::SkillUsage { .. }
            | Self::Completion => None,
        }
    }

    /// Borrow the tool result carried by this step, if any.
    #[must_use]
    pub const fn tool_result(&self) -> Option<&serde_json::Value> {
        match self {
            Self::ToolExecution { tool_result, .. } => tool_result.as_ref(),
            Self::Understanding
            | Self::Planning { .. }
            | Self::SkillUsage { .. }
            | Self::Completion => None,
        }
    }

    /// Borrow planner reasoning carried by this step, if any.
    #[must_use]
    pub fn reasoning(&self) -> Option<&str> {
        match self {
            Self::Planning { reasoning, .. } => reasoning.as_deref(),
            Self::Understanding
            | Self::SkillUsage { .. }
            | Self::ToolExecution { .. }
            | Self::Completion => None,
        }
    }

    /// Borrow planned tool calls carried by this step, if any.
    #[must_use]
    pub fn planned_tools(&self) -> Option<&[PlannedTool]> {
        match self {
            Self::Planning { planned_tools, .. } => planned_tools.as_deref(),
            Self::Understanding
            | Self::SkillUsage { .. }
            | Self::ToolExecution { .. }
            | Self::Completion => None,
        }
    }

    /// Attach a result to a [`Self::ToolExecution`] payload (no-op for
    /// other variants).
    #[must_use]
    pub fn with_tool_result(self, result: serde_json::Value) -> Self {
        match self {
            Self::ToolExecution {
                tool_name,
                tool_arguments,
                ..
            } => Self::ToolExecution {
                tool_name,
                tool_arguments,
                tool_result: Some(result),
            },
            other @ (Self::Understanding
            | Self::Planning { .. }
            | Self::SkillUsage { .. }
            | Self::Completion) => other,
        }
    }
}
