//! SSE keep-alive wrapper for proxied event streams: emits a comment frame
//! whenever the upstream stays silent for the keep-alive interval, so idle
//! intermediaries do not drop a long-lived MCP stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Instant, Sleep};

pub(super) const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSE_KEEPALIVE_PAYLOAD: &[u8] = b": keepalive\n\n";

pub(super) struct SseKeepaliveStream<S> {
    inner: S,
    keepalive_interval: Duration,
    deadline: Pin<Box<Sleep>>,
}

impl<S> SseKeepaliveStream<S> {
    pub(super) fn new(inner: S, keepalive_interval: Duration) -> Self {
        Self {
            inner,
            keepalive_interval,
            deadline: Box::pin(tokio::time::sleep(keepalive_interval)),
        }
    }
}

impl<S> futures_util::Stream for SseKeepaliveStream<S>
where
    S: futures_util::Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
{
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let interval = self.keepalive_interval;
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(item)) => {
                self.deadline.as_mut().reset(Instant::now() + interval);
                Poll::Ready(Some(item))
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => match self.deadline.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    self.deadline.as_mut().reset(Instant::now() + interval);
                    Poll::Ready(Some(Ok(Bytes::from_static(SSE_KEEPALIVE_PAYLOAD))))
                },
                Poll::Pending => Poll::Pending,
            },
        }
    }
}
