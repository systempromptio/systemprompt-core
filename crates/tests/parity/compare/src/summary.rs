//! The `session` object of a trace, and the input normalisation both
//! adapters apply.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::transcript::{CallStatus, Message, Transcript};

pub(crate) fn session(transcript: &Transcript) -> Value {
    let mut messages = 0u32;
    let stored = transcript
        .session(&transcript.root)
        .map(|log| log.messages.as_slice())
        .unwrap_or_default();
    for message in stored
        .iter()
        .filter(|message| !transcript.hidden.contains(message_id(message)))
    {
        match message {
            Message::User { counted, .. } => messages += u32::from(*counted),
            Message::Assistant(reply) if !reply.summary => {
                messages += 1;
                if reply
                    .calls
                    .iter()
                    .any(|call| call.status != CallStatus::Unfinished)
                {
                    messages += 1;
                }
            },
            Message::Assistant(_) => {},
        }
    }
    let mut summary = json!({ "messages": messages, "state": "idle" });
    if let Some(todos) = &transcript.todos {
        let listed: Vec<Value> = todos
            .as_array()
            .into_iter()
            .flatten()
            .map(|todo| json!({ "content": todo.get("content"), "status": todo.get("status") }))
            .collect();
        summary["todos"] = Value::Array(listed);
    }
    summary
}

fn message_id(message: &Message) -> &str {
    match message {
        Message::User { id, .. } => id,
        Message::Assistant(reply) => &reply.id,
    }
}

/// Paths in a tool's input made relative to the workspace with forward
/// slashes, every other value unchanged.
pub fn relative_input(input: &Value, workspace: &str) -> Value {
    match input {
        Value::String(text) => {
            let prefix = format!("{}/", workspace.trim_end_matches('/'));
            text.strip_prefix(&prefix).map_or_else(
                || input.clone(),
                |rest| Value::String(rest.replace('\\', "/")),
            )
        },
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| relative_input(item, workspace))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), relative_input(value, workspace)))
                .collect(),
        ),
        _ => input.clone(),
    }
}

/// Session ids by parent, for adapters that need the child of a call.
pub fn children(transcript: &Transcript) -> BTreeMap<String, Vec<String>> {
    let mut by_parent: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, log) in &transcript.sessions {
        if let Some(parent) = &log.parent {
            by_parent
                .entry(parent.clone())
                .or_default()
                .push(id.clone());
        }
    }
    by_parent
}
