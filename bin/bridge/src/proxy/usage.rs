//! Usage-metrics tap over proxied response bodies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use bytes::Bytes;
use futures_util::Stream;
use hyper::body::Frame;
use serde::Deserialize;

use crate::activity::ActivityLog;
use crate::proxy::server::ProxyStats;

const JSON_BUFFER_LIMIT: usize = 256 * 1024;

pub fn is_messages_path(path: &str) -> bool {
    path.ends_with("/v1/messages") || path.ends_with("/messages")
}

pub fn wrap_response_stream<S>(
    content_type: &str,
    enabled: bool,
    stats: Arc<ProxyStats>,
    activity: ActivityLog,
    stream: S,
) -> impl Stream<Item = std::io::Result<Frame<Bytes>>> + Send + use<S>
where
    S: Stream<Item = std::io::Result<Frame<Bytes>>> + Send + 'static,
{
    use futures_util::{StreamExt, future};
    let tap = if enabled {
        UsageTap::for_content_type(content_type, Sink { stats, activity })
    } else {
        UsageTap::Disabled
    };
    stream.scan(TapGuard(Some(tap)), |guard, item| {
        if let (Ok(frame), Some(tap)) = (&item, guard.0.as_mut())
            && let Some(data) = frame.data_ref()
        {
            tap.observe(data);
        }
        future::ready(Some(item))
    })
}

struct TapGuard(Option<UsageTap>);

impl Drop for TapGuard {
    fn drop(&mut self) {
        if let Some(tap) = self.0.take() {
            tap.flush();
        }
    }
}

struct Sink {
    stats: Arc<ProxyStats>,
    activity: ActivityLog,
}

enum UsageTap {
    Disabled,
    Json {
        buf: Vec<u8>,
        sink: Sink,
    },
    Sse {
        carry: Vec<u8>,
        usage: StreamUsage,
        sink: Sink,
    },
}

#[derive(Default)]
struct StreamUsage {
    input_tokens: u64,
    output_tokens: u64,
    cache_read: u64,
    cache_write: u64,
    model: Option<String>,
}

impl UsageTap {
    fn for_content_type(content_type: &str, sink: Sink) -> Self {
        if content_type.contains("text/event-stream") {
            Self::Sse {
                carry: Vec::new(),
                usage: StreamUsage::default(),
                sink,
            }
        } else if content_type.contains("application/json") {
            Self::Json {
                buf: Vec::new(),
                sink,
            }
        } else {
            Self::Disabled
        }
    }

    fn observe(&mut self, chunk: &Bytes) {
        match self {
            Self::Disabled => {},
            Self::Json { buf, .. } => {
                if buf.len() < JSON_BUFFER_LIMIT {
                    buf.extend_from_slice(chunk);
                }
            },
            Self::Sse { carry, usage, .. } => {
                carry.extend_from_slice(chunk);
                consume_sse_lines(carry, usage);
            },
        }
    }

    fn flush(self) {
        match self {
            Self::Disabled => {},
            Self::Json { buf, sink } => {
                if let Ok(parsed) = serde_json::from_slice::<MessageResponse>(&buf) {
                    let usage = StreamUsage {
                        input_tokens: parsed.usage.input_tokens.unwrap_or(0),
                        output_tokens: parsed.usage.output_tokens.unwrap_or(0),
                        cache_read: parsed.usage.cache_read_input_tokens.unwrap_or(0),
                        cache_write: parsed.usage.cache_creation_input_tokens.unwrap_or(0),
                        model: parsed.model,
                    };
                    if usage.input_tokens > 0 || usage.output_tokens > 0 {
                        record_usage(&sink, &usage);
                    }
                }
            },
            Self::Sse { usage, sink, .. } => {
                if usage.input_tokens > 0 || usage.output_tokens > 0 {
                    record_usage(&sink, &usage);
                }
            },
        }
    }
}

#[derive(Deserialize)]
struct MessageResponse {
    usage: UsagePayload,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    message: Option<StreamMessage>,
    #[serde(default)]
    usage: Option<UsagePayload>,
}

#[derive(Deserialize)]
struct StreamMessage {
    usage: UsagePayload,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Default, Deserialize)]
#[expect(
    clippy::struct_field_names,
    reason = "these are Anthropic's own wire field names; renaming them would break the mapping"
)]
struct UsagePayload {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

fn consume_sse_lines(carry: &mut Vec<u8>, usage: &mut StreamUsage) {
    while let Some(newline) = carry.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = carry.drain(..=newline).collect();
        let Ok(text) = std::str::from_utf8(&line) else {
            continue;
        };
        let Some(payload) = text.trim().strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }
        let Ok(event) = serde_json::from_str::<StreamEvent>(payload) else {
            continue;
        };
        match event.event_type.as_str() {
            "message_start" => {
                if let Some(msg) = event.message {
                    apply_usage(usage, &msg.usage);
                    if msg.model.is_some() {
                        usage.model = msg.model;
                    }
                }
            },
            "message_delta" => {
                if let Some(delta) = event.usage {
                    apply_usage(usage, &delta);
                }
            },
            _ => {},
        }
    }
}

// Why: Anthropic reports usage cumulatively, so the last value wins rather than
// accumulating -- adding deltas would double-count every streamed message.
const fn apply_usage(usage: &mut StreamUsage, payload: &UsagePayload) {
    if let Some(v) = payload.input_tokens {
        usage.input_tokens = v;
    }
    if let Some(v) = payload.output_tokens {
        usage.output_tokens = v;
    }
    if let Some(v) = payload.cache_read_input_tokens {
        usage.cache_read = v;
    }
    if let Some(v) = payload.cache_creation_input_tokens {
        usage.cache_write = v;
    }
}

fn record_usage(sink: &Sink, usage: &StreamUsage) {
    let (input, output) = (usage.input_tokens, usage.output_tokens);
    sink.stats.messages_total.fetch_add(1, Ordering::Relaxed);
    if input > 0 {
        sink.stats
            .tokens_in_total
            .fetch_add(input, Ordering::Relaxed);
    }
    if output > 0 {
        sink.stats
            .tokens_out_total
            .fetch_add(output, Ordering::Relaxed);
    }
    let total = sink.stats.messages_total.load(Ordering::Relaxed);
    sink.activity.append(format!(
        "tokens: +{input} in / +{output} out (total {total} msgs)"
    ));
}
