use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use bytes::{Bytes, BytesMut};
use futures_util::stream::Stream;
use serde_json::Value;

use super::audit::{CapturedToolUse, CapturedUsage, GatewayAudit};

#[derive(Default)]
struct TapState {
    sse_buffer: Vec<u8>,
    response_buffer: BytesMut,
    input_tokens: u32,
    output_tokens: u32,
    tool_uses_in_progress: Vec<PartialToolUse>,
    tool_uses_done: Vec<CapturedToolUse>,
    served_model: Option<String>,
    error: Option<String>,
    finalized: bool,
}

#[derive(Default, Debug)]
struct PartialToolUse {
    index: i64,
    id: String,
    name: String,
    input_json: String,
}

pub fn tap<S>(upstream: S, audit: Arc<GatewayAudit>) -> Body
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
{
    let state = Arc::new(Mutex::new(TapState::default()));
    let tapped = TappedStream {
        inner: Box::pin(upstream),
        state: Arc::clone(&state),
        audit,
    };
    Body::from_stream(tapped)
}

struct TappedStream<S> {
    inner: Pin<Box<S>>,
    state: Arc<Mutex<TapState>>,
    audit: Arc<GatewayAudit>,
}

impl<S> Stream for TappedStream<S>
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
{
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(bytes))) => {
                if let Ok(mut s) = self.state.lock() {
                    s.response_buffer.extend_from_slice(&bytes);
                    s.sse_buffer.extend_from_slice(&bytes);
                    drain_sse(&mut s);
                }
                Poll::Ready(Some(Ok(bytes)))
            },
            Poll::Ready(Some(Err(e))) => {
                if let Ok(mut s) = self.state.lock() {
                    s.error = Some(e.to_string());
                }
                Poll::Ready(Some(Err(e)))
            },
            Poll::Ready(None) => {
                let (usage, tools, body, served_model, error, already_finalized) = {
                    let mut s = self
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if s.finalized {
                        return Poll::Ready(None);
                    }
                    finalize_partials(&mut s);
                    s.finalized = true;
                    (
                        CapturedUsage {
                            input_tokens: s.input_tokens,
                            output_tokens: s.output_tokens,
                        },
                        std::mem::take(&mut s.tool_uses_done),
                        std::mem::take(&mut s.response_buffer).freeze(),
                        s.served_model.take(),
                        s.error.take(),
                        false,
                    )
                };
                if already_finalized {
                    return Poll::Ready(None);
                }

                let audit = Arc::clone(&self.audit);
                tokio::spawn(async move {
                    if let Some(err) = error {
                        let _ = audit.fail(&err).await;
                    } else {
                        if let Some(model) = served_model.as_deref() {
                            audit.set_served_model(model).await;
                        }
                        if let Err(e) = audit.complete(usage, tools, &body).await {
                            tracing::warn!(error = %e, "stream audit complete failed");
                        }
                    }
                });
                Poll::Ready(None)
            },
        }
    }
}

impl<S> Drop for TappedStream<S> {
    fn drop(&mut self) {
        let snapshot = self.state.lock().ok().and_then(|mut s| {
            if s.finalized {
                return None;
            }
            finalize_partials(&mut s);
            s.finalized = true;
            Some((
                CapturedUsage {
                    input_tokens: s.input_tokens,
                    output_tokens: s.output_tokens,
                },
                std::mem::take(&mut s.tool_uses_done),
                std::mem::take(&mut s.response_buffer).freeze(),
                s.served_model.take(),
                s.error.take(),
            ))
        });

        let Some((usage, tools, body, served_model, prior_error)) = snapshot else {
            return;
        };

        let audit = Arc::clone(&self.audit);
        tokio::spawn(async move {
            if let Some(model) = served_model.as_deref() {
                audit.set_served_model(model).await;
            }
            let saw_message_stop = body
                .windows(b"\"type\":\"message_stop\"".len())
                .any(|w| w == b"\"type\":\"message_stop\"");
            if saw_message_stop && prior_error.is_none() {
                if let Err(e) = audit.complete(usage, tools, &body).await {
                    tracing::warn!(error = %e, "drop-path audit complete failed");
                }
            } else {
                let msg =
                    prior_error.unwrap_or_else(|| "stream abandoned by downstream".to_string());
                let _ = audit.fail(&msg).await;
            }
        });
    }
}

