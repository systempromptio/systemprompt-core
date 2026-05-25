//! Per-kind step content payload and the [`PlannedTool`] descriptor.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::SkillId;

use super::enums::StepType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedTool {
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepContent {
    Understanding,
    Planning {
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        planned_tools: Option<Vec<PlannedTool>>,
    },
    SkillUsage {
        skill_id: SkillId,
        skill_name: String,
    },
    ToolExecution {
        tool_name: String,
        tool_arguments: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_result: Option<serde_json::Value>,
    },
    Completion,
}

impl StepContent {
    #[must_use]
    pub const fn understanding() -> Self {
        Self::Understanding
    }

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

    pub fn skill_usage(skill_id: SkillId, skill_name: impl Into<String>) -> Self {
        Self::SkillUsage {
            skill_id,
            skill_name: skill_name.into(),
        }
    }

    pub fn tool_execution(tool_name: impl Into<String>, tool_arguments: serde_json::Value) -> Self {
        Self::ToolExecution {
            tool_name: tool_name.into(),
            tool_arguments,
            tool_result: None,
        }
    }

    #[must_use]
    pub const fn completion() -> Self {
        Self::Completion
    }

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

    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::Understanding => "Analyzing request...".to_owned(),
            Self::Planning { .. } => "Planning response...".to_owned(),
            Self::SkillUsage { skill_name, .. } => format!("Using {skill_name} skill..."),
            Self::ToolExecution { tool_name, .. } => format!("Running {tool_name}..."),
            Self::Completion => "Complete".to_owned(),
        }
    }

    #[must_use]
    pub const fn is_instant(&self) -> bool {
        !matches!(self, Self::ToolExecution { .. })
    }

    #[must_use]
    pub fn tool_name(&self) -> Option<&str> {
        match self {
            Self::ToolExecution { tool_name, .. } => Some(tool_name),
            Self::SkillUsage { skill_name, .. } => Some(skill_name),
            Self::Understanding | Self::Planning { .. } | Self::Completion => None,
        }
    }

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
