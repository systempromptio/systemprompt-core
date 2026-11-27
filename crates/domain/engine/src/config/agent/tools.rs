//! Which tools an agent may see, and the shapes settings use to say so.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::FieldError;
use crate::permission::wildcard;

/// Which tools an agent may see.
///
/// A non-empty `allow` is exclusive; `deny` always wins. Names are wildcard
/// patterns compared without regard to case. Written as `"read, grep"` or
/// `["read", "grep"]` (allow lists) or `{"shell": false}` (where `true` only
/// lifts an earlier deny).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolFilter {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

impl ToolFilter {
    pub fn permits(&self, tool: &str) -> bool {
        let tool = tool.to_lowercase();
        let hit = |pattern: &String| wildcard(&pattern.to_lowercase(), &tool);
        if self.deny.iter().any(hit) {
            return false;
        }
        self.allow.is_empty() || self.allow.iter().any(hit)
    }

    pub const fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }

    // JSON: a tool filter as written in front matter or settings.
    pub fn from_value(value: &Value) -> Result<Self, FieldError> {
        let mut filter = Self::default();
        match value {
            Value::Null => {},
            Value::String(_) | Value::Array(_) => filter.allow = names(value)?,
            Value::Object(map) => {
                for (tool, enabled) in map {
                    match enabled {
                        Value::Bool(false) => filter.deny.push(tool.clone()),
                        Value::Bool(true) => filter.deny.retain(|denied| denied != tool),
                        _ => {
                            let expected = "true or false for each tool";
                            return Err(FieldError::Type { expected });
                        },
                    }
                }
            },
            _ => {
                return Err(FieldError::Type {
                    expected: "a list of tool names or an object of tool to boolean",
                });
            },
        }
        Ok(filter)
    }
}

// JSON: a comma-separated string or a list of tool names.
pub(super) fn names(value: &Value) -> Result<Vec<String>, FieldError> {
    match value {
        Value::String(text) => Ok(text
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect()),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::String(name) => Ok(name.trim().to_owned()),
                _ => Err(FieldError::Type {
                    expected: "a list of tool names",
                }),
            })
            .collect(),
        _ => Err(FieldError::Type {
            expected: "a list of tool names",
        }),
    }
}