fn drain_sse(state: &mut TapState) {
    loop {
        let Some(pos) = find_double_newline(&state.sse_buffer) else {
            return;
        };
        let frame_bytes: Vec<u8> = state.sse_buffer.drain(..pos + 2).collect();
        let frame = String::from_utf8_lossy(&frame_bytes);
        for line in frame.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    continue;
                }
                let Ok(json) = serde_json::from_str::<Value>(data) else {
                    continue;
                };
                handle_sse_event(state, &json);
            }
        }
    }
}

fn find_double_newline(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

fn handle_sse_event(state: &mut TapState, event: &Value) {
    let Some(kind) = event.get("type").and_then(Value::as_str) else {
        return;
    };
    match kind {
        "message_start" => {
            if let Some(message) = event.get("message") {
                if let Some(model) = message.get("model").and_then(Value::as_str) {
                    if !model.is_empty() {
                        state.served_model = Some(model.to_string());
                    }
                }
                if let Some(usage) = message.get("usage") {
                    if let Some(v) = usage.get("input_tokens").and_then(Value::as_u64) {
                        state.input_tokens = v as u32;
                    }
                    if let Some(v) = usage.get("output_tokens").and_then(Value::as_u64) {
                        state.output_tokens = v as u32;
                    }
                }
            }
        },
        "message_delta" => {
            if let Some(usage) = event.get("usage") {
                if let Some(v) = usage.get("output_tokens").and_then(Value::as_u64) {
                    state.output_tokens = v as u32;
                }
                if let Some(v) = usage.get("input_tokens").and_then(Value::as_u64) {
                    state.input_tokens = v as u32;
                }
            }
        },
        "content_block_start" => {
            let index = event.get("index").and_then(Value::as_i64).unwrap_or(-1);
            if let Some(block) = event.get("content_block") {
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    let id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    state.tool_uses_in_progress.push(PartialToolUse {
                        index,
                        id,
                        name,
                        input_json: String::new(),
                    });
                }
            }
        },
        "content_block_delta" => {
            let index = event.get("index").and_then(Value::as_i64).unwrap_or(-1);
            if let Some(delta) = event.get("delta") {
                if delta.get("type").and_then(Value::as_str) == Some("input_json_delta") {
                    if let Some(partial) = delta.get("partial_json").and_then(Value::as_str) {
                        if let Some(pt) = state
                            .tool_uses_in_progress
                            .iter_mut()
                            .find(|p| p.index == index)
                        {
                            pt.input_json.push_str(partial);
                        }
                    }
                }
            }
        },
        "content_block_stop" => {
            let index = event.get("index").and_then(Value::as_i64).unwrap_or(-1);
            if let Some(pos) = state
                .tool_uses_in_progress
                .iter()
                .position(|p| p.index == index)
            {
                let done = state.tool_uses_in_progress.remove(pos);
                state.tool_uses_done.push(CapturedToolUse {
                    ai_tool_call_id: done.id,
                    tool_name: done.name,
                    tool_input: if done.input_json.is_empty() {
                        "{}".to_string()
                    } else {
                        done.input_json
                    },
                });
            }
        },
        _ => {},
    }
}

fn finalize_partials(state: &mut TapState) {
    let leftover: Vec<PartialToolUse> = std::mem::take(&mut state.tool_uses_in_progress);
    for p in leftover {
        state.tool_uses_done.push(CapturedToolUse {
            ai_tool_call_id: p.id,
            tool_name: p.name,
            tool_input: if p.input_json.is_empty() {
                "{}".to_string()
            } else {
                p.input_json
            },
        });
    }
}
