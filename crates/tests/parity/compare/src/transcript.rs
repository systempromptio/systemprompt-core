//! A session as either product left it, in a form that names neither: the
//! sessions with their messages and tool calls, the model requests in arrival
//! order, the runner's actions and the permission replies.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallStatus {
    Ok,
    Error,
    DeniedByRule,
    DeniedByPolicy,
    Rejected,
    Aborted,
    Unfinished,
}

impl CallStatus {
    pub const fn result(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error | Self::Unfinished => "error",
            Self::DeniedByRule | Self::DeniedByPolicy => "denied",
            Self::Rejected => "rejected",
            Self::Aborted => "aborted",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub id: String,
    pub tool: String,
    pub input: Value,
    pub status: CallStatus,
    pub output: String,
    pub truncated: bool,
    pub feedback: bool,
    pub child: Option<String>,
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reply {
    pub id: String,
    pub text: String,
    pub calls: Vec<Call>,
    pub finish: Option<String>,
    pub error: Option<String>,
    pub summary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    User {
        id: String,
        counted: bool,
        compaction: Option<bool>,
    },
    Assistant(Reply),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionLog {
    pub parent: Option<String>,
    pub title: Option<String>,
    pub messages: Vec<Message>,
}

/// One model request as the scripted instance received it. `results` are
/// the tool-call ids the request carries, `status` is the error status it
/// was answered with, `has_tools` whether it advertised any tool.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelRequest {
    pub results: Vec<String>,
    pub status: Option<u16>,
    pub has_tools: bool,
    pub unscripted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    Steer,
    Prompt,
    Abort,
    Revert,
}

/// A runner action and the number of model requests received when it was
/// taken; `message` is the message number in `prompt.txt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub kind: MarkKind,
    pub requests_before: usize,
    pub message: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Transcript {
    pub root: String,
    pub sessions: BTreeMap<String, SessionLog>,
    pub requests: Vec<ModelRequest>,
    pub marks: Vec<Mark>,
    /// Call id to the reply the user gave its ask: `once`, `always`, `reject`.
    pub replies: BTreeMap<String, String>,
    pub todos: Option<Value>,
    /// Ids of messages a revert hides: traced, but not counted.
    pub hidden: BTreeSet<String>,
}

impl Transcript {
    pub fn session(&self, id: &str) -> Option<&SessionLog> {
        self.sessions.get(id)
    }
}

/// The tool-call ids a model request body carries, in either protocol.
pub fn carried_results(body: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    for message in body
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(id) = message.get("tool_call_id").and_then(Value::as_str) {
            ids.push(id.to_owned());
        }
        for block in message
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if block.get("type").and_then(Value::as_str) == Some("tool_result")
                && let Some(id) = block.get("tool_use_id").and_then(Value::as_str)
            {
                ids.push(id.to_owned());
            }
        }
    }
    ids
}

pub fn advertises_tools(body: &Value) -> bool {
    body.get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| !tools.is_empty())
}
