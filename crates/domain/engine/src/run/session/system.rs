//! The system prompt of one step: the agent's prompt, the environment,
//! instructions, the admitted extensions' sections, skills, subagents, the
//! governing organisation and the plan or build mode reminder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::prompt::system::{
    self, Environment, ExtensionSection, InstructionFile, NamedEntry, ShellInUse, SystemInput,
};
use crate::prompt::{agents, notices, tool_names};
use crate::store::MessageWithParts;

use super::attachments::slash;
use super::run::StepContext;
use crate::run::directory::prompt_shell;
use crate::run::extension::PromptContext;
use crate::run::tools::registry::ToolBox;

pub(crate) fn prompt(
    ctx: &StepContext,
    toolbox: &ToolBox,
    history: &[MessageWithParts],
) -> Vec<String> {
    let config = &ctx.config;
    let instructions: Vec<InstructionFile<'_>> = config
        .instructions
        .iter()
        .map(|file| InstructionFile {
            path: &file.path,
            text: &file.text,
        })
        .collect();
    let skills: Vec<NamedEntry<'_>> = if toolbox.has(tool_names::SKILL) {
        config
            .skills
            .values()
            .map(|skill| NamedEntry {
                name: &skill.name,
                description: &skill.description,
            })
            .collect()
    } else {
        Vec::new()
    };
    let subagents: Vec<NamedEntry<'_>> = if toolbox.has(tool_names::TASK) {
        config
            .subagents()
            .filter(|agent| agent.name != ctx.agent.name)
            .map(|agent| NamedEntry {
                name: &agent.name,
                description: agent.description.as_deref().unwrap_or_default(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let program = ctx.dir.shell.path.display().to_string();
    let shell = toolbox.has(tool_names::SHELL).then(|| ShellInUse {
        kind: prompt_shell(ctx.dir.shell.kind),
        program: &program,
    });
    let date = chrono::Local::now().format("%Y-%m-%d (%A)").to_string();
    let governed_by = ctx.inner.governed.governed_by();
    let step = PromptContext {
        session_id: &ctx.session.id,
        agent: &ctx.agent.name,
        model_id: &ctx.model.id,
        directory: &ctx.session.directory,
        subagent: ctx.session.parent_id.is_some(),
    };
    let contributed = ctx.inner.governed.prompt_sections(&step);
    let extension_sections: Vec<ExtensionSection<'_>> = contributed
        .iter()
        .map(|section| ExtensionSection {
            heading: &section.item.heading,
            text: &section.item.text,
        })
        .collect();
    let extra = mode_reminder(ctx, history);
    let input = SystemInput {
        agent_prompt: ctx.agent.prompt.as_deref(),
        environment: Environment {
            working_directory: &ctx.session.directory,
            worktree: &config.worktree.root,
            is_git: config.worktree.is_git(),
            platform: std::env::consts::OS,
            shell,
            date: &date,
            model_id: &ctx.model.id,
        },
        instructions: &instructions,
        extension_sections: &extension_sections,
        skills: &skills,
        subagents: &subagents,
        governed_by: governed_by.as_deref(),
        extra: extra.as_deref(),
    };
    system::build(&input)
}

fn mode_reminder(ctx: &StepContext, history: &[MessageWithParts]) -> Option<String> {
    if ctx.agent.name == agents::PLAN.name {
        let plan = ctx
            .inner
            .paths
            .state
            .join("plans")
            .join(format!("{}.md", ctx.session.id));
        return Some(notices::plan_mode(Some(slash(&plan).as_str())));
    }
    let previous = history.iter().rev().find_map(|message| {
        message
            .message
            .as_assistant()
            .filter(|assistant| !assistant.system)
    })?;
    (previous.agent == agents::PLAN.name).then(notices::build_switch)
}
