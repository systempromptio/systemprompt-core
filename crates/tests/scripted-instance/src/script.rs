//! The model script: an ordered list of turns, each matching the next
//! model request and producing one streamed response.
//!
//! The types deserialise from a scenario's `turns.json`. One script serves
//! both wire protocols; the encoder chooses the stream form from the route
//! that was called. Tool names are logical names, translated per product
//! outside this crate.
//!
//! Beside the turns a script holds asides: answers to the model calls a
//! product makes outside its steps, whose place among the turns is not
//! fixed.

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asides: Vec<Aside>,
}

impl ModelScript {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_turns(turns: impl IntoIterator<Item = Turn>) -> Self {
        Self {
            turns: turns.into_iter().collect(),
            asides: Vec::new(),
        }
    }

    pub fn with_aside(mut self, aside: Aside) -> Self {
        self.asides.push(aside);
        self
    }

    pub(crate) fn aside_for(&self, system: &str) -> Option<&Aside> {
        self.asides.iter().find(|aside| {
            !aside.system_contains.is_empty() && system.contains(&aside.system_contains)
        })
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ScriptedInstanceError> {
        file::read_json(path.as_ref())
    }

    pub fn from_json(json: &str) -> Result<Self, ScriptedInstanceError> {
        file::parse_json(json)
    }
}

/// A model call a product makes beside its steps, a session title for one.
///
/// A request whose system prompt contains `system_contains` is answered
/// with `respond` whenever it arrives. It consumes no turn, is recorded with
/// `purpose`, and is never counted as unscripted. An aside with an empty
/// `system_contains` recognises nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aside {
    pub purpose: String,
    pub system_contains: String,
    #[serde(default)]
    pub respond: Vec<Respond>,
}

impl Aside {
    pub const TITLE: &'static str = "title";

    pub fn title(system_contains: impl Into<String>, reply: impl Into<String>) -> Self {
        Self {
            purpose: Self::TITLE.to_owned(),
            system_contains: system_contains.into(),
            respond: vec![Respond::Text(reply.into())],
        }
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

/// How a turn's answer ends. `MaxTokens` and `ContentFilter` are the stops
/// a provider sends when the output limit is reached and when it refuses
/// (Messages `max_tokens` / `refusal`, Chat Completions `length` /
/// `content_filter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    End,
    ToolUse,
    MaxTokens,
    ContentFilter,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub input: u32,
    pub output: u32,
}

/// What a turn does in place of, or around, its answer.
///
/// `ContextOverflow` answers 400 with the body each provider sends when the
/// prompt does not fit the model's context window. `HeadersAfterMs` holds
/// the whole response, status line included, for that long and then answers
/// the turn; the turn is taken when the request arrives, so a client that
/// gives up first and retries reaches the next turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnFault {
    Status(u16),
    CloseAfterEvents(u32),
    StallMs(u64),
    MalformedEvent,
    ContextOverflow,
    HeadersAfterMs(u64),
}
