use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::registry::TuiTool;
use crate::tools::{RiskLevel, ToolResult};

#[derive(Debug, Clone, Copy)]
pub struct DbTablesTool;

#[async_trait]
impl TuiTool for DbTablesTool {
    fn name(&self) -> &'static str {
        "db_tables"
    }

    fn description(&self) -> &'static str {
        "List all tables in the database"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    fn requires_approval(&self) -> bool {
        false
    }

    fn preview(&self, _args: &Value) -> Option<String> {
        None
    }

    async fn execute(&self, _args: Value, _context: &Arc<()>) -> Result<ToolResult> {
        Ok(ToolResult::error(
            "Database schema inspection is not available in TUI mode. This feature requires \
             direct server access."
                .to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DbDescribeTool;

#[async_trait]
impl TuiTool for DbDescribeTool {
    fn name(&self) -> &'static str {
        "db_describe"
    }

    fn description(&self) -> &'static str {
        "Describe the schema of a database table"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "table_name": {
                    "type": "string",
                    "description": "Name of the table to describe"
                }
            },
            "required": ["table_name"]
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    fn requires_approval(&self) -> bool {
        false
    }

    fn preview(&self, args: &Value) -> Option<String> {
        args.get("table_name")
            .and_then(|v| v.as_str())
            .map(|name| format!("Describe table: {name}"))
    }

    async fn execute(&self, _args: Value, _context: &Arc<()>) -> Result<ToolResult> {
        Ok(ToolResult::error(
            "Database schema inspection is not available in TUI mode. This feature requires \
             direct server access."
                .to_string(),
        ))
    }
}
