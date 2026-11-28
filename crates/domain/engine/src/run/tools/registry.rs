//! The tools one step offers: built-ins, the admitted extensions' tools, the
//! engine's own tools and MCP tools, filtered by the agent's tool filter and by
//! rules that deny a tool outright. The file writers share the `edit`
//! permission; every other tool is governed by its own name. The built-in file
//! tools and the engine's own tools ask the gate themselves; any other tool is
//! asked about by the engine before it runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::config::{Action, permission};
use crate::llm::ToolDef;
use crate::prompt::tool_names;
use crate::tools::{Tool, names};

use super::mcp::McpToolAdapter;
use super::question::QuestionTool;
use super::skill::SkillTool;
use super::task::TaskTool;
use crate::run::extension::ExtensionTool;
use crate::run::session::run::StepContext;
use crate::run::session::subtask::can_write;

pub(crate) struct ToolBox {
    tools: Vec<Arc<dyn Tool>>,
    extension_tools: Vec<ExtensionTool>,
    skills: Vec<String>,
}

impl ToolBox {
    pub(crate) fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.iter().find(|tool| tool.name() == name)
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub(crate) fn permission<'a>(&'a self, name: &'a str) -> &'a str {
        self.extension_tool(name)
            .map_or_else(|| permission_for(name), |tool| tool.permission)
    }

    pub(crate) fn asks_itself(&self, name: &str) -> bool {
        self.extension_tool(name)
            .map_or_else(|| asks_itself(name), |tool| tool.asks_itself)
    }

    fn extension_tool(&self, name: &str) -> Option<&ExtensionTool> {
        self.extension_tools
            .iter()
            .find(|tool| tool.tool.name() == name)
    }

    /// The skills the skill tool offers: none when the tool is not offered,
    /// and never one a rule denies by name.
    pub(crate) fn offers_skill(&self, name: &str) -> bool {
        self.skills.iter().any(|skill| skill == name)
    }

    pub(crate) fn names(&self) -> Vec<&str> {
        self.tools.iter().map(|tool| tool.name()).collect()
    }

    pub(crate) fn defs(&self) -> Vec<ToolDef> {
        self.tools
            .iter()
            .map(|tool| ToolDef {
                name: tool.name().to_owned(),
                description: tool.description(),
                input_schema: tool.schema(),
            })
            .collect()
    }
}

pub(crate) fn permission_for(tool: &str) -> &str {
    match tool {
        names::WRITE | names::EDIT | names::PATCH => permission::EDIT,
        other => other,
    }
}

pub(crate) fn asks_itself(tool: &str) -> bool {
    matches!(
        tool,
        names::SHELL
            | names::READ
            | names::WRITE
            | names::EDIT
            | names::PATCH
            | names::GLOB
            | names::GREP
            | names::FETCH
            | tool_names::TASK
            | tool_names::SKILL
            | tool_names::QUESTION
    )
}

fn is_engine_tool(name: &str) -> bool {
    matches!(
        name,
        tool_names::TASK | tool_names::SKILL | tool_names::QUESTION
    )
}

pub(crate) fn assemble(ctx: &StepContext) -> ToolBox {
    let rules = ctx.config.rules_for(&ctx.agent, &ctx.rules);
    let visible =
        |name: &str| ctx.agent.tools.permits(name) && !rules.denies_all(permission_for(name));
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();
    for tool in &ctx.dir.builtin {
        if visible(tool.name()) {
            tools.push(Arc::clone(tool));
        }
    }
    let contributions = ctx.inner.governed.current();
    let mut extension_tools = Vec::new();
    for contributed in &contributions.tools {
        let offered = &contributed.item;
        let name = offered.tool.name();
        if is_engine_tool(name) || tools.iter().any(|tool| tool.name() == name) {
            tracing::warn!(
                extension = %contributed.extension,
                tool = name,
                "an extension tool shares the name of a tool already offered; not offered"
            );
            continue;
        }
        if ctx.agent.tools.permits(name) && !rules.denies_all(offered.permission) {
            tools.push(Arc::clone(&offered.tool));
            extension_tools.push(offered.clone());
        }
    }
    let subagents: Vec<String> = ctx
        .config
        .subagents()
        .filter(|agent| agent.name != ctx.agent.name)
        .map(|agent| agent.name.clone())
        .collect();
    if !subagents.is_empty() && visible(tool_names::TASK) {
        let caller_writes = can_write(&ctx.config, &ctx.agent);
        let model = ctx.model.id.clone();
        let task = TaskTool::new(Arc::clone(&ctx.inner), subagents, model, caller_writes);
        tools.push(Arc::new(task));
    }
    let mut skills = Vec::new();
    if visible(tool_names::SKILL) {
        skills = ctx
            .config
            .skills
            .keys()
            .filter(|name| rules.evaluate(permission::SKILL, name.as_str()) != Action::Deny)
            .cloned()
            .collect();
    }
    if !skills.is_empty() {
        let tool = SkillTool::new(Arc::clone(&ctx.config), skills.clone());
        tools.push(Arc::new(tool));
    }
    if ctx.session.parent_id.is_none() && visible(tool_names::QUESTION) {
        tools.push(Arc::new(QuestionTool::new(Arc::clone(&ctx.inner))));
    }
    for tool in ctx.dir.mcp.tools() {
        if visible(&tool.qualified_name) {
            tools.push(Arc::new(McpToolAdapter::new(ctx.dir.mcp.clone(), tool)));
        }
    }
    ToolBox {
        tools,
        extension_tools,
        skills,
    }
}
