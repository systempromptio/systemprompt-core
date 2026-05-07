use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use bytes::{Bytes, BytesMut};
use futures_util::stream::{BoxStream, Stream};

use super::audit::GatewayAudit;
use super::captures::{CapturedToolUse, CapturedUsage};
use super::protocol::canonical::CanonicalContent;
use super::protocol::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalUsage, ContentBlockKind,
};
use super::protocol::inbound::InboundAdapter;

#[derive(Default)]
struct TapState {
    response_id: String,
    served_model: String,
    usage: CanonicalUsage,
    blocks: Vec<BlockAccumulator>,
    final_stop_reason: Option<super::protocol::canonical_response::CanonicalStopReason>,
    final_bytes: BytesMut,
    error: Option<String>,
    finalized: bool,
}

#[derive(Clone)]
enum BlockAccumulator {
    Text(String),
    Thinking { text: String, signature: Option<String> },
    ToolUse { id: String, name: String, partial: String },
}

pub fn tap(
    upstream: BoxStream<'static, Result<CanonicalEvent, String>>,
    inbound: Arc<dyn InboundAdapter>,
    request_model: String,
    audit: Arc<GatewayAudit>,
) -> Body {
    let state = Arc::new(Mutex::new(TapState::default()));
    let tapped = TappedStream {
        inner: upstream,
        state: Arc::clone(&state),
        inbound,
        request_model,
        audit,
    };
    Body::from_stream(tapped)
}

struct TappedStream {
    inner: BoxStream<'static, Result<CanonicalEvent, String>>,
    state: Arc<Mutex<TapState>>,
    inbound: Arc<dyn InboundAdapter>,
    request_model: String,
    audit: Arc<GatewayAudit>,
}

impl Stream for TappedStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    return self.finalize_on_eof();
                },
                Poll::Ready(Some(Err(e))) => {
                    if let Ok(mut s) = self.state.lock() {
                        s.error = Some(e.clone());
                    }
                    let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, e);
                    return Poll::Ready(Some(Err(err)));
                },
                Poll::Ready(Some(Ok(event))) => {
                    if let Ok(mut s) = self.state.lock() {
                        accumulate_event(&mut s, &event);
                    }
                    let rendered = self.inbound.render_event(&event, &self.request_model);
                    if let Some(bytes) = rendered {
                        if let Ok(mut s) = self.state.lock() {
                            s.final_bytes.extend_from_slice(&bytes);
                        }
                        return Poll::Ready(Some(Ok(bytes)));
                    }
                },
            }
        }
    }
}

impl TappedStream {
    fn finalize_on_eof(&self) -> Poll<Option<Result<Bytes, std::io::Error>>> {
        let snapshot = self.state.lock().ok().and_then(|mut s| {
            if s.finalized {
                return None;
            }
            s.finalized = true;
            Some(extract_summary(&mut s))
        });

        let Some(summary) = snapshot else {
            return Poll::Ready(None);
        };

        spawn_audit_complete(self.audit.clone(), self.request_model.clone(), summary);
        Poll::Ready(None)
    }
}

impl Drop for TappedStream {
    fn drop(&mut self) {
        let snapshot = self.state.lock().ok().and_then(|mut s| {
            if s.finalized {
                return None;
            }
            s.finalized = true;
            Some(extract_summary(&mut s))
        });
        let Some(summary) = snapshot else {
            return;
        };

        let audit = Arc::clone(&self.audit);
        tokio::spawn(async move {
            if let Some(model) = summary.served_model.as_deref() {
                audit.set_served_model(model).await;
            }
            if summary.saw_stop && summary.error.is_none() {
                if let Err(e) = audit
                    .complete(
                        summary.usage,
                        summary.tool_calls,
                        &summary.response,
                        &summary.final_bytes,
                    )
                    .await
                {
                    tracing::warn!(error = %e, "drop-path audit complete failed");
                }
            } else {
                let msg = summary
                    .error
                    .unwrap_or_else(|| "stream abandoned by downstream".into());
                if let Err(e) = audit.fail(&msg).await {
                    tracing::warn!(error = %e, "drop-path audit fail failed");
                }
            }
        });
    }
}

struct Summary {
    usage: CapturedUsage,
    tool_calls: Vec<CapturedToolUse>,
    response: CanonicalResponse,
    final_bytes: Bytes,
    served_model: Option<String>,
    error: Option<String>,
    saw_stop: bool,
}

