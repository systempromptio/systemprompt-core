//! Agent definitions: built-ins handed in by the caller, markdown files, and
//! per-agent settings overrides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod load;
mod overrides;
mod tools;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::permission::Ruleset;

pub(crate) use load::{AgentInputs, AgentSource, load};
pub use tools::ToolFilter;

/// Where an agent can be chosen: as the session's main agent, as a delegated
/// subagent, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    Primary,
    Subagent,
    #[default]
    All,
}

impl AgentMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "primary" => Some(Self::Primary),
            "subagent" => Some(Self::Subagent),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    pub const fn is_primary(self) -> bool {
        matches!(self, Self::Primary | Self::All)
    }

    pub const fn is_subagent(self) -> bool {
        matches!(self, Self::Subagent | Self::All)
    }
}

/// Where the last word on an agent came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Builtin,
    File(PathBuf),
    Settings,
}

/// One agent the engine can run.
///
/// `steps` caps the model calls of one turn and `max_output` the tokens of
/// each call's answer (held under the model's own ceiling); unset, the engine's
/// step default and the model's ceiling apply.
///
/// Its rules sit in two layers: `defaults` below the settings files (the base
/// an agent starts from, such as [`Ruleset::build_defaults`]) and `permission`
/// above them (rules from the agent's own file or its `agent.<name>` settings
/// entry).
#[derive(Debug, Clone, PartialEq)]
pub struct AgentDef {
    pub name: String,
    pub description: Option<String>,
    pub mode: AgentMode,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub tools: ToolFilter,
    pub permission: Ruleset,
    pub defaults: Ruleset,
    pub temperature: Option<f32>,
    pub steps: Option<u32>,
    pub max_output: Option<u32>,
    pub hidden: bool,
    pub origin: Origin,
    // JSON: front matter and settings keys the engine does not interpret, kept as written.
    pub extra: Map<String, Value>,
}

impl AgentDef {
    pub fn new(name: impl Into<String>, mode: AgentMode) -> Self {
        Self {
            name: name.into(),
            description: None,
            mode,
            model: None,
            prompt: None,
            tools: ToolFilter::default(),
            permission: Ruleset::new(),
            defaults: Ruleset::new(),
            temperature: None,
            steps: None,
            max_output: None,
            hidden: false,
            origin: Origin::Builtin,
            extra: Map::new(),
        }
    }

    pub fn apply(&mut self, patch: &AgentOverride) {
        if let Some(description) = &patch.description {
            self.description = Some(description.clone());
        }
        if let Some(mode) = patch.mode {
            self.mode = mode;
        }
        if let Some(model) = &patch.model {
            self.model = Some(model.clone());
        }
        if let Some(prompt) = &patch.prompt {
            self.prompt = Some(prompt.clone());
        }
        if let Some(tools) = &patch.tools {
            self.tools = tools.clone();
        }
        if let Some(temperature) = patch.temperature {
            self.temperature = Some(temperature);
        }
        if let Some(steps) = patch.steps {
            self.steps = Some(steps);
        }
        if let Some(max_output) = patch.max_output {
            self.max_output = Some(max_output);
        }
        if let Some(hidden) = patch.hidden {
            self.hidden = hidden;
        }
        self.permission.extend(&patch.permission);
        for (key, value) in &patch.extra {
            self.extra.insert(key.clone(), value.clone());
        }
    }
}

/// A partial agent: the front matter of an agent file or an `agent.<name>`
/// object in settings. Unset fields leave the agent as it was; rules are
/// appended to the agent's own.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentOverride {
    pub name: Option<String>,
    pub description: Option<String>,
    pub mode: Option<AgentMode>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub tools: Option<ToolFilter>,
    pub temperature: Option<f32>,
    pub steps: Option<u32>,
    pub max_output: Option<u32>,
    pub hidden: Option<bool>,
    pub disable: bool,
    pub permission: Ruleset,
    // JSON: front matter and settings keys the engine does not interpret, kept as written.
    pub extra: Map<String, Value>,
}
