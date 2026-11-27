//! Drives one open response body: bytes through the SSE parser and the protocol
//! decoder, out as unified events, until the decoder completes, the body fails
//! or ends, or the call is cancelled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::VecDeque;

use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::chat::ChatDecoder;
use crate::error::LlmError;
use crate::event::Event;
use crate::messages::MessagesDecoder;
use crate::model::Protocol;
use crate::sse::{SseFrame, SseParser};
use crate::stream::EventStream;
use crate::transport::ByteStream;

pub(crate) fn events(
    body: ByteStream,
    protocol: Protocol,
    cancel: CancellationToken,
) -> EventStream {
    let decoder = match protocol {
        Protocol::Messages => Decoder::Messages(MessagesDecoder::default()),
        Protocol::ChatCompletions => Decoder::Chat(ChatDecoder::default()),
    };
    let driver = Driver {
        body,
        parser: SseParser::default(),
        decoder,
        pending: VecDeque::new(),
        cancel,
        done: false,
    };
    let events = futures_util::stream::unfold(driver, |mut driver| async move {
        loop {
            if let Some(item) = driver.pending.pop_front() {
                return Some((item, driver));
            }
            if driver.done {
                return None;
            }
            driver.advance().await;
        }
    });
    Box::pin(events)
}

enum Decoder {
    Messages(MessagesDecoder),
    Chat(ChatDecoder),
}

impl Decoder {
    fn push(&mut self, frame: &SseFrame) -> Result<Vec<Event>, LlmError> {
        match self {
            Self::Messages(decoder) => decoder.push(frame),
            Self::Chat(decoder) => decoder.push(frame),
        }
    }

    fn finish(&mut self) -> Result<Vec<Event>, LlmError> {
        match self {
            Self::Messages(decoder) => decoder.finish(),
            Self::Chat(decoder) => decoder.finish(),
        }
    }

    const fn is_complete(&self) -> bool {
        match self {
            Self::Messages(decoder) => decoder.is_complete(),
            Self::Chat(decoder) => decoder.is_complete(),
        }
    }
}

struct Driver {
    body: ByteStream,
    parser: SseParser,
    decoder: Decoder,
    pending: VecDeque<Result<Event, LlmError>>,
    cancel: CancellationToken,
    done: bool,
}

impl Driver {
    async fn advance(&mut self) {
        let next = tokio::select! {
            biased;
            () = self.cancel.cancelled() => None,
            chunk = self.body.next() => Some(chunk),
        };
        match next {
            None => self.fail(LlmError::Aborted),
            Some(Some(Ok(bytes))) => {
                let frames = self.parser.push(&bytes);
                self.handle(frames);
            },
            Some(Some(Err(error))) => self.fail(LlmError::Transport(error)),
            Some(None) => {
                let frames = self.parser.finish();
                self.handle(frames);
                if !self.done {
                    self.complete();
                }
            },
        }
    }

    fn handle(&mut self, frames: Vec<SseFrame>) {
        for frame in frames {
            match self.decoder.push(&frame) {
                Ok(events) => self.pending.extend(events.into_iter().map(Ok)),
                Err(error) => return self.fail(error),
            }
            if self.decoder.is_complete() {
                return self.complete();
            }
        }
    }

    fn complete(&mut self) {
        match self.decoder.finish() {
            Ok(events) => self.pending.extend(events.into_iter().map(Ok)),
            Err(error) => self.pending.push_back(Err(error)),
        }
        self.done = true;
    }

    fn fail(&mut self, error: LlmError) {
        if matches!(error, LlmError::Aborted) {
            tracing::debug!("model stream cancelled");
        } else {
            tracing::warn!(error = %error, "model stream failed");
        }
        self.pending.push_back(Err(error));
        self.done = true;
    }
}
