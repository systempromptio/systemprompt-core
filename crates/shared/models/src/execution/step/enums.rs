//! Step identifier and lifecycle / kind enums.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepId(pub String);

impl StepId {
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for StepId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for StepId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for StepId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for StepStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "in_progress" | "running" | "active" => Ok(Self::InProgress),
            "completed" | "done" | "success" => Ok(Self::Completed),
            "failed" | "error" => Ok(Self::Failed),
            _ => Err(format!("Invalid step status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    #[default]
    Understanding,
    Planning,
    SkillUsage,
    ToolExecution,
    Completion,
}

impl std::fmt::Display for StepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Understanding => write!(f, "understanding"),
            Self::Planning => write!(f, "planning"),
            Self::SkillUsage => write!(f, "skill_usage"),
            Self::ToolExecution => write!(f, "tool_execution"),
            Self::Completion => write!(f, "completion"),
        }
    }
}

impl std::str::FromStr for StepType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "understanding" => Ok(Self::Understanding),
            "planning" => Ok(Self::Planning),
            "skill_usage" => Ok(Self::SkillUsage),
            "tool_execution" | "toolexecution" => Ok(Self::ToolExecution),
            "completion" => Ok(Self::Completion),
            _ => Err(format!("Invalid step type: {s}")),
        }
    }
}
