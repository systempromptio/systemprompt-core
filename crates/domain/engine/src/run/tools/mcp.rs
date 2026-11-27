//! MCP server tools offered to the model under their qualified names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use crate::mcp::{McpError, McpManager, McpTool};
use crate::tools::{
    Attachment, Keep, OutputLimits, Tool, ToolContext, ToolError, ToolOutput, truncate_output,
};
use serde_json::{Value, json};

pub(crate) struct McpToolAdapter {
    manager: McpManager,
    tool: McpTool,
}

impl McpToolAdapter {
    pub(crate) const fn new(manager: McpManager, tool: McpTool) -> Self {
        Self { manager, tool }
    }

    fn call_error(&self, error: McpError) -> ToolError {
        match error {
            McpError::Cancelled => ToolError::Aborted,
            other => {
                let message = format!(
                    "the {} server could not run {}",
                    self.tool.server, self.tool.name
                );
                ToolError::caused(message, other)
            },
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    #[expect(
        clippy::misnamed_getters,
        reason = "an MCP tool is offered to the model under its server-qualified name"
    )]
    fn name(&self) -> &str {
        &self.tool.qualified_name
    }

    fn description(&self) -> String {
        if self.tool.description.trim().is_empty() {
            format!(
                "The {} tool of the {} MCP server.",
                self.tool.name, self.tool.server
            )
        } else {
            self.tool.description.clone()
        }
    }

    // JSON: the server's own input schema for the tool, sent to the model as it is.
    fn schema(&self) -> Value {
        self.tool.input_schema.clone()
    }

    // JSON: the object the model wrote, passed to the server as the tool's
    // arguments.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        ctx.ensure_live()?;
        let name = &self.tool.qualified_name;
        let result = self
            .manager
            .call(name, input, ctx.cancel.clone())
            .await
            .map_err(|error| self.call_error(error))?;
        let limits = OutputLimits::default();
        let truncated = truncate_output(result.text, limits, Keep::Head, &ctx.data_dir, name).await;
        if result.is_error {
            return Err(ToolError::failed(truncated.text));
        }
        let title = format!("{}: {}", self.tool.server, self.tool.name);
        let metadata = json!({
            "server": self.tool.server,
            "tool": self.tool.name,
            "truncated": truncated.truncated,
            "structured": result.structured,
        });
        let mut output = ToolOutput::new(title, truncated.text).with_metadata(metadata);
        for attachment in result.attachments {
            let data = attachment.data;
            let attachment = Attachment {
                mime: attachment.mime_type,
                filename: None,
                data,
            };
            output = output.with_attachment(attachment);
        }
        Ok(output)
    }
}
