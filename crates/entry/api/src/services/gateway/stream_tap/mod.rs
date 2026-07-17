//! Streaming response tap: re-renders upstream canonical events to the inbound
//! wire format while accumulating a full response snapshot for the audit sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod accumulator;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::accumulator::{Summary, TapState, accumulate_event, extract_summary, snapshot};
}

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use bytes::Bytes;
use futures_util::stream::{BoxStream, Stream};

use self::accumulator::{Summary, TapState, accumulate_event, extract_summary, snapshot};
use super::audit::GatewayAudit;
use super::protocol::canonical_response::CanonicalEvent;
use super::protocol::inbound::InboundAdapter;

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
                    let terminal = matches!(
                        event,
                        CanonicalEvent::ContentBlockStop { .. }
                            | CanonicalEvent::MessageStop { .. }
                    );
                    let snap = self.state.lock().map_or(None, |mut s| {
                        accumulate_event(&mut s, &event);
                        terminal.then(|| snapshot(&s))
                    });
                    let rendered = snap
                        .as_ref()
                        .and_then(|snapshot| {
                            self.inbound.render_terminal_event(
                                &event,
                                snapshot,
                                &self.request_model,
                            )
                        })
                        .or_else(|| self.inbound.render_event(&event, &self.request_model));
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
    fn take_summary(&self) -> Option<Summary> {
        self.state.lock().ok().and_then(|mut s| {
            if s.finalized {
                return None;
            }
            s.finalized = true;
            Some(extract_summary(&mut s))
        })
    }

    fn finalize_on_eof(&self) -> Poll<Option<Result<Bytes, std::io::Error>>> {
        let Some(summary) = self.take_summary() else {
            return Poll::Ready(None);
        };
        finalize(Arc::clone(&self.audit), summary, "eof");
        Poll::Ready(None)
    }
}

impl Drop for TappedStream {
    fn drop(&mut self) {
        let Some(summary) = self.take_summary() else {
            return;
        };
        finalize(Arc::clone(&self.audit), summary, "drop");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalizeDecision {
    Fail(&'static str),
    Complete { cost_capture_miss: bool },
}

pub const fn classify(
    error: Option<&str>,
    saw_stop: bool,
    has_content: bool,
    has_usage: bool,
) -> FinalizeDecision {
    if error.is_some() {
        return FinalizeDecision::Fail("upstream stream error");
    }
    if !saw_stop {
        return FinalizeDecision::Fail(if has_content {
            "stream ended without stop event"
        } else {
            "empty upstream stream"
        });
    }
    FinalizeDecision::Complete {
        cost_capture_miss: has_content && !has_usage,
    }
}

fn finalize(audit: Arc<GatewayAudit>, summary: Summary, origin: &'static str) {
    tokio::spawn(async move {
        if let Some(model) = summary.served_model.as_deref() {
            audit.set_served_model(model).await;
        }

        let has_content = !summary.final_bytes.is_empty();
        let has_usage = summary.usage.input_tokens > 0 || summary.usage.output_tokens > 0;
        match classify(
            summary.error.as_deref(),
            summary.saw_stop,
            has_content,
            has_usage,
        ) {
            FinalizeDecision::Fail(reason) => {
                let msg = summary.error.as_deref().unwrap_or(reason);
                if let Err(e) = audit.fail(msg).await {
                    tracing::warn!(origin, error = %e, "stream audit fail failed");
                }
            },
            FinalizeDecision::Complete { cost_capture_miss } => {
                if cost_capture_miss {
                    tracing::warn!(
                        origin,
                        "stream completed with content but zero usage: cost capture miss"
                    );
                }
                if let Err(e) = audit
                    .complete(
                        summary.usage,
                        summary.tool_calls,
                        &summary.response,
                        &summary.final_bytes,
                    )
                    .await
                {
                    tracing::warn!(origin, error = %e, "stream audit complete failed");
                }
            },
        }
    });
}
