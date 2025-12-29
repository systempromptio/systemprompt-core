use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::registry::TuiTool;
use crate::tools::{RiskLevel, ToolResult};

#[derive(Debug, Clone, Copy)]
pub struct LogsFilterTool;

#[async_trait]
impl TuiTool for LogsFilterTool {
    fn name(&self) -> &'static str {
        "logs_filter"
    }

    fn description(&self) -> &'static str {
        "Apply a filter to the log viewer by level or module"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "level": {
                    "type": "string",
                    "enum": ["error", "warn", "info", "debug"],
                    "description": "Filter logs by level"
                },
                "module": {
                    "type": "string",
                    "description": "Filter logs by module name"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of logs to return (default: 50)",
                    "default": 50
                }
            },
            "required": []
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    fn requires_approval(&self) -> bool {
        false
    }

    fn preview(&self, args: &Value) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(level) = args.get("level").and_then(|v| v.as_str()) {
            parts.push(format!("level={level}"));
        }
        if let Some(module) = args.get("module").and_then(|v| v.as_str()) {
            parts.push(format!("module={module}"));
        }
        if parts.is_empty() {
            None
        } else {
            Some(format!("Filter: {}", parts.join(", ")))
        }
    }

    async fn execute(&self, _args: Value, _context: &Arc<()>) -> Result<ToolResult> {
        Ok(ToolResult::error(
            "Log filtering is not available in TUI mode. Use the Logs tab to view and filter logs."
                .to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LogsSearchTool;

#[async_trait]
impl TuiTool for LogsSearchTool {
    fn name(&self) -> &'static str {
        "logs_search"
    }

    fn description(&self) -> &'static str {
        "Search through log entries for a specific pattern"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (case-insensitive)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 50)",
                    "default": 50
                }
            },
            "required": ["query"]
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    fn requires_approval(&self) -> bool {
        false
    }

    fn preview(&self, args: &Value) -> Option<String> {
        args.get("query")
            .and_then(|v| v.as_str())
            .map(|q| format!("Search logs: \"{}\"", q))
    }

    async fn execute(&self, _args: Value, _context: &Arc<()>) -> Result<ToolResult> {
        Ok(ToolResult::error(
            "Log searching is not available in TUI mode. Use the Logs tab to view and search logs."
                .to_string(),
        ))
    }
}
