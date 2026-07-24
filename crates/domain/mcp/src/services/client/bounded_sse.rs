//! Byte-ceiling enforcement for inbound SSE streams.
//!
//! rmcp's streamable-HTTP transport carries a `max_sse_event_size` budget, but
//! it can only be applied inside the client implementation — at the raw byte
//! layer, before SSE parsing. rmcp enforces it for its own reqwest-backed
//! client via a crate-private helper; because [`HttpClientWithContext`] parses
//! SSE itself, the ceiling would go unenforced unless we reproduce it here.
//!
//! [`bounded_sse_stream`] wraps a byte stream and fails it as soon as a single
//! event's retained payload would exceed the limit, matching rmcp's accounting:
//! comment lines are excluded, a blank line resets the budget, and each joined
//! data field costs an extra byte for the newline the parser inserts.
//!
//! [`HttpClientWithContext`]: super::HttpClientWithContext
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures::StreamExt;
use futures::stream::{BoxStream, Stream};
use sse_stream::{Error as SseError, Sse, SseStream};

pub(super) const DEFAULT_MAX_SSE_EVENT_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
enum BoundedSseError {
    #[error(transparent)]
    Source(Box<dyn std::error::Error + Send + Sync>),
    #[error("SSE event exceeded the maximum size of {max_size} bytes")]
    EventTooLarge { max_size: usize },
}

#[derive(Debug)]
struct EventSizeLimiter {
    max_size: usize,
    retained_size: usize,
    line_size: usize,
    line_is_comment: bool,
    previous_was_cr: bool,
}

impl EventSizeLimiter {
    const fn new(max_size: usize) -> Self {
        Self {
            max_size,
            retained_size: 0,
            line_size: 0,
            line_is_comment: false,
            previous_was_cr: false,
        }
    }

    fn observe(&mut self, chunk: &[u8]) -> Result<(), ()> {
        for &byte in chunk {
            if self.previous_was_cr {
                self.previous_was_cr = false;
                if byte == b'\n' {
                    continue;
                }
            }

            match byte {
                b'\r' => {
                    self.finish_line()?;
                    self.previous_was_cr = true;
                },
                b'\n' => self.finish_line()?,
                _ => {
                    if self.line_size == 0 {
                        self.line_is_comment = byte == b':';
                    }
                    self.line_size = self.line_size.saturating_add(1);
                    self.check_limit()?;
                },
            }
        }
        Ok(())
    }

    const fn finish_line(&mut self) -> Result<(), ()> {
        if self.line_size == 0 {
            self.retained_size = 0;
        } else if !self.line_is_comment {
            self.retained_size = self
                .retained_size
                .saturating_add(self.line_size)
                .saturating_add(1);
        }
        self.line_size = 0;
        self.line_is_comment = false;
        self.check_limit()
    }

    const fn check_limit(&self) -> Result<(), ()> {
        if self.retained_size.saturating_add(self.line_size) > self.max_size {
            Err(())
        } else {
            Ok(())
        }
    }
}

pub(super) fn bounded_sse_stream<S, B, E>(
    stream: S,
    max_event_size: usize,
) -> BoxStream<'static, Result<Sse, SseError>>
where
    S: Stream<Item = Result<B, E>> + Send + 'static,
    B: AsRef<[u8]> + bytes::Buf + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let bounded = futures::stream::unfold(
        (
            Box::pin(stream),
            Some(EventSizeLimiter::new(max_event_size)),
        ),
        |(mut stream, limiter)| async move {
            let mut limiter = limiter?;
            let chunk = match stream.next().await? {
                Ok(chunk) => chunk,
                Err(error) => {
                    let item = Err(BoundedSseError::Source(Box::new(error)));
                    return Some((item, (stream, None)));
                },
            };
            if limiter.observe(chunk.as_ref()).is_err() {
                let item = Err(BoundedSseError::EventTooLarge {
                    max_size: limiter.max_size,
                });
                return Some((item, (stream, None)));
            }
            Some((Ok(chunk), (stream, Some(limiter))))
        },
    );

    SseStream::from_bytes_stream(bounded).boxed()
}
