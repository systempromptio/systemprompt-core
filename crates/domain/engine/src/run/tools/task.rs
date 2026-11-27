//! The task tool: starts a subagent in a child session and returns its final
//! answer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use crate::prompt::engine_tools::{TASK_DESCRIPTION, task_schema};
use crate::prompt::tool_names::TASK;
use crate::tools::{PermissionRequest, Tool, ToolContext, ToolError, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::run::inner::Inner;
use crate::run::session::subtask::{self, SubtaskRequest};

pub(crate) struct TaskTool {
    inner: Arc<Inner>,
    subagents: Vec<String>,
    model: String,
    caller_writes: bool,
}

impl TaskTool {
    pub(crate) const fn new(
        inner: Arc<Inner>,
        subagents: Vec<String>,
        model: String,
        caller_writes: bool,
    ) -> Self {
        Self {
            inner,
            subagents,
            model,
            caller_writes,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Input {
    description: String,
    prompt: String,
    agent: String,
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        TASK
    }

    fn description(&self) -> String {
        TASK_DESCRIPTION.to_owned()
    }

    // JSON: the tool's input schema is JSON Schema, sent to the model as it is.
    fn schema(&self) -> Value {
        let names: Vec<&str> = self.subagents.iter().map(String::as_str).collect();
        task_schema(&names)
    }

    // JSON: the object the model wrote, read into this tool's input.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input =
            serde_json::from_value(input).map_err(|error| ToolError::malformed("", error))?;
        ctx.ask(PermissionRequest {
            permission: TASK.to_owned(),
            patterns: vec![input.agent.clone()],
            always: vec![input.agent.clone()],
            title: format!("Start the {} agent: {}", input.agent, input.description),
            metadata: json!({ "agent": input.agent, "description": input.description }),
        })
        .await?;
        let request = SubtaskRequest {
            parent: ctx.session_id.clone(),
            directory: ctx.cwd.clone(),
            agent: input.agent.clone(),
            description: input.description.clone(),
            prompt: input.prompt,
            model: self.model.clone(),
            caller_writes: self.caller_writes,
        };
        let description = input.description.clone();
        let result = subtask::run(&self.inner, request, &ctx.cancel, |child| {
            let metadata = json!({ "session_id": child.to_string(), "agent": input.agent });
            ctx.report(&description, metadata);
        })
        .await?;
        let output = if result.text.trim().is_empty() {
            format!(
                "The {} agent finished without a final message.",
                input.agent
            )
        } else {
            result.text
        };
        let metadata = json!({ "session_id": result.session.to_string(), "agent": input.agent });
        Ok(ToolOutput::new(input.description, output).with_metadata(metadata))
    }
}
