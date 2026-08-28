//! What a governed call asks for, and what it carries.
//!
//! The governance chain sees two kinds of call: an MCP tool invocation and a
//! prompt the user submitted. Both reach the model and both are enforced, but
//! they differ in what a policy may key on — a prompt names no tool — and in
//! how a finding must be reported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::McpToolName;

pub const PROMPT_TARGET_NAME: &str = "user_prompt";

pub const UNKNOWN_TARGET_NAME: &str = "unknown";

/// Untyped MCP tool input wrapped at the protocol boundary.
///
/// The MCP protocol mandates schema-less JSON for tool arguments — every tool
/// defines its own input shape. This wrapper is the single point where
/// governance reaches into that JSON; everywhere else the typed path is
/// preferred. Callers extract fields via [`Self::as_str`] / [`Self::as_path`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpToolInput(
    // JSON: MCP-protocol boundary — schema-less tool arguments mandated by the spec.
    serde_json::Value,
);

impl McpToolInput {
    #[must_use]
    pub const fn new(value: serde_json::Value) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_value(&self) -> &serde_json::Value {
        &self.0
    }

    #[must_use]
    pub fn as_str(&self, field: &str) -> Option<&str> {
        self.0.get(field).and_then(serde_json::Value::as_str)
    }

    #[must_use]
    pub fn as_path(&self, field: &str) -> Option<&str> {
        self.as_str(field)
    }
}

/// What a governed call is asking the platform to do.
///
/// A prompt is a distinct variant rather than a reserved tool name, which would
/// collide with any tool a deployment happened to name the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GovernedTarget {
    Tool { tool: McpToolName },
    Prompt,
    Unknown,
}

impl GovernedTarget {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Tool { tool } => tool.as_str(),
            Self::Prompt => PROMPT_TARGET_NAME,
            Self::Unknown => UNKNOWN_TARGET_NAME,
        }
    }

    #[must_use]
    pub const fn tool(&self) -> Option<&McpToolName> {
        match self {
            Self::Tool { tool } => Some(tool),
            Self::Prompt | Self::Unknown => None,
        }
    }
}

/// The payload a governance policy inspects.
///
/// A finding is reported against the surface it was found on, so arguments and
/// prompt text stay separate variants rather than one JSON blob under a
/// conventional key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GovernedInput {
    ToolArguments { arguments: McpToolInput },
    Prompt { parts: Vec<PromptPart> },
}

/// One text surface of a governed prompt submission, named by its source.
///
/// The path is where the text came from — `system`, `messages[2].user`,
/// `forwarded.tools[0].description` — so a finding is reported against its
/// true source, not an anonymous blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptPart {
    pub path: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernedString<'a> {
    pub path: String,
    pub value: &'a str,
}

/// One non-container JSON value of a governed call's arguments, with the path
/// it was found at.
///
/// Why: `strings()` cannot serve a policy that compares numbers, and a second
/// traversal to reach them would be a second path grammar to keep in step with
/// the first. This is the one walk; `strings()` is a filter over it.
#[derive(Debug, Clone, PartialEq)]
pub struct GovernedScalar<'a> {
    pub path: String,
    pub value: &'a serde_json::Value,
}

impl GovernedInput {
    #[must_use]
    pub const fn tool_arguments(arguments: McpToolInput) -> Self {
        Self::ToolArguments { arguments }
    }

    #[must_use]
    pub fn prompt_parts(parts: impl IntoIterator<Item = (String, String)>) -> Self {
        Self::Prompt {
            parts: parts
                .into_iter()
                .map(|(path, value)| PromptPart { path, value })
                .collect(),
        }
    }

    #[must_use]
    pub fn prompt_text(text: String) -> Self {
        Self::Prompt {
            parts: vec![PromptPart {
                path: PROMPT_PATH.to_owned(),
                value: text,
            }],
        }
    }

    #[must_use]
    pub const fn location_kind(&self) -> &'static str {
        match self {
            Self::ToolArguments { .. } => "tool_input",
            Self::Prompt { .. } => "prompt",
        }
    }

    #[must_use]
    pub const fn arguments(&self) -> Option<&McpToolInput> {
        match self {
            Self::ToolArguments { arguments } => Some(arguments),
            Self::Prompt { .. } => None,
        }
    }

    #[must_use]
    pub fn strings(&self) -> Vec<GovernedString<'_>> {
        match self {
            Self::ToolArguments { .. } => self
                .scalars()
                .into_iter()
                .filter_map(|scalar| {
                    scalar.value.as_str().map(|value| GovernedString {
                        path: scalar.path,
                        value,
                    })
                })
                .collect(),
            Self::Prompt { parts } => parts
                .iter()
                .map(|part| GovernedString {
                    path: part.path.clone(),
                    value: &part.value,
                })
                .collect(),
        }
    }

    // Why: a prompt has no argument structure to address, so a condition that
    // names a field can never be satisfied by one. Returning empty rather than
    // the prompt's text keeps a path-addressed policy from matching a prompt on
    // a coincidence of naming.
    #[must_use]
    pub fn scalars(&self) -> Vec<GovernedScalar<'_>> {
        match self {
            Self::ToolArguments { arguments } => {
                let mut out = Vec::new();
                collect_scalars(arguments.as_value(), &mut String::new(), &mut out);
                out
            },
            Self::Prompt { .. } => Vec::new(),
        }
    }
}

const PROMPT_PATH: &str = "text";

fn collect_scalars<'a>(
    value: &'a serde_json::Value,
    path: &mut String,
    out: &mut Vec<GovernedScalar<'a>>,
) {
    match value {
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let parent = path.len();
                path.push_str(&format!("[{index}]"));
                collect_scalars(item, path, out);
                path.truncate(parent);
            }
        },
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                let parent = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(key);
                collect_scalars(item, path, out);
                path.truncate(parent);
            }
        },
        scalar => out.push(GovernedScalar {
            path: path.clone(),
            value: scalar,
        }),
    }
}
