//! Mapping stored messages to the model's history: parts become content blocks,
//! tool calls are followed by their results, and the latest compaction replaces
//! everything before it with its summary and verbatim tail. Reasoning is sent
//! back when it holds text or a signature; a failed reply is sent back only up
//! to its last tool call that ran.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use crate::llm::{Attachment, Content, Message};
use crate::prompt::compaction;
use crate::store::{MessageErrorKind, MessageWithParts, Part, PartBody, ToolState};
use serde_json::Value;
use systemprompt_identifiers::engine::{CallId, MessageId};

use super::attachments::parse_data_url;
use super::meta::CompactionMark;
use super::user_content::user_content;

const INTERRUPTED: &str = "The tool call was interrupted before it finished.";

pub(crate) fn build(
    all: &[MessageWithParts],
    mark: Option<&CompactionMark>,
    steered: &HashSet<MessageId>,
) -> Vec<Message> {
    let Some(at) = boundary(all) else {
        return map_messages(all, steered);
    };
    let mut out = vec![Message::user_text(compaction::summary_message(&text_of(
        &all[at + 1],
    )))];
    if let Some(start) = tail_start(all, at, mark) {
        out.extend(map_messages(&all[start..at], steered));
    }
    out.extend(map_messages(&all[at + 2..], steered));
    merge(out)
}

pub(crate) fn live<'a>(
    all: &'a [MessageWithParts],
    mark: Option<&CompactionMark>,
) -> (Option<String>, Vec<&'a MessageWithParts>) {
    let Some(at) = boundary(all) else {
        return (None, all.iter().collect());
    };
    let mut live: Vec<&MessageWithParts> = Vec::new();
    if let Some(start) = tail_start(all, at, mark) {
        live.extend(all[start..at].iter());
    }
    live.extend(all[at + 2..].iter());
    (Some(text_of(&all[at + 1])), live)
}

fn boundary(all: &[MessageWithParts]) -> Option<usize> {
    (0..all.len()).rev().find(|&index| {
        is_compaction_request(&all[index]) && all.get(index + 1).is_some_and(is_summary)
    })
}

fn tail_start(all: &[MessageWithParts], at: usize, mark: Option<&CompactionMark>) -> Option<usize> {
    let mark = mark.filter(|mark| &mark.message == all[at].message.id())?;
    let tail = mark.tail.as_ref()?;
    all[..at]
        .iter()
        .position(|message| message.message.id() == tail)
}

fn is_compaction_request(message: &MessageWithParts) -> bool {
    message.message.as_user().is_some()
        && message
            .parts
            .iter()
            .any(|part| matches!(part.body, PartBody::Compaction { .. }))
}

fn is_summary(message: &MessageWithParts) -> bool {
    message
        .message
        .as_assistant()
        .is_some_and(|assistant| assistant.system && assistant.error.is_none())
        && !text_of(message).trim().is_empty()
}

pub(crate) fn text_of(message: &MessageWithParts) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match &part.body {
            PartBody::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn map_messages(
    messages: &[MessageWithParts],
    steered: &HashSet<MessageId>,
) -> Vec<Message> {
    let mut out = Vec::with_capacity(messages.len());
    for message in messages {
        match &message.message {
            crate::store::Message::User(user) => {
                if is_compaction_request(message) {
                    continue;
                }
                let content = user_content(message, steered.contains(&user.id));
                if !content.is_empty() {
                    out.push(Message::user(content));
                }
            },
            crate::store::Message::Assistant(assistant) => {
                if assistant.system {
                    continue;
                }
                let failed = assistant
                    .error
                    .as_ref()
                    .is_some_and(|error| error.kind != MessageErrorKind::Aborted);
                let parts = if failed {
                    replayable(&message.parts)
                } else {
                    &message.parts
                };
                let (content, results) = assistant_content(parts);
                if !content.is_empty() {
                    out.push(Message::assistant(content));
                }
                if !results.is_empty() {
                    out.push(Message::user(results));
                }
            },
        }
    }
    merge(out)
}

fn replayable(parts: &[Part]) -> &[Part] {
    let ran = parts.iter().rposition(|part| {
        matches!(
            &part.body,
            PartBody::Tool { state, .. }
                if matches!(**state, ToolState::Completed { .. } | ToolState::Error { .. })
        )
    });
    ran.map_or(&[], |last| &parts[..=last])
}

fn assistant_content(parts: &[Part]) -> (Vec<Content>, Vec<Content>) {
    let mut content = Vec::new();
    let mut results = Vec::new();
    for part in parts {
        match &part.body {
            PartBody::Text { text, .. } if !text.trim().is_empty() => {
                content.push(Content::Text { text: text.clone() });
            },
            PartBody::Reasoning {
                text,
                provider_data,
            } => {
                if let Some(block) = reasoning(text, provider_data.as_ref()) {
                    content.push(block);
                }
            },
            PartBody::Tool {
                call_id,
                tool,
                state,
            } => {
                content.push(Content::ToolCall {
                    id: call_id.to_string(),
                    name: tool.clone(),
                    input: state.input().clone(),
                });
                results.push(tool_result(call_id, state));
            },
            _ => {},
        }
    }
    (content, results)
}

// JSON: provider replay data stored with a reasoning part, sent back verbatim.
fn reasoning(text: &str, provider_data: Option<&Value>) -> Option<Content> {
    if let Some(data) = replay_field(provider_data, "redacted") {
        return Some(Content::RedactedReasoning {
            data: data.to_owned(),
        });
    }
    let signature = replay_field(provider_data, "signature").map(str::to_owned);
    if text.trim().is_empty() && signature.is_none() {
        return None;
    }
    Some(Content::Reasoning {
        text: text.to_owned(),
        signature,
    })
}

// JSON: provider replay data stored with a reasoning part, sent back verbatim.
fn replay_field<'a>(provider_data: Option<&'a Value>, key: &str) -> Option<&'a str> {
    provider_data
        .and_then(|data| data.get(key))
        .and_then(Value::as_str)
}

fn tool_result(call_id: &CallId, state: &ToolState) -> Content {
    let id = call_id.to_string();
    match state {
        ToolState::Completed {
            output,
            attachments,
            ..
        } => Content::ToolResult {
            id,
            output: output.clone(),
            is_error: false,
            attachments: attachments
                .iter()
                .filter_map(|attachment| {
                    let (mime, data) = parse_data_url(&attachment.url)?;
                    let mime = if attachment.mime.is_empty() {
                        mime
                    } else {
                        attachment.mime.clone()
                    };
                    Some(Attachment {
                        mime,
                        data,
                        name: attachment.name.clone(),
                    })
                })
                .collect(),
        },
        ToolState::Error { error, .. } => {
            let output = error.clone();
            Content::ToolResult {
                id,
                output,
                is_error: true,
                attachments: Vec::new(),
            }
        },
        ToolState::Pending { .. } | ToolState::Running { .. } => Content::ToolResult {
            id,
            output: INTERRUPTED.to_owned(),
            is_error: true,
            attachments: Vec::new(),
        },
    }
}

fn merge(messages: Vec<Message>) -> Vec<Message> {
    let mut out: Vec<Message> = Vec::with_capacity(messages.len());
    for message in messages {
        match out.last_mut() {
            Some(last) if last.role == message.role => last.content.extend(message.content),
            _ => out.push(message),
        }
    }
    out
}
