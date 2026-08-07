//! SSE keepalive injection for proxied event streams.
//!
//! An MCP session rides a long-lived Streamable-HTTP/SSE connection between
//! the host app and this loopback proxy. When that socket dies silently, the
//! host queues tool calls against it until a TCP timeout fires minutes later —
//! observed as ~147 s stalls in Cowork against a healthy upstream. A comment
//! frame every few seconds makes a dead connection fail fast so the host
//! reconnects in seconds instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use futures_util::Stream;
use hyper::body::Frame;
use tokio::time::{Instant, Sleep};

pub const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSE_KEEPALIVE_PAYLOAD: &[u8] = b": keepalive\n\n";

#[derive(Debug)]
pub struct SseKeepalive<S> {
    inner: S,
    interval: Duration,
    deadline: Pin<Box<Sleep>>,
}

impl<S> SseKeepalive<S> {
    pub fn new(inner: S, interval: Duration) -> Self {
        Self {
            inner,
            interval,
            deadline: Box::pin(tokio::time::sleep(interval)),
        }
    }
}

impl<S> Stream for SseKeepalive<S>
where
    S: Stream<Item = std::io::Result<Frame<Bytes>>> + Unpin,
{
    type Item = std::io::Result<Frame<Bytes>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let interval = self.interval;
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(item)) => {
                self.deadline.as_mut().reset(Instant::now() + interval);
                Poll::Ready(Some(item))
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => match self.deadline.as_mut().poll(cx) {
                Poll::Ready(()) => {
                    self.deadline.as_mut().reset(Instant::now() + interval);
                    Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(
                        SSE_KEEPALIVE_PAYLOAD,
                    )))))
                },
                Poll::Pending => Poll::Pending,
            },
        }
    }
}