fn extract_summary(state: &mut TapState) -> Summary {
    let response = build_response(state);
    let usage = CapturedUsage {
        input_tokens: state.usage.input_tokens,
        output_tokens: state.usage.output_tokens,
    };
    let tool_calls = response
        .content
        .iter()
        .filter_map(|c| {
            if let CanonicalContent::ToolUse { id, name, input } = c {
                Some(CapturedToolUse {
                    ai_tool_call_id: id.clone(),
                    tool_name: name.clone(),
                    tool_input: serde_json::to_string(input).unwrap_or_default(),
                })
            } else {
                None
            }
        })
        .collect();
    let final_bytes = std::mem::take(&mut state.final_bytes).freeze();
    let served_model = if state.served_model.is_empty() {
        None
    } else {
        Some(state.served_model.clone())
    };
    Summary {
        usage,
        tool_calls,
        response,
        final_bytes,
        served_model,
        error: state.error.clone(),
        saw_stop: state.final_stop_reason.is_some(),
    }
}

fn build_response(state: &TapState) -> CanonicalResponse {
    let content = state
        .blocks
        .iter()
        .map(|b| match b {
            BlockAccumulator::Text(t) => CanonicalContent::Text(t.clone()),
            BlockAccumulator::Thinking { text, signature } => CanonicalContent::Thinking {
                text: text.clone(),
                signature: signature.clone(),
            },
            BlockAccumulator::ToolUse { id, name, partial } => CanonicalContent::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: serde_json::from_str(partial)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
            },
        })
        .collect();
    CanonicalResponse {
        id: state.response_id.clone(),
        model: state.served_model.clone(),
        content,
        stop_reason: state.final_stop_reason,
        usage: state.usage,
    }
}

fn accumulate_event(state: &mut TapState, event: &CanonicalEvent) {
    match event {
        CanonicalEvent::MessageStart { id, model, usage } => {
            state.response_id = id.clone();
            if !model.is_empty() {
                state.served_model = model.clone();
            }
            state.usage = *usage;
        },
        CanonicalEvent::ContentBlockStart { index, block } => {
            let slot = match block {
                ContentBlockKind::Text => BlockAccumulator::Text(String::new()),
                ContentBlockKind::Thinking { signature } => BlockAccumulator::Thinking {
                    text: String::new(),
                    signature: signature.clone(),
                },
                ContentBlockKind::ToolUse { id, name } => BlockAccumulator::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    partial: String::new(),
                },
            };
            let idx = *index as usize;
            while state.blocks.len() <= idx {
                state.blocks.push(BlockAccumulator::Text(String::new()));
            }
            state.blocks[idx] = slot;
        },
        CanonicalEvent::TextDelta { index, text } => {
            if let Some(BlockAccumulator::Text(buf)) = state.blocks.get_mut(*index as usize) {
                buf.push_str(text);
            }
        },
        CanonicalEvent::ThinkingDelta { index, text } => {
            if let Some(BlockAccumulator::Thinking { text: buf, .. }) =
                state.blocks.get_mut(*index as usize)
            {
                buf.push_str(text);
            }
        },
        CanonicalEvent::ToolUseDelta { index, partial_json } => {
            if let Some(BlockAccumulator::ToolUse { partial, .. }) =
                state.blocks.get_mut(*index as usize)
            {
                partial.push_str(partial_json);
            }
        },
        CanonicalEvent::ContentBlockStop { .. } => {},
        CanonicalEvent::UsageDelta(u) => {
            if u.input_tokens > 0 {
                state.usage.input_tokens = u.input_tokens;
            }
            if u.output_tokens > 0 {
                state.usage.output_tokens = u.output_tokens;
            }
        },
        CanonicalEvent::MessageStop { stop_reason } => {
            state.final_stop_reason = stop_reason.or(Some(
                super::protocol::canonical_response::CanonicalStopReason::EndTurn,
            ));
        },
        CanonicalEvent::Error(msg) => {
            state.error = Some(msg.clone());
        },
    }
}

fn spawn_audit_complete(audit: Arc<GatewayAudit>, _request_model: String, summary: Summary) {
    tokio::spawn(async move {
        if let Some(model) = summary.served_model.as_deref() {
            audit.set_served_model(model).await;
        }
        if let Some(err) = summary.error {
            if let Err(e) = audit.fail(&err).await {
                tracing::warn!(error = %e, "stream audit fail failed");
            }
        } else {
            if let Err(e) = audit
                .complete(
                    summary.usage,
                    summary.tool_calls,
                    &summary.response,
                    &summary.final_bytes,
                )
                .await
            {
                tracing::warn!(error = %e, "stream audit complete failed");
            }
        }
    });
}
