//! Multi-step tool-execution planning and result tracking.
//!
//! A [`PlanningResult`] is either a direct response or a sequence of
//! [`PlannedToolCall`]s. As calls run, [`ExecutionState`] accumulates
//! [`ToolCallResult`]s and halts on the first failure. [`TemplateRef`] parses
//! the `$N.output.field` references that let a later call consume an earlier
//! call's output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::LazyLock;

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

    pub const fn tool_count(&self) -> usize {
        match self {
            Self::DirectResponse { .. } => 0,
            Self::ToolCalls { calls, .. } => calls.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedToolCall {
    pub tool_name: String,
    // JSON: MCP tool-call arguments / result are the tool's own JSON.
    pub arguments: Value,
}

impl PlannedToolCall {
    // JSON: MCP tool-call arguments / result are the tool's own JSON.
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
    // JSON: MCP tool-call arguments / result are the tool's own JSON.
    pub arguments: Value,
    pub success: bool,
    // JSON: MCP tool-call arguments / result are the tool's own JSON.
    pub output: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    // JSON: MCP `_meta` is an open map of vendor-prefixed keys.
    pub meta: Option<Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl ToolCallResult {
    pub const fn success(
        tool_name: String,
        // JSON: MCP tool-call arguments / result are the tool's own JSON.
        arguments: Value,
        // JSON: MCP tool-call arguments / result are the tool's own JSON.
        output: Value,
        duration_ms: u64,
    ) -> Self {
        Self {
            tool_name,
            arguments,
            success: true,
            output,
            meta: None,
            error: None,
            duration_ms,
        }
    }

    #[must_use]
    // JSON: MCP `_meta` is an open map of vendor-prefixed keys.
    pub fn with_meta(mut self, meta: Option<Value>) -> Self {
        self.meta = meta;
        self
    }

    pub fn failure(
        tool_name: String,
        // JSON: MCP tool-call arguments / result are the tool's own JSON.
        arguments: Value,
        error: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            tool_name,
            arguments,
            success: false,
            output: Value::Null,
            meta: None,
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
            self.halt_reason.clone_from(&result.error);
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

#[expect(
    clippy::expect_used,
    reason = "compile-time-constant regex; failure is a programmer bug, not runtime input"
)]
static TEMPLATE_REF_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\$(\d+)\.output\.(.+)$")
        .expect("TEMPLATE_REF_REGEX is a valid regex - this is a compile-time constant")
});

impl TemplateRef {
    pub fn parse(template: &str) -> Option<Self> {
        let caps = TEMPLATE_REF_REGEX.captures(template)?;

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
