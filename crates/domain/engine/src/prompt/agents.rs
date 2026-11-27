//! The built-in agents, as data. Agents defined in files are loaded elsewhere;
//! these are the ones that always exist.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::base::{CORE_PROMPT, EXPLORE_PROMPT, GENERAL_PROMPT, PLAN_PROMPT};
use crate::compaction::COMPACTION_PROMPT;
use crate::generate::{SUMMARY_PROMPT, TITLE_PROMPT};
use crate::tool_names::{ALL, EDIT, PATCH, QUESTION, TASK, TODO, WRITE};

/// Where an agent can be used: picked by the user, started through the task
/// tool, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    Primary,
    Subagent,
    All,
}

impl AgentMode {
    pub const fn is_primary(self) -> bool {
        matches!(self, Self::Primary | Self::All)
    }

    pub const fn is_subagent(self) -> bool {
        matches!(self, Self::Subagent | Self::All)
    }
}

/// An agent that ships with the engine.
///
/// `tools_denied` lists tool names the agent may never call; [`ALL`] denies
/// every tool. `read_only` agents must also not run shell commands that change
/// anything, which the permission layer enforces by asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinAgent {
    pub name: &'static str,
    pub description: &'static str,
    pub mode: AgentMode,
    pub prompt: &'static str,
    pub read_only: bool,
    pub hidden: bool,
    pub tools_denied: &'static [&'static str],
}

impl BuiltinAgent {
    pub fn denies(&self, tool: &str) -> bool {
        self.tools_denied
            .iter()
            .any(|denied| *denied == ALL || *denied == tool)
    }
}

pub const DEFAULT_AGENT: &str = "build";

pub const BUILD: BuiltinAgent = BuiltinAgent {
    name: "build",
    description: "The default agent. Reads, edits and runs commands to get the task done, within your permission settings.",
    mode: AgentMode::Primary,
    prompt: CORE_PROMPT,
    read_only: false,
    hidden: false,
    tools_denied: &[],
};

pub const PLAN: BuiltinAgent = BuiltinAgent {
    name: "plan",
    description: "Investigates and proposes a plan without changing files. Asks before anything is changed.",
    mode: AgentMode::Primary,
    prompt: PLAN_PROMPT,
    read_only: true,
    hidden: false,
    tools_denied: &[WRITE, EDIT, PATCH],
};

pub const GENERAL: BuiltinAgent = BuiltinAgent {
    name: "general",
    description: "A general-purpose subagent for delegated multi-step work: research, investigation, or a self-contained change. Start several at once for independent pieces of work.",
    mode: AgentMode::Subagent,
    prompt: GENERAL_PROMPT,
    read_only: false,
    hidden: false,
    tools_denied: &[TODO, QUESTION, TASK],
};

pub const EXPLORE: BuiltinAgent = BuiltinAgent {
    name: "explore",
    description: "A fast, read-only subagent for finding things in a codebase: files by pattern, code by keyword, or how a feature works. Say how thorough to be: a quick lookup or a sweep of every likely location.",
    mode: AgentMode::Subagent,
    prompt: EXPLORE_PROMPT,
    read_only: true,
    hidden: false,
    tools_denied: &[WRITE, EDIT, PATCH, TODO, QUESTION, TASK],
};

pub const TITLE: BuiltinAgent = BuiltinAgent {
    name: "title",
    description: "Names a conversation from its first message.",
    mode: AgentMode::Primary,
    prompt: TITLE_PROMPT,
    read_only: true,
    hidden: true,
    tools_denied: &[ALL],
};

pub const SUMMARY: BuiltinAgent = BuiltinAgent {
    name: "summary",
    description: "Summarises what a session changed.",
    mode: AgentMode::Primary,
    prompt: SUMMARY_PROMPT,
    read_only: true,
    hidden: true,
    tools_denied: &[ALL],
};

pub const COMPACTION: BuiltinAgent = BuiltinAgent {
    name: "compaction",
    description: "Condenses a long session into a summary so it can continue in a fresh context.",
    mode: AgentMode::Primary,
    prompt: COMPACTION_PROMPT,
    read_only: true,
    hidden: true,
    tools_denied: &[ALL],
};

pub const BUILTIN_AGENTS: &[BuiltinAgent] =
    &[BUILD, PLAN, GENERAL, EXPLORE, TITLE, SUMMARY, COMPACTION];

pub fn builtin(name: &str) -> Option<&'static BuiltinAgent> {
    BUILTIN_AGENTS.iter().find(|agent| agent.name == name)
}
