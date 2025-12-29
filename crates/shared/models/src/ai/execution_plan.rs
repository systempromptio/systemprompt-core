//! Execution Plan Types for Plan-Based Agentic Execution
//!
//! Simplified model: PLAN → EXECUTE → RESPOND
//!
//! - PLAN: AI outputs tool_calls[] or direct_response with template references
//! - EXECUTE: Sequential tool execution with template resolution
//! - RESPOND: AI generates response with full context
//!
//! Template syntax: `$N.output.field.path` references output from tool at index
//! N

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlanningResult {
    DirectResponse {
        content: String,
    },
    ToolCalls {
        reasoning: String,
        calls: Vec<PlannedToolCall>,
    },
}

impl PlanningResult {
    pub fn direct_response(content: impl Into<String>) -> Self {
        Self::DirectResponse {
            content: content.into(),
        }
    }

    pub fn tool_calls(reasoning: impl Into<String>, calls: Vec<PlannedToolCall>) -> Self {
        Self::ToolCalls {
            reasoning: reasoning.into(),
            calls,
        }
    }

    pub const fn is_direct(&self) -> bool {
        matches!(self, Self::DirectResponse { .. })
    }

    pub const fn is_tool_calls(&self) -> bool {
        matches!(self, Self::ToolCalls { .. })
    }

    pub fn tool_count(&self) -> usize {
        match self {
            Self::DirectResponse { .. } => 0,
            Self::ToolCalls { calls, .. } => calls.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedToolCall {
    pub tool_name: String,
    pub arguments: Value,
}

impl PlannedToolCall {
    pub fn new(tool_name: impl Into<String>, arguments: Value) -> Self {
        Self {
            tool_name: tool_name.into(),
            arguments,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub tool_name: String,
    pub arguments: Value,
    pub success: bool,
    pub output: Value,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl ToolCallResult {
    pub const fn success(
        tool_name: String,
        arguments: Value,
        output: Value,
        duration_ms: u64,
    ) -> Self {
        Self {
            tool_name,
            arguments,
            success: true,
            output,
            error: None,
            duration_ms,
        }
    }

    pub fn failure(
        tool_name: String,
        arguments: Value,
        error: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            tool_name,
            arguments,
            success: false,
            output: Value::Null,
            error: Some(error.into()),
            duration_ms,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionState {
    pub results: Vec<ToolCallResult>,
    pub halted: bool,
    pub halt_reason: Option<String>,
}

impl ExecutionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_result(&mut self, result: ToolCallResult) {
        if !result.success && !self.halted {
            self.halted = true;
            self.halt_reason = result.error.clone();
        }
        self.results.push(result);
    }

    pub fn successful_results(&self) -> Vec<&ToolCallResult> {
        self.results.iter().filter(|r| r.success).collect()
    }

    pub fn failed_results(&self) -> Vec<&ToolCallResult> {
        self.results.iter().filter(|r| !r.success).collect()
    }

    pub fn total_duration_ms(&self) -> u64 {
        self.results.iter().map(|r| r.duration_ms).sum()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRef {
    pub tool_index: usize,
    pub field_path: Vec<String>,
}

impl TemplateRef {
    pub fn parse(template: &str) -> Option<Self> {
        let re = Regex::new(r"^\$(\d+)\.output\.(.+)$").ok()?;
        let caps = re.captures(template)?;

        let tool_index = caps.get(1)?.as_str().parse().ok()?;
        let path = caps.get(2)?.as_str();
        let field_path = path.split('.').map(String::from).collect();

        Some(Self {
            tool_index,
            field_path,
        })
    }

    pub fn format(&self) -> String {
        format!("${}.output.{}", self.tool_index, self.field_path.join("."))
    }
}
