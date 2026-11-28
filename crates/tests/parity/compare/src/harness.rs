//! The harness's side: its headless event stream (one JSON object a line, as
//! `systemprompt_harness::command::RunEvent` writes them) and the model
//! requests the scripted instance recorded, read into a [`Transcript`].
//!
//! The harness's tool names and input fields are the logical ones, so its table
//! is the identity; paths in tool input are made relative to the workspace.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::script::ScenarioScript;
use crate::summary::relative_input;
use crate::transcript::{
    Call, CallStatus, Mark, Message, ModelRequest, Reply, SessionLog, Transcript, advertises_tools,
    carried_results,
};

/// One model request body as recorded, and whether the instance refused it as
/// unscripted.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub body: Value,
    pub unscripted: bool,
}

#[derive(Debug, Default)]
struct Collected {
    root: String,
    sessions: BTreeMap<String, (Option<String>, Option<String>)>,
    reverted: BTreeMap<String, String>,
    messages: Vec<Value>,
    parts: Vec<Value>,
    asks: BTreeMap<String, String>,
    replies: BTreeMap<String, String>,
    todos: Option<Value>,
}

fn text_of<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    value.pointer(pointer).and_then(Value::as_str)
}

fn collect(events: &[Value]) -> Collected {
    let mut seen = Collected::default();
    for event in events {
        match text_of(event, "/type").unwrap_or_default() {
            "started" => seen.root = text_of(event, "/session_id").unwrap_or_default().to_owned(),
            "session" => {
                let id = text_of(event, "/session/id").unwrap_or_default().to_owned();
                let parent = text_of(event, "/session/parent_id").map(str::to_owned);
                let title = text_of(event, "/session/title")
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned);
                match text_of(event, "/session/revert/message_id") {
                    Some(from) => seen.reverted.insert(id.clone(), from.to_owned()),
                    None => seen.reverted.remove(&id),
                };
                seen.sessions.insert(id, (parent, title));
            },
            "message" => upsert(&mut seen.messages, &event["message"]),
            "message_removed" => {
                let id = text_of(event, "/message_id").unwrap_or_default();
                seen.messages
                    .retain(|message| text_of(message, "/id") != Some(id));
                seen.parts
                    .retain(|part| text_of(part, "/message_id") != Some(id));
            },
            "part" => upsert(&mut seen.parts, &event["part"]),
            "part_removed" => {
                let id = text_of(event, "/part_id").unwrap_or_default();
                seen.parts.retain(|part| text_of(part, "/id") != Some(id));
            },
            "todos" if text_of(event, "/session_id") == Some(seen.root.as_str()) => {
                seen.todos = event.get("todos").cloned();
            },
            "permission_asked" => {
                let ask = text_of(event, "/ask/id").unwrap_or_default().to_owned();
                let call = text_of(event, "/ask/call_id")
                    .unwrap_or_default()
                    .to_owned();
                seen.asks.insert(ask, call);
            },
            "permission_replied" => {
                let ask = text_of(event, "/id").unwrap_or_default();
                if let Some(call) = seen.asks.get(ask) {
                    let reply = text_of(event, "/reply/type").unwrap_or_default().to_owned();
                    seen.replies.insert(call.clone(), reply);
                }
            },
            _ => {},
        }
    }
    seen
}

fn upsert(list: &mut Vec<Value>, item: &Value) {
    let id = item.get("id").cloned();
    match list.iter_mut().find(|known| known.get("id") == id.as_ref()) {
        Some(known) => *known = item.clone(),
        None => list.push(item.clone()),
    }
}

