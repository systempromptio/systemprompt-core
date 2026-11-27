//! The skill tool: loads a discovered skill's instructions into the
//! conversation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use crate::config::Loaded;
use crate::prompt::engine_tools::{SKILL_DESCRIPTION, skill_content, skill_schema};
use crate::prompt::tool_names::SKILL;
use crate::tools::{PermissionRequest, Tool, ToolContext, ToolError, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::run::session::attachments::slash;

pub(crate) struct SkillTool {
    config: Arc<Loaded>,
}

impl SkillTool {
    pub(crate) const fn new(config: Arc<Loaded>) -> Self {
        Self { config }
    }
}

#[derive(Debug, Deserialize)]
struct Input {
    name: String,
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        SKILL
    }

    fn description(&self) -> String {
        SKILL_DESCRIPTION.to_owned()
    }

    // JSON: the tool's input schema is JSON Schema, sent to the model as it is.
    fn schema(&self) -> Value {
        let names: Vec<&str> = self.config.skills.keys().map(String::as_str).collect();
        skill_schema(&names)
    }

    // JSON: the object the model wrote, read into this tool's input.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input =
            serde_json::from_value(input).map_err(|error| ToolError::malformed("", error))?;
        let Some(skill) = self.config.skills.get(input.name.trim()) else {
            let known: Vec<&str> = self.config.skills.keys().map(String::as_str).collect();
            let known = known.join(", ");
            let name = &input.name;
            let message = format!("there is no skill named {name}; the skills are: {known}");
            return Err(ToolError::invalid(message));
        };
        let directory = slash(&skill.dir);
        ctx.ask(PermissionRequest {
            permission: SKILL.to_owned(),
            patterns: vec![skill.name.clone()],
            always: vec![skill.name.clone()],
            title: format!("Load the {} skill", skill.name),
            metadata: json!({ "name": skill.name, "directory": directory }),
        })
        .await?;
        let title = format!("Loaded skill {}", skill.name);
        let content = skill_content(&skill.name, &directory, &skill.body);
        let metadata = json!({ "name": skill.name, "directory": directory });
        Ok(ToolOutput::new(title, content).with_metadata(metadata))
    }
}
