//! The analytics event subscription a conferred route set is handed.
//!
//! The kernel builds the analytics broadcaster and hands it, as an
//! [`AnalyticsSubscription`], only to the route set whose base carries the
//! broadcaster conferral. It is subscribe-only: a subscriber receives
//! pre-rendered [`SseFrame`]s and is unregistered when its
//! [`SubscribedStream`] is dropped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures::future::BoxFuture;
use systemprompt_identifiers::{ConnectionId, UserId};
use tokio::sync::mpsc::Receiver;

/// Subscribes a connection to the analytics event stream.
pub trait AnalyticsSubscription: Send + Sync {
    /// `None` means the per-user subscription cap is reached.
    fn subscribe<'a>(
        &'a self,
        user_id: &'a UserId,
        connection_id: &'a ConnectionId,
    ) -> BoxFuture<'a, Option<SubscribedStream>>;
}

/// One subscriber's frames, unregistered from the broadcaster on drop.
pub struct SubscribedStream {
    pub frames: Receiver<SseFrame>,
    _guard: Box<dyn Send + Sync>,
}

impl SubscribedStream {
    #[must_use]
    pub fn new(frames: Receiver<SseFrame>, guard: Box<dyn Send + Sync>) -> Self {
        Self {
            frames,
            _guard: guard,
        }
    }
}

impl std::fmt::Debug for SubscribedStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscribedStream").finish_non_exhaustive()
    }
}

/// One pre-rendered server-sent event.
#[derive(Debug, Clone)]
pub struct SseFrame {
    pub event: Option<String>,
    pub id: Option<String>,
    pub data: String,
}
