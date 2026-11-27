//! Decodes a Chat Completions event stream into unified events. Tool calls
//! arrive as indexed fragments that may interleave; each is announced once its
//! name is known and closed, with its arguments read leniently, when the stream
//! finishes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::llm::error::{LlmError, stream_error};
use crate::llm::event::{Event, FinishReason, Usage};
use crate::llm::repair::{ToolInput, repair_tool_input};
use crate::llm::sse::SseFrame;

#[derive(Default)]
struct Slot {
    id: String,
    name: String,
    arguments: String,
    started: bool,
}

#[derive(Default)]
pub(crate) struct ChatDecoder {
    slots: BTreeMap<u64, Slot>,
    finish_reason: Option<String>,
    usage: Option<Usage>,
    done: bool,
    finished: bool,
}

impl ChatDecoder {
    pub(crate) const fn is_complete(&self) -> bool {
        self.done
    }

    pub(crate) fn push(&mut self, frame: &SseFrame) -> Result<Vec<Event>, LlmError> {
        let data = frame.data.trim();
        if data == "[DONE]" {
            self.done = true;
            return Ok(Vec::new());
        }
        if data.is_empty() {
            return Ok(Vec::new());
        }
        let value: Value = serde_json::from_str(data).map_err(LlmError::Decode)?;
        if let Some(error) = value.get("error").filter(|e| !e.is_null()) {
            let text = |key: &str| error.get(key).and_then(Value::as_str).map(str::to_owned);
            let message = text("message").or_else(|| error.as_str().map(str::to_owned));
            return Err(stream_error(text("code").or_else(|| text("type")), message));
        }
        if let Some(usage) = value.get("usage").filter(|u| u.is_object()) {
            self.usage = Some(read_usage(usage));
        }
        let mut events = Vec::new();
        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|c| c.first());
        let Some(choice) = choice else {
            return Ok(events);
        };
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.to_owned());
        }
        let Some(delta) = choice.get("delta") else {
            return Ok(events);
        };
        let text = |key: &str| {
            delta
                .get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        };
        if let Some(thought) = text("reasoning_content").or_else(|| text("reasoning")) {
            events.push(Event::ReasoningDelta {
                text: thought.to_owned(),
                signature: None,
            });
        }
        if let Some(chunk) = text("content") {
            events.push(Event::TextDelta(chunk.to_owned()));
        }
        for call in delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            self.tool_delta(call, &mut events);
        }
        Ok(events)
    }

    pub(crate) fn finish(&mut self) -> Result<Vec<Event>, LlmError> {
        if self.finished {
            return Ok(Vec::new());
        }
        if self.finish_reason.is_none() && !self.done {
            return Err(LlmError::Truncated);
        }
        self.finished = true;
        let mut events = Vec::new();
        let mut calls = 0usize;
        for (index, slot) in std::mem::take(&mut self.slots) {
            if slot.name.is_empty() {
                continue;
            }
            let id = if slot.id.is_empty() {
                format!("call_{index}")
            } else {
                slot.id
            };
            if !slot.started {
                events.push(Event::ToolCallStart {
                    id: id.clone(),
                    name: slot.name.clone(),
                });
            }
            let ToolInput { value, malformed } = repair_tool_input(&slot.arguments);
            events.push(Event::ToolCallEnd {
                id,
                name: slot.name,
                input: value,
                malformed,
            });
            calls += 1;
        }
        if let Some(usage) = self.usage {
            events.push(Event::Usage(usage));
        }
        let reason = match self.finish_reason.as_deref() {
            Some("tool_calls" | "function_call") => FinishReason::ToolCalls,
            Some("stop") | None if calls > 0 => FinishReason::ToolCalls,
            Some("stop") => FinishReason::Stop,
            Some("length" | "max_tokens") => FinishReason::Length,
            Some("content_filter") => FinishReason::ContentFilter,
            Some(other) => FinishReason::Other(other.to_owned()),
            None => FinishReason::Other("unknown".to_owned()),
        };
        events.push(Event::Finish { reason });
        Ok(events)
    }

    // JSON: one entry of a Chat Completions `delta.tool_calls` array, read field by
    // field.
    fn tool_delta(&mut self, call: &Value, events: &mut Vec<Event>) {
        let id = call
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        let index = call
            .get("index")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| self.unindexed_slot(id));
        let slot = self.slots.entry(index).or_default();
        if let Some(id) = id
            && slot.id.is_empty()
        {
            id.clone_into(&mut slot.id);
        }
        if let Some(name) = call.pointer("/function/name").and_then(Value::as_str)
            && !slot.started
        {
            slot.name.push_str(name);
        }
        if !slot.started && !slot.name.is_empty() {
            if slot.id.is_empty() {
                slot.id = format!("call_{index}");
            }
            slot.started = true;
            events.push(Event::ToolCallStart {
                id: slot.id.clone(),
                name: slot.name.clone(),
            });
            if !slot.arguments.is_empty() {
                let fragment = slot.arguments.clone();
                events.push(Event::ToolCallDelta {
                    id: slot.id.clone(),
                    fragment,
                });
            }
        }
        let fragment = call
            .pointer("/function/arguments")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !fragment.is_empty() {
            slot.arguments.push_str(fragment);
            if slot.started {
                let fragment = fragment.to_owned();
                events.push(Event::ToolCallDelta {
                    id: slot.id.clone(),
                    fragment,
                });
            }
        }
    }

    fn unindexed_slot(&self, id: Option<&str>) -> u64 {
        let last = self.slots.keys().next_back().copied();
        let Some(id) = id else {
            return last.unwrap_or(0);
        };
        self.slots
            .iter()
            .find(|(_, slot)| slot.id == id)
            .map_or_else(|| last.map_or(0, |last| last + 1), |(index, _)| *index)
    }
}

// JSON: a Chat Completions `usage` object, read field by field.
fn read_usage(usage: &Value) -> Usage {
    let count = |pointer: &str| usage.pointer(pointer).and_then(Value::as_u64);
    Usage {
        input: count("/prompt_tokens").unwrap_or(0),
        output: count("/completion_tokens").unwrap_or(0),
        reasoning: count("/completion_tokens_details/reasoning_tokens").unwrap_or(0),
        cache_read: count("/prompt_tokens_details/cached_tokens").unwrap_or(0),
        cache_write: count("/prompt_tokens_details/cache_write_tokens")
            .or_else(|| count("/prompt_tokens_details/cache_creation_input_tokens"))
            .unwrap_or(0),
    }
}
