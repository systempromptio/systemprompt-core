use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::registry::TuiTool;
use crate::tools::{RiskLevel, ToolResult};

#[derive(Debug, Clone, Copy)]
pub struct ServicesStatusTool;

#[async_trait]
impl TuiTool for ServicesStatusTool {
    fn name(&self) -> &'static str {
        "services_status"
    }

    fn description(&self) -> &'static str {
        "Get detailed status of a specific service or all services"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service_name": {
                    "type": "string",
                    "description": "Name of the service to check (optional, checks all if not provided)"
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
        args.get("service_name")
            .and_then(|v| v.as_str())
            .map(|name| format!("Check status of: {name}"))
    }

    async fn execute(&self, _args: Value, _context: &Arc<()>) -> Result<ToolResult> {
        Ok(ToolResult::error(
            "Service status checking is not available in TUI mode. This feature requires direct \
             server access. Use the Services tab to view service status."
                .to_string(),
        ))
    }
}
