//! Streaming response tap: re-renders upstream canonical events to the inbound
//! wire format while accumulating a full response snapshot for the audit sink.

mod accumulator;

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use bytes::Bytes;
use futures_util::stream::{BoxStream, Stream};

use self::accumulator::{Summary, TapState, accumulate_event, extract_summary};
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
        spawn_audit_complete(Arc::clone(&self.audit), summary);
        Poll::Ready(None)
    }
}

impl Drop for TappedStream {
    fn drop(&mut self) {
        let Some(summary) = self.take_summary() else {
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

fn spawn_audit_complete(audit: Arc<GatewayAudit>, summary: Summary) {
    tokio::spawn(async move {
        if let Some(model) = summary.served_model.as_deref() {
            audit.set_served_model(model).await;
        }
        if let Some(err) = summary.error {
            if let Err(e) = audit.fail(&err).await {
                tracing::warn!(error = %e, "stream audit fail failed");
            }
        } else if let Err(e) = audit
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
    });
}
