//! Permission rules: ordered `(permission, pattern, action)` triples where the
//! last matching rule wins.
//!
//! Rules come in layers, lowest first: an agent's built-in defaults, the global
//! settings, the project settings, the agent's own rules, then the session's
//! approvals. [`Ruleset::layered`] concatenates them in that order and
//! [`Ruleset::evaluate`] reads the result from the end.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod matching;
mod ruleset;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use matching::{inside, wildcard};
pub use ruleset::Ruleset;

pub const READ: &str = "read";
pub const EDIT: &str = "edit";
pub const SHELL: &str = "shell";
pub const GLOB: &str = "glob";
pub const GREP: &str = "grep";
pub const FETCH: &str = "fetch";
pub const TODO: &str = "todo";
pub const TASK: &str = "task";
pub const SKILL: &str = "skill";
pub const QUESTION: &str = "question";
pub const EXTERNAL_DIRECTORY: &str = "external_directory";
pub const REPEATED_CALL: &str = "repeated_call";

/// What a matching rule decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Ask,
    Deny,
}

impl Action {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "allow" => Some(Self::Allow),
            "ask" => Some(Self::Ask),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One rule: for tool permission `permission` and subject `pattern` (a path, a
/// command line, a URL), do `action`.
///
/// Both fields are wildcard patterns: `*` matches any run of characters, `?`
/// one character, and a trailing `" *"` also matches nothing, so `"git *"`
/// covers `"git"` as well as `"git status"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rule {
    pub permission: String,
    pub pattern: String,
    pub action: Action,
}

impl Rule {
    pub fn new(permission: impl Into<String>, pattern: impl Into<String>, action: Action) -> Self {
        Self {
            permission: permission.into(),
            pattern: pattern.into(),
            action,
        }
    }

    pub fn matches(&self, permission: &str, pattern: &str) -> bool {
        wildcard(&self.permission, permission) && wildcard(&self.pattern, pattern)
    }
}

/// A rule list that settings could not be turned into. Refused rather than
/// skipped, since a dropped deny widens.
#[derive(Debug, thiserror::Error)]
pub enum InvalidRule {
    #[error("`{key}`: unknown action `{value}`, expected allow, ask or deny")]
    Action { key: String, value: String },
    #[error("`{key}`: expected an action or an object of patterns to actions")]
    Shape { key: String },
    #[error("rules must be an action, an object or a list of rules")]
    Root,
    #[error("rule {index} is malformed")]
    Entry {
        index: usize,
        #[source]
        source: serde_json::Error,
    },
}
