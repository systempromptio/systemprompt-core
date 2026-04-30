use std::sync::Arc;
use std::sync::atomic::Ordering;

use bytes::Bytes;
use futures_util::Stream;
use hyper::body::Frame;
use serde::Deserialize;

use crate::proxy::server::ProxyStats;

const JSON_BUFFER_LIMIT: usize = 256 * 1024;

pub fn is_messages_path(path: &str) -> bool {
    path.ends_with("/v1/messages") || path.ends_with("/messages")
}

pub fn wrap_response_stream<S>(
    content_type: &str,
    enabled: bool,
    stats: Arc<ProxyStats>,
    stream: S,
) -> impl Stream<Item = std::io::Result<Frame<Bytes>>> + Send + use<S>
where
    S: Stream<Item = std::io::Result<Frame<Bytes>>> + Send + 'static,
{
    use futures_util::{StreamExt, future};
    let tap = if enabled {
        UsageTap::for_content_type(content_type, stats)
    } else {
        UsageTap::Disabled
    };
    stream.scan(TapGuard(Some(tap)), |guard, item| {
        if let (Ok(frame), Some(tap)) = (&item, guard.0.as_mut())
            && let Some(data) = frame.data_ref() {
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

enum UsageTap {
    Disabled,
    Json {
        buf: Vec<u8>,
        stats: Arc<ProxyStats>,
    },
    Sse {
        carry: Vec<u8>,
        input_tokens: u64,
        output_tokens: u64,
        stats: Arc<ProxyStats>,
    },
}

impl UsageTap {
    fn for_content_type(content_type: &str, stats: Arc<ProxyStats>) -> Self {
        if content_type.contains("text/event-stream") {
            Self::Sse {
                carry: Vec::new(),
                input_tokens: 0,
                output_tokens: 0,
                stats,
            }
        } else if content_type.contains("application/json") {
            Self::Json {
                buf: Vec::new(),
                stats,
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
            Self::Sse {
                carry,
                input_tokens,
                output_tokens,
                ..
            } => {
                carry.extend_from_slice(chunk);
                consume_sse_lines(carry, input_tokens, output_tokens);
            },
        }
    }

    fn flush(self) {
        match self {
            Self::Disabled => {},
            Self::Json { buf, stats } => {
                if let Ok(parsed) = serde_json::from_slice::<MessageResponse>(&buf) {
                    let input = parsed.usage.input_tokens.unwrap_or(0);
                    let output = parsed.usage.output_tokens.unwrap_or(0);
                    if input > 0 || output > 0 {
                        record_usage(&stats, input, output);
                    }
                }
            },
            Self::Sse {
                input_tokens,
                output_tokens,
                stats,
                ..
            } => {
                if input_tokens > 0 || output_tokens > 0 {
                    record_usage(&stats, input_tokens, output_tokens);
                }
            },
        }
    }
}

#[derive(Deserialize)]
struct MessageResponse {
    usage: UsagePayload,
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
}

#[derive(Default, Deserialize)]
struct UsagePayload {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
}

fn consume_sse_lines(carry: &mut Vec<u8>, input: &mut u64, output: &mut u64) {
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
                    if let Some(v) = msg.usage.input_tokens {
                        *input = v;
                    }
                    if let Some(v) = msg.usage.output_tokens {
                        *output = v;
                    }
                }
            },
            "message_delta" => {
                if let Some(usage) = event.usage {
                    if let Some(v) = usage.input_tokens {
                        *input = v;
                    }
                    if let Some(v) = usage.output_tokens {
                        *output = v;
                    }
                }
            },
            _ => {},
        }
    }
}

fn record_usage(stats: &ProxyStats, input: u64, output: u64) {
    stats.messages_total.fetch_add(1, Ordering::Relaxed);
    if input > 0 {
        stats.tokens_in_total.fetch_add(input, Ordering::Relaxed);
    }
    if output > 0 {
        stats.tokens_out_total.fetch_add(output, Ordering::Relaxed);
    }
    let total = stats.messages_total.load(Ordering::Relaxed);
    crate::activity::activity_log().append(format!(
        "tokens: +{input} in / +{output} out (total {total} msgs)"
    ));
}
