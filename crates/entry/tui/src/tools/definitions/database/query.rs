use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::registry::TuiTool;
use crate::tools::{RiskLevel, ToolResult};

#[derive(Debug, Clone, Copy)]
pub struct DbQueryTool;

#[async_trait]
impl TuiTool for DbQueryTool {
    fn name(&self) -> &'static str {
        "db_query"
    }

    fn description(&self) -> &'static str {
        "Execute a read-only SQL query against the database"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "SQL SELECT query to execute"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of rows to return (default: 100)",
                    "default": 100
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
        args.get("query").and_then(|v| v.as_str()).map(|q| {
            if q.len() > 50 {
                format!("{}...", &q[..50])
            } else {
                q.to_string()
            }
        })
    }

    async fn execute(&self, _args: Value, _context: &Arc<()>) -> Result<ToolResult> {
        Ok(ToolResult::error(
            "Database queries are not available in TUI mode. This feature requires direct server \
             access."
                .to_string(),
        ))
    }
}
