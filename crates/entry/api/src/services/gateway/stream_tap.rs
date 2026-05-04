mod sse_parser;

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use bytes::{Bytes, BytesMut};
use futures_util::stream::Stream;

use super::audit::GatewayAudit;
use super::captures::{CapturedToolUse, CapturedUsage};
use sse_parser::{drain_sse, finalize_partials};

#[derive(Default)]
pub(super) struct TapState {
    pub sse_buffer: Vec<u8>,
    pub response_buffer: BytesMut,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub tool_uses_in_progress: Vec<PartialToolUse>,
    pub tool_uses_done: Vec<CapturedToolUse>,
    pub served_model: Option<String>,
    pub error: Option<String>,
    pub finalized: bool,
}

#[derive(Default, Debug)]
pub(super) struct PartialToolUse {
    pub index: i64,
    pub id: String,
    pub name: String,
    pub input_json: String,
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
                // Why: lock failure means the mutex is poisoned; we drop telemetry
                // for this chunk rather than killing the live response stream.
                if let Ok(mut s) = self.state.lock() {
                    s.response_buffer.extend_from_slice(&bytes);
                    s.sse_buffer.extend_from_slice(&bytes);
                    drain_sse(&mut s);
                }
                Poll::Ready(Some(Ok(bytes)))
            },
            Poll::Ready(Some(Err(e))) => {
                // Why: same poisoned-lock recovery — record the error if we can,
                // otherwise the Drop path will mark the stream as failed anyway.
                if let Ok(mut s) = self.state.lock() {
                    s.error = Some(e.to_string());
                }
                Poll::Ready(Some(Err(e)))
            },
            Poll::Ready(None) => self.finalize_on_eof(),
        }
    }
}

impl<S> TappedStream<S> {
    fn finalize_on_eof(&self) -> Poll<Option<Result<Bytes, std::io::Error>>> {
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
                if let Err(e) = audit.fail(&err).await {
                    tracing::warn!(error = %e, "stream audit fail failed");
                }
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
    }
}

impl<S> Drop for TappedStream<S> {
    fn drop(&mut self) {
        // Why: poisoned-lock recovery — if `.lock()` errors we silently skip the
        // drop-path audit because the stream was already torn down.
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
                if let Err(e) = audit.fail(&msg).await {
                    tracing::warn!(error = %e, "drop-path audit fail failed");
                }
            }
        });
    }
}
