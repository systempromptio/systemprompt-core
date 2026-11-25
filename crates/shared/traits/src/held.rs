//! A guard held for as long as a response is being read.
//!
//! [`Held`] wraps a stream or an HTTP body together with a guard (a
//! semaphore permit, a gauge entry) and drops the guard only when the
//! wrapped value is dropped. A streamed answer therefore keeps its slot until
//! the client has read the body or gone away, not until the headers were
//! sent. The HTTP body form forwards the size hint and end-of-stream state, so
//! a fixed-length body keeps its `Content-Length`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

/// A stream or HTTP body that keeps `guard` alive until it is dropped.
#[derive(Debug)]
pub struct Held<S, G> {
    inner: S,
    _guard: G,
}

impl<S, G> Held<S, G> {
    #[must_use]
    pub const fn new(inner: S, guard: G) -> Self {
        Self {
            inner,
            _guard: guard,
        }
    }
}

impl<S: Unpin, G> Unpin for Held<S, G> {}

impl<S: Stream + Unpin, G> Stream for Held<S, G> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().inner).poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<B: http_body::Body + Unpin, G> http_body::Body for Held<B, G> {
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.get_mut().inner).poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}
