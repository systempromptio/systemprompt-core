//! The model script: an ordered list of turns, each matching the next
//! model request and producing one streamed response.
//!
//! The types deserialise from a scenario's `turns.json`. One script serves
//! both wire protocols; the encoder chooses the stream form from the route
//! that was called. Tool names are logical names, translated per product
//! outside this crate.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ScriptedInstanceError;
use crate::file;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelScript {
    #[serde(default)]
    pub turns: Vec<Turn>,
}

impl ModelScript {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_turns(turns: impl IntoIterator<Item = Turn>) -> Self {
        Self {
            turns: turns.into_iter().collect(),
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ScriptedInstanceError> {
        file::read_json(path.as_ref())
    }

    pub fn from_json(json: &str) -> Result<Self, ScriptedInstanceError> {
        file::parse_json(json)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    #[serde(default)]
    pub expect: Expect,
    #[serde(default)]
    pub respond: Vec<Respond>,
    #[serde(default)]
    pub stop: Option<StopReason>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub fault: Option<TurnFault>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    #[serde(default)]
    pub tools_include: Vec<String>,
    #[serde(default)]
    pub tool_results: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Respond {
    Text(String),
    ToolCall(ToolCall),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
    #[serde(default)]
    pub argument_chunks: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    End,
    ToolUse,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub input: u32,
    pub output: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnFault {
    Status(u16),
    CloseAfterEvents(u32),
    StallMs(u64),
    MalformedEvent,
}
