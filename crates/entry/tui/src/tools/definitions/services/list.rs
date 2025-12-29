use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::registry::TuiTool;
use crate::tools::{RiskLevel, ToolResult};

#[derive(Debug, Clone, Copy)]
pub struct ServicesListTool;

#[async_trait]
impl TuiTool for ServicesListTool {
    fn name(&self) -> &'static str {
        "services_list"
    }

    fn description(&self) -> &'static str {
        "List all SystemPrompt services (API, agents, MCP servers) with their current status"
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
            "Service listing is not available in TUI mode. This feature requires direct server \
             access. Use the Services tab to view service status."
                .to_string(),
        ))
    }
}