fn call(part: &Value, workspace: &str) -> Option<Call> {
    let body = part.get("body")?;
    if text_of(body, "/type") != Some("tool") {
        return None;
    }
    let state = body.get("state")?;
    let metadata = state.get("metadata");
    let denied = metadata
        .and_then(|meta| meta.get("denied"))
        .and_then(Value::as_str);
    let (status, output) = match text_of(state, "/status").unwrap_or_default() {
        "completed" => (
            CallStatus::Ok,
            text_of(state, "/output").unwrap_or_default(),
        ),
        "error" => (
            match denied {
                Some("policy") => CallStatus::DeniedByPolicy,
                Some("rule") => CallStatus::DeniedByRule,
                Some("rejected") => CallStatus::Rejected,
                Some("aborted") => CallStatus::Aborted,
                _ => CallStatus::Error,
            },
            text_of(state, "/error").unwrap_or_default(),
        ),
        _ => (CallStatus::Unfinished, ""),
    };
    let input = state.get("input").cloned().unwrap_or(Value::Null);
    Some(Call {
        id: text_of(body, "/call_id").unwrap_or_default().to_owned(),
        tool: text_of(body, "/tool").unwrap_or_default().to_owned(),
        agent: input
            .get("agent")
            .and_then(Value::as_str)
            .map(str::to_owned),
        input: relative_input(&input, workspace),
        status,
        output: output.to_owned(),
        truncated: metadata
            .and_then(|meta| meta.get("truncated"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        feedback: metadata.and_then(|meta| meta.get("feedback")).is_some(),
        child: None,
    })
}

/// The transcript of one harness run.
pub fn transcript(
    events: &[Value],
    requests: &[RecordedRequest],
    marks: Vec<Mark>,
    workspace: &str,
    script: &ScenarioScript,
) -> Transcript {
    let seen = collect(events);
    let hidden = hidden_by_revert(&seen);
    let mut sessions: BTreeMap<String, SessionLog> = seen
        .sessions
        .iter()
        .map(|(id, (parent, title))| {
            let log = SessionLog {
                parent: parent.clone(),
                title: title.clone(),
                messages: Vec::new(),
            };
            (id.clone(), log)
        })
        .collect();
    let mut compaction_users: BTreeMap<String, bool> = BTreeMap::new();
    for message in &seen.messages {
        let id = text_of(message, "/id").unwrap_or_default().to_owned();
        let session = text_of(message, "/session_id")
            .unwrap_or_default()
            .to_owned();
        let parts: Vec<&Value> = seen
            .parts
            .iter()
            .filter(|part| text_of(part, "/message_id") == Some(id.as_str()))
            .collect();
        let entry = if text_of(message, "/role") == Some("user") {
            let compaction = parts
                .iter()
                .find(|part| text_of(part, "/body/type") == Some("compaction"))
                .map(|part| {
                    part.pointer("/body/auto")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                });
            let synthetic = !parts.is_empty()
                && parts.iter().all(|part| {
                    part.pointer("/body/synthetic").and_then(Value::as_bool) == Some(true)
                });
            let system = message
                .get("system")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if let Some(auto) = compaction {
                compaction_users.insert(id.clone(), auto);
            }
            Message::User {
                id,
                counted: compaction.is_none() && !synthetic && !system,
                compaction,
            }
        } else {
            let text: String = parts
                .iter()
                .filter(|part| text_of(part, "/body/type") == Some("text"))
                .filter_map(|part| text_of(part, "/body/text"))
                .collect();
            let parent = text_of(message, "/parent_id").unwrap_or_default();
            Message::Assistant(Reply {
                id,
                text,
                calls: parts
                    .iter()
                    .filter_map(|part| call(part, workspace))
                    .collect(),
                finish: text_of(message, "/finish").map(str::to_owned),
                error: text_of(message, "/error/kind").map(str::to_owned),
                summary: compaction_users.contains_key(parent),
            })
        };
        sessions.entry(session).or_default().messages.push(entry);
    }
    link_children(&mut sessions);
    let mut turns = script.turns.iter();
    let requests = requests
        .iter()
        .map(|recorded| ModelRequest {
            results: carried_results(&recorded.body),
            status: if recorded.unscripted {
                None
            } else {
                turns.next().and_then(|turn| turn.status)
            },
            has_tools: advertises_tools(&recorded.body),
            unscripted: recorded.unscripted,
        })
        .collect();
    Transcript {
        root: seen.root,
        sessions,
        requests,
        marks,
        replies: seen.replies,
        todos: seen.todos,
        hidden,
    }
}

/// The messages a pending revert hides: from the message it names onward in
/// its session.
fn hidden_by_revert(seen: &Collected) -> BTreeSet<String> {
    let mut gone = BTreeSet::new();
    for (session, from) in &seen.reverted {
        let mut hidden = false;
        for message in &seen.messages {
            if text_of(message, "/session_id") != Some(session.as_str()) {
                continue;
            }
            hidden |= text_of(message, "/id") == Some(from.as_str());
            if hidden {
                gone.insert(text_of(message, "/id").unwrap_or_default().to_owned());
            }
        }
    }
    gone
}

/// Each child session is the child of the parent's task calls in order.
fn link_children(sessions: &mut BTreeMap<String, SessionLog>) {
    let ids: Vec<(String, String)> = sessions
        .iter()
        .filter_map(|(id, log)| log.parent.clone().map(|parent| (parent, id.clone())))
        .collect();
    for (parent, child) in ids {
        let Some(log) = sessions.get_mut(&parent) else {
            continue;
        };
        let open = log.messages.iter_mut().find_map(|message| match message {
            Message::Assistant(reply) => reply
                .calls
                .iter_mut()
                .find(|call| call.tool == "task" && call.child.is_none()),
            Message::User { .. } => None,
        });
        if let Some(call) = open {
            call.child = Some(child);
        }
    }
}
