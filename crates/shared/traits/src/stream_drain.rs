//! The signal that tells long-lived response streams to end themselves.
//!
//! When the server's connection drain runs out, the connections still open
//! are about to be dropped. A stream holding a [`StreamDrainWatch`] sees
//! [`StreamDrain::end_streams`] first and closes with a protocol-correct
//! terminal frame, so its client reads a clean error instead of a truncated
//! body. The server owns one [`StreamDrain`] (carried on
//! [`crate::BackgroundTasks`]) and raises it once, at drain expiry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::pin::Pin;
use std::task::Context;

use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};

#[derive(Debug, Clone, Default)]
pub struct StreamDrain {
    token: CancellationToken,
}

impl StreamDrain {
    pub fn end_streams(&self) {
        self.token.cancel();
    }

    #[must_use]
    pub fn is_ending(&self) -> bool {
        self.token.is_cancelled()
    }

    #[must_use]
    pub fn watch(&self) -> StreamDrainWatch {
        StreamDrainWatch {
            ended: Box::pin(self.token.clone().cancelled_owned()),
        }
    }
}

/// A pollable view of [`StreamDrain`] for a hand-written `Stream`; it
/// registers the task's waker, so a pending stream is woken when streams end.
#[derive(Debug)]
pub struct StreamDrainWatch {
    ended: Pin<Box<WaitForCancellationFutureOwned>>,
}

impl StreamDrainWatch {
    pub fn poll_ended(&mut self, cx: &mut Context<'_>) -> bool {
        self.ended.as_mut().poll(cx).is_ready()
    }
}
