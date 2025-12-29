use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use super::{RiskLevel, ToolResult};

#[async_trait]
pub trait TuiTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Value;
    fn risk_level(&self) -> RiskLevel;
    fn requires_approval(&self) -> bool;
    fn preview(&self, args: &Value) -> Option<String>;

    async fn execute(&self, args: Value, context: &Arc<()>) -> Result<ToolResult>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn TuiTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn TuiTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn TuiTool>> {
        self.tools.get(name)
    }

    pub fn list(&self) -> impl Iterator<Item = &Arc<dyn TuiTool>> {
        self.tools.values()
    }

    pub fn to_mcp_tool_definitions(&self) -> Vec<McpToolDefinition> {
        self.tools
            .values()
            .map(|tool| McpToolDefinition {
                name: tool.name().to_string(),
                description: Some(tool.description().to_string()),
                input_schema: tool.parameters(),
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}
