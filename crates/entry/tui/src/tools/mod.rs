pub mod definitions;
mod executor;
mod registry;

pub use executor::ToolExecutor;
pub use registry::ToolRegistry;

use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PendingToolCall {
    pub id: Uuid,
    pub tool_name: String,
    pub arguments: Value,
    pub description: String,
    pub risk_level: RiskLevel,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Safe,
    Moderate,
    Dangerous,
}

impl RiskLevel {
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Safe => "✓",
            Self::Moderate => "⚠",
            Self::Dangerous => "⛔",
        }
    }

    pub const fn label(&self) -> &'static str {
        match self {
            Self::Safe => "Safe",
            Self::Moderate => "Moderate",
            Self::Dangerous => "Dangerous",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

impl ToolResult {
    pub const fn success(output: String) -> Self {
        Self {
            success: true,
            output,
            error: None,
        }
    }

    pub const fn error(error: String) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error),
        }
    }
}
