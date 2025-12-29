use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::registry::TuiTool;
use crate::tools::{RiskLevel, ToolResult};

#[derive(Debug, Clone, Copy)]
pub struct ServicesStartTool;

#[async_trait]
impl TuiTool for ServicesStartTool {
    fn name(&self) -> &'static str {
        "services_start"
    }

    fn description(&self) -> &'static str {
        "Start a SystemPrompt service (agent or MCP server)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service_name": {
                    "type": "string",
                    "description": "Name of the service to start"
                }
            },
            "required": ["service_name"]
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Moderate
    }

    fn requires_approval(&self) -> bool {
        true
    }

    fn preview(&self, args: &Value) -> Option<String> {
        args.get("service_name")
            .and_then(|v| v.as_str())
            .map(|name| format!("Start service: {name}"))
    }

    async fn execute(&self, _args: Value, _context: &Arc<()>) -> Result<ToolResult> {
        Ok(ToolResult::error(
            "Service control is not available in TUI mode. This feature requires direct server \
             access."
                .to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ServicesStopTool;

#[async_trait]
impl TuiTool for ServicesStopTool {
    fn name(&self) -> &'static str {
        "services_stop"
    }

    fn description(&self) -> &'static str {
        "Stop a running SystemPrompt service"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service_name": {
                    "type": "string",
                    "description": "Name of the service to stop"
                }
            },
            "required": ["service_name"]
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Moderate
    }

    fn requires_approval(&self) -> bool {
        true
    }

    fn preview(&self, args: &Value) -> Option<String> {
        args.get("service_name")
            .and_then(|v| v.as_str())
            .map(|name| format!("Stop service: {name}"))
    }

    async fn execute(&self, _args: Value, _context: &Arc<()>) -> Result<ToolResult> {
        Ok(ToolResult::error(
            "Service control is not available in TUI mode. This feature requires direct server \
             access."
                .to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ServicesRestartTool;

#[async_trait]
impl TuiTool for ServicesRestartTool {
    fn name(&self) -> &'static str {
        "services_restart"
    }

    fn description(&self) -> &'static str {
        "Restart a SystemPrompt service"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service_name": {
                    "type": "string",
                    "description": "Name of the service to restart"
                }
            },
            "required": ["service_name"]
        })
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Moderate
    }

    fn requires_approval(&self) -> bool {
        true
    }

    fn preview(&self, args: &Value) -> Option<String> {
        args.get("service_name")
            .and_then(|v| v.as_str())
            .map(|name| format!("Restart service: {name}"))
    }

    async fn execute(&self, _args: Value, _context: &Arc<()>) -> Result<ToolResult> {
        Ok(ToolResult::error(
            "Service control is not available in TUI mode. This feature requires direct server \
             access."
                .to_string(),
        ))
    }
}
