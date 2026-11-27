//! Decodes a Messages event stream into unified events. Content blocks are
//! tracked by index; a tool block's argument text is read leniently when the
//! block stops.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use serde_json::Value;

use crate::llm::error::{LlmError, stream_error};
use crate::llm::event::{Event, FinishReason, Usage};
use crate::llm::repair::{ToolInput, repair_tool_input};
use crate::llm::sse::SseFrame;

enum Block {
    Text,
    Thinking {
        signature: String,
    },
    Tool {
        id: String,
        name: String,
        arguments: String,
    },
    Ignored,
}

#[derive(Default)]
pub(crate) struct MessagesDecoder {
    blocks: HashMap<u64, Block>,
    input: Option<u64>,
    output: Option<u64>,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    stop_reason: Option<String>,
    complete: bool,
}

impl MessagesDecoder {
    pub(crate) const fn is_complete(&self) -> bool {
        self.complete
    }

    pub(crate) fn push(&mut self, frame: &SseFrame) -> Result<Vec<Event>, LlmError> {
        let data = frame.data.trim();
        if data.is_empty() || data == "[DONE]" {
            return Ok(Vec::new());
        }
        let value: Value = serde_json::from_str(data).map_err(LlmError::Decode)?;
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .or(frame.event.as_deref())
            .unwrap_or_default();
        let index = value.get("index").and_then(Value::as_u64).unwrap_or(0);
        let mut events = Vec::new();
        match kind {
            "message_start" => self.merge_usage(value.pointer("/message/usage")),
            "content_block_start" => {
                self.start_block(index, value.get("content_block"), &mut events);
            },
            "content_block_delta" => self.delta(index, value.get("delta"), &mut events),
            "content_block_stop" => self.stop_block(index, &mut events),
            "message_delta" => {
                if let Some(reason) = value.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    self.stop_reason = Some(reason.to_owned());
                }
                self.merge_usage(value.get("usage"));
            },
            "message_stop" => events.extend(self.final_events()),
            "error" => {
                let text = |pointer: &str| {
                    value
                        .pointer(pointer)
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                };
                return Err(stream_error(text("/error/type"), text("/error/message")));
            },
            _ => {},
        }
        Ok(events)
    }

    pub(crate) fn finish(&mut self) -> Result<Vec<Event>, LlmError> {
        if self.complete {
            return Ok(Vec::new());
        }
        if self.stop_reason.is_some() {
            return Ok(self.final_events());
        }
        Err(LlmError::Truncated)
    }

    // JSON: a Messages `content_block` object, read field by field.
    fn start_block(&mut self, index: u64, block: Option<&Value>, events: &mut Vec<Event>) {
        let Some(block) = block else { return };
        let text = |key: &str| {
            block
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let state = match block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "text" => {
                let initial = text("text");
                if !initial.is_empty() {
                    events.push(Event::TextDelta(initial));
                }
                Block::Text
            },
            "thinking" => {
                let initial = text("thinking");
                if !initial.is_empty() {
                    events.push(Event::ReasoningDelta {
                        text: initial,
                        signature: None,
                    });
                }
                Block::Thinking {
                    signature: text("signature"),
                }
            },
            "redacted_thinking" => {
                events.push(Event::RedactedReasoning { data: text("data") });
                Block::Ignored
            },
            "tool_use" => {
                let id = Some(text("id"))
                    .filter(|id| !id.is_empty())
                    .unwrap_or_else(|| format!("toolu_{index}"));
                let name = text("name");
                let arguments = match block.get("input") {
                    Some(Value::Object(map)) if !map.is_empty() => {
                        Value::Object(map.clone()).to_string()
                    },
                    _ => String::new(),
                };
                events.push(Event::ToolCallStart {
                    id: id.clone(),
                    name: name.clone(),
                });
                if !arguments.is_empty() {
                    let fragment = arguments.clone();
                    events.push(Event::ToolCallDelta {
                        id: id.clone(),
                        fragment,
                    });
                }
                Block::Tool {
                    id,
                    name,
                    arguments,
                }
            },
            _ => Block::Ignored,
        };
        self.blocks.insert(index, state);
    }

    // JSON: a Messages `delta` object, read field by field.
    fn delta(&mut self, index: u64, delta: Option<&Value>, events: &mut Vec<Event>) {
        let Some(delta) = delta else { return };
        let text = |key: &str| {
            delta
                .get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        };
        match delta
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "text_delta" => {
                if let Some(chunk) = text("text") {
                    events.push(Event::TextDelta(chunk.to_owned()));
                }
            },
            "thinking_delta" => {
                if let Some(chunk) = text("thinking") {
                    events.push(Event::ReasoningDelta {
                        text: chunk.to_owned(),
                        signature: None,
                    });
                }
            },
            "signature_delta" => {
                if let (Some(chunk), Some(Block::Thinking { signature })) =
                    (text("signature"), self.blocks.get_mut(&index))
                {
                    signature.push_str(chunk);
                }
            },
            "input_json_delta" => {
                if let (Some(chunk), Some(Block::Tool { id, arguments, .. })) =
                    (text("partial_json"), self.blocks.get_mut(&index))
                {
                    arguments.push_str(chunk);
                    let fragment = chunk.to_owned();
                    events.push(Event::ToolCallDelta {
                        id: id.clone(),
                        fragment,
                    });
                }
            },
            _ => {},
        }
    }

    fn stop_block(&mut self, index: u64, events: &mut Vec<Event>) {
        match self.blocks.remove(&index) {
            Some(Block::Tool {
                id,
                name,
                arguments,
            }) => {
                let ToolInput { value, malformed } = repair_tool_input(&arguments);
                events.push(Event::ToolCallEnd {
                    id,
                    name,
                    input: value,
                    malformed,
                });
            },
            Some(Block::Thinking { signature }) if !signature.is_empty() => {
                let signature = Some(signature);
                events.push(Event::ReasoningDelta {
                    text: String::new(),
                    signature,
                });
            },
            _ => {},
        }
    }

    // JSON: a Messages `usage` object, read field by field.
    fn merge_usage(&mut self, usage: Option<&Value>) {
        let Some(usage) = usage else { return };
        let count = |key: &str| usage.get(key).and_then(Value::as_u64);
        self.input = count("input_tokens").or(self.input);
        self.output = count("output_tokens").or(self.output);
        self.cache_read = count("cache_read_input_tokens").or(self.cache_read);
        self.cache_write = count("cache_creation_input_tokens").or(self.cache_write);
    }

    fn final_events(&mut self) -> Vec<Event> {
        self.complete = true;
        let mut events = Vec::with_capacity(2);
        if self.input.is_some() || self.output.is_some() {
            let cache_read = self.cache_read.unwrap_or(0);
            let cache_write = self.cache_write.unwrap_or(0);
            events.push(Event::Usage(Usage {
                input: self
                    .input
                    .unwrap_or(0)
                    .saturating_add(cache_read)
                    .saturating_add(cache_write),
                output: self.output.unwrap_or(0),
                reasoning: 0,
                cache_read,
                cache_write,
            }));
        }
        let reason = match self.stop_reason.as_deref() {
            Some("end_turn" | "stop_sequence" | "pause_turn") => FinishReason::Stop,
            Some("tool_use") => FinishReason::ToolCalls,
            Some("max_tokens" | "model_context_window_exceeded") => FinishReason::Length,
            Some("refusal") => FinishReason::ContentFilter,
            Some(other) => FinishReason::Other(other.to_owned()),
            None => FinishReason::Other("unknown".to_owned()),
        };
        events.push(Event::Finish { reason });
        events
    }
}
