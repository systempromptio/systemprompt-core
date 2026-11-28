//! The reference agent's side: the reference runner's export of one run
//! (`crates/tests/parity/reference/README.md`, "Output layout") read into a
//! [`Transcript`], with tool names and input fields mapped back to the
//! logical ones through the runner's `tools.json`.
//!
//! Written against the layout as that README states it; fields the README
//! does not pin (the runner's action records) are read where present.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Context as _;
use serde_json::{Map, Value};

use crate::summary::relative_input;
use crate::transcript::{
    Call, CallStatus, Mark, MarkKind, Message, ModelRequest, Reply, SessionLog, Transcript,
    advertises_tools, carried_results,
};

/// The reference table read backwards: the agent's tool name to the logical
/// one, and per logical tool the agent's field name to the logical field.
#[derive(Debug, Clone, Default)]
pub struct Table {
    tools: BTreeMap<String, (String, BTreeMap<String, String>)>,
    dropped: BTreeMap<String, Vec<String>>,
}

impl Table {
    pub fn parse(tools_json: &str) -> anyhow::Result<Self> {
        let document: Value = serde_json::from_str(tools_json)?;
        let mut tools = BTreeMap::new();
        let mut dropped = BTreeMap::new();
        for (logical, entry) in document
            .get("tools")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(logical)
                .to_owned();
            let fields = entry
                .get("input")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(field, theirs)| {
                    theirs
                        .as_str()
                        .map(|theirs| (theirs.to_owned(), field.clone()))
                })
                .collect();
            let unmapped: Vec<String> = entry
                .get("input")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter(|(_, theirs)| theirs.is_null())
                .map(|(field, _)| field.clone())
                .collect();
            dropped.insert(logical.clone(), unmapped);
            tools.insert(name, (logical.clone(), fields));
        }
        Ok(Self { tools, dropped })
    }

    /// The logical input fields of `logical` the reference tool does not
    /// take: the table drops them, so a comparison leaves them out of the
    /// harness's input too.
    pub fn dropped(&self, logical: &str) -> &[String] {
        self.dropped.get(logical).map_or(&[], Vec::as_slice)
    }

    fn logical(&self, name: &str, input: &Value) -> (String, Value) {
        let Some((logical, fields)) = self.tools.get(name) else {
            return (name.to_owned(), input.clone());
        };
        let mapped: Map<String, Value> = input
            .as_object()
            .into_iter()
            .flatten()
            .map(|(field, value)| {
                (
                    fields.get(field).cloned().unwrap_or_else(|| field.clone()),
                    value.clone(),
                )
            })
            .collect();
        (logical.clone(), Value::Object(mapped))
    }
}

fn read_json(path: &Path) -> anyhow::Result<Value> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

fn content_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect(),
        Some(Value::Object(fields)) => fields
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    }
}

fn session(export: &Value, table: &Table, workspace: &str) -> SessionLog {
    let mut log = SessionLog::default();
    for (at, message) in export
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let id = message
            .get("id")
            .and_then(Value::as_str)
            .map_or_else(|| at.to_string(), str::to_owned);
        match message.get("type").and_then(Value::as_str) {
            Some("user") => log.messages.push(Message::User {
                id,
                counted: true,
                compaction: None,
            }),
            Some("assistant") => {
                let mut reply = Reply {
                    id,
                    ..Reply::default()
                };
                for item in message
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    match item.get("type").and_then(Value::as_str) {
                        Some("text") => reply
                            .text
                            .push_str(item.get("text").and_then(Value::as_str).unwrap_or_default()),
                        Some("tool") => reply.calls.push(call(item, table, workspace)),
                        _ => {},
                    }
                }
                reply.finish = message
                    .get("finish")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                log.messages.push(Message::Assistant(reply));
            },
            Some("compaction") => {
                let auto = message
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_none_or(|reason| reason == "auto");
                log.messages.push(Message::User {
                    id: format!("{id}/request"),
                    counted: false,
                    compaction: Some(auto),
                });
                log.messages.push(Message::Assistant(Reply {
                    id,
                    text: message
                        .get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    finish: Some("stop".to_owned()),
                    summary: true,
                    ..Reply::default()
                }));
            },
            _ => {},
        }
    }
    log
}

fn call(item: &Value, table: &Table, workspace: &str) -> Call {
    let name = item.get("name").and_then(Value::as_str).unwrap_or_default();
    let state = item.get("state").cloned().unwrap_or(Value::Null);
    let (tool, input) = table.logical(name, state.get("input").unwrap_or(&Value::Null));
    let (status, output) = match state.get("status").and_then(Value::as_str) {
        Some("completed") => (CallStatus::Ok, content_text(state.get("content"))),
        Some("error") => (
            // A refusal with no ask behind it is a rule's; one the user
            // answered is turned into a rejection once the asks are read.
            if state.pointer("/error/type").and_then(Value::as_str) == Some("permission.rejected") {
                CallStatus::DeniedByRule
            } else {
                CallStatus::Error
            },
            content_text(state.get("error").or_else(|| state.get("content"))),
        ),
        _ => (CallStatus::Unfinished, String::new()),
    };
    Call {
        id: item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        agent: input
            .get("agent")
            .and_then(Value::as_str)
            .map(str::to_owned),
        input: relative_input(&input, workspace),
        tool,
        status,
        output,
        truncated: state
            .pointer("/metadata/truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        feedback: false,
        child: None,
    }
}

/// The number of model requests the agent made before the `number`th user
/// message of `export`: one per assistant reply and per compaction stored
/// ahead of it. The runner's action records carry no request count, so a
/// prompt or a steer is placed by where the agent stored the message.
fn requests_before_message(export: &Value, number: u32) -> Option<usize> {
    let mut users = 0u32;
    let mut replies = 0usize;
    for message in export
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match message.get("type").and_then(Value::as_str) {
            Some("user") => {
                users += 1;
                if users == number {
                    return Some(replies);
                }
            },
            Some("assistant" | "compaction") => replies += 1,
            _ => {},
        }
    }
    None
}

