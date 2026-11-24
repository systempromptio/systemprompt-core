//! The deployment slots a send holds for as long as its answer is being read:
//! the bulkhead permit and the in-flight gauge entry. A buffered answer ends
//! the hold when it is returned; a streamed one carries it inside the stream,
//! so the slot is released when the client's body is dropped, not when the
//! upstream answered with headers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures_util::StreamExt;
use tokio::sync::OwnedSemaphorePermit;

use super::InFlight;
use crate::protocol::outbound::OutboundOutcome;
use crate::protocol::outbound::relay::{Relayed, RelayedBody};

pub(super) struct DeploymentSlot {
    pub(super) _permit: Option<OwnedSemaphorePermit>,
    pub(super) _in_flight: InFlight,
}

impl DeploymentSlot {
    pub(super) fn hold(self, outcome: OutboundOutcome) -> OutboundOutcome {
        match outcome {
            OutboundOutcome::Streaming(stream) => OutboundOutcome::Streaming(carry(stream, self)),
            OutboundOutcome::RawStreaming {
                content_type,
                stream,
            } => OutboundOutcome::RawStreaming {
                content_type,
                stream: carry(stream, self),
            },
            OutboundOutcome::Relayed(Relayed {
                headers,
                body: RelayedBody::Streaming { bytes, events },
            }) => OutboundOutcome::Relayed(Relayed {
                headers,
                body: RelayedBody::Streaming {
                    bytes: carry(bytes, self),
                    events,
                },
            }),
            buffered => buffered,
        }
    }
}

fn carry<T: Send + 'static>(
    stream: futures_util::stream::BoxStream<'static, T>,
    slot: DeploymentSlot,
) -> futures_util::stream::BoxStream<'static, T> {
    stream
        .map(move |item| {
            let _held = &slot;
            item
        })
        .boxed()
}