fn mark(action: &Value, requests: usize, export: &Value) -> Option<Mark> {
    let kind = match action.get("do").and_then(Value::as_str)? {
        "steer" => MarkKind::Steer,
        "prompt" => MarkKind::Prompt,
        "abort" => MarkKind::Abort,
        "revert" => MarkKind::Revert,
        _ => return None,
    };
    let message = action
        .get("message")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok());
    let stored = match kind {
        MarkKind::Steer | MarkKind::Prompt => {
            message.and_then(|number| requests_before_message(export, number))
        },
        MarkKind::Abort | MarkKind::Revert => None,
    };
    let before = action
        .get("requests_before")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .or(stored)
        .unwrap_or(requests);
    Some(Mark {
        kind,
        requests_before: before,
        message,
    })
}

/// The transcript of one exported reference run in `dir`
/// (`<out>/<scenario>/<protocol>`).
pub fn transcript(dir: &Path, table: &Table) -> anyhow::Result<Transcript> {
    let workspace = dir.join("workspace").to_string_lossy().into_owned();
    let last = read_json(&dir.join("session.json"))?;
    // After a revert the agent's export no longer holds the reverted
    // messages; the runner keeps the export taken just before it, and the
    // messages missing from the last one are the ones the revert hid.
    let before_revert = dir.join("session.before-revert.json");
    let export = if before_revert.is_file() {
        read_json(&before_revert)?
    } else {
        last.clone()
    };
    let kept = message_ids(&last);
    let hidden: BTreeSet<String> = message_ids(&export)
        .into_iter()
        .filter(|id| !kept.contains(id))
        .collect();
    let root = export
        .pointer("/info/id")
        .and_then(Value::as_str)
        .unwrap_or("root")
        .to_owned();
    let mut sessions = BTreeMap::new();
    let mut root_log = session(&export, table, &workspace);
    let children_dir = dir.join("children");
    let mut children = Vec::new();
    if children_dir.is_dir() {
        let mut files: Vec<_> = std::fs::read_dir(&children_dir)?.collect::<Result<_, _>>()?;
        files.sort_by_key(std::fs::DirEntry::file_name);
        for entry in files {
            let child = read_json(&entry.path())?;
            let id = child
                .pointer("/info/id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let mut log = session(&child, table, &workspace);
            log.parent = Some(root.clone());
            children.push(id.clone());
            sessions.insert(id, log);
        }
    }
    let mut pending = children.into_iter();
    for message in &mut root_log.messages {
        if let Message::Assistant(reply) = message {
            for call in reply.calls.iter_mut().filter(|call| call.tool == "task") {
                call.child = pending.next();
            }
        }
    }
    let todos = root_log
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::Assistant(reply) => Some(reply),
            Message::User { .. } => None,
        })
        .flat_map(|reply| &reply.calls)
        .filter(|call| call.tool == "todo" && call.status == CallStatus::Ok)
        .filter_map(|call| call.input.get("todos").cloned())
        .next_back();
    sessions.insert(root.clone(), root_log);
    let mut requests = Vec::new();
    let lines = std::fs::read_to_string(dir.join("requests.jsonl")).unwrap_or_default();
    for line in lines.lines().filter(|line| !line.trim().is_empty()) {
        let record: Value = serde_json::from_str(line)?;
        if record.get("title_call").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let body = record.get("body").cloned().unwrap_or(Value::Null);
        let status = record
            .get("status")
            .and_then(Value::as_u64)
            .filter(|status| *status >= 400)
            .and_then(|status| u16::try_from(status).ok());
        let unscripted = record.get("unscripted").and_then(Value::as_bool) == Some(true);
        requests.push(ModelRequest {
            results: carried_results(&body),
            status: if unscripted { None } else { status },
            has_tools: advertises_tools(&body),
            unscripted,
        });
    }
    let run = read_json(&dir.join("run.json")).unwrap_or(Value::Null);
    let marks = run
        .get("actions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|action| mark(action, requests.len(), &export))
        .collect();
    let mut replies = BTreeMap::new();
    let mut with_feedback = BTreeSet::new();
    for ask in run
        .get("asks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(call) = ask
            .get("call")
            .or_else(|| ask.get("callID"))
            .or_else(|| ask.pointer("/request/source/id"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let Some(reply) = ask.get("reply").and_then(|reply| {
            reply
                .as_str()
                .or_else(|| reply.get("decision").and_then(Value::as_str))
        }) else {
            continue;
        };
        if ask
            .pointer("/reply/message")
            .and_then(Value::as_str)
            .is_some_and(|message| !message.trim().is_empty())
        {
            with_feedback.insert(call.to_owned());
        }
        replies.insert(call.to_owned(), reply.to_owned());
    }
    for log in sessions.values_mut() {
        for message in &mut log.messages {
            let Message::Assistant(reply) = message else {
                continue;
            };
            for call in &mut reply.calls {
                if replies.get(&call.id).map(String::as_str) == Some("reject")
                    && matches!(call.status, CallStatus::Error | CallStatus::DeniedByRule)
                {
                    call.status = CallStatus::Rejected;
                    call.feedback = with_feedback.contains(&call.id);
                }
            }
        }
    }
    Ok(Transcript {
        root,
        sessions,
        requests,
        marks,
        replies,
        todos,
        hidden,
    })
}

fn message_ids(export: &Value) -> BTreeSet<String> {
    export
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|message| message.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}
