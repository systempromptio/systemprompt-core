//! [`StreamFeeds`]: the per-user SSE subscriptions the stream routes open.
//!
//! The agent member builds it in its provide fn from the kernel's event
//! router; route state holds only this handle, which registers one
//! connection for the user a call names on the A2A, AG-UI or context feed and
//! returns the guard that deregisters it. It cannot publish, enumerate users
//! or read another connection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_events::{
    A2ABroadcaster, AgUiBroadcaster, Broadcaster, ConnectionGuard, ContextBroadcaster, EventRouter,
    EventSender, GenericBroadcaster, ToSse,
};
use systemprompt_identifiers::{ConnectionId, UserId};
use systemprompt_models::{A2AEvent, AgUiEvent, ContextEvent};

/// Opens one user's subscriptions to the A2A, AG-UI and context feeds.
#[derive(Clone)]
pub struct StreamFeeds {
    a2a: Arc<A2ABroadcaster>,
    agui: Arc<AgUiBroadcaster>,
    context: Arc<ContextBroadcaster>,
}

impl std::fmt::Debug for StreamFeeds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamFeeds").finish_non_exhaustive()
    }
}

async fn open<E: ToSse + Clone + Send + Sync + 'static>(
    broadcaster: &Arc<GenericBroadcaster<E>>,
    user_id: &UserId,
    connection: &ConnectionId,
    sender: EventSender,
) -> Option<ConnectionGuard<E>> {
    if !broadcaster.register(user_id, connection, sender).await {
        return None;
    }
    Some(ConnectionGuard::new(
        Arc::clone(broadcaster),
        user_id.clone(),
        connection.clone(),
    ))
}

impl StreamFeeds {
    #[must_use]
    pub fn new(router: &EventRouter) -> Self {
        let broadcasters = router.broadcasters();
        Self {
            a2a: Arc::clone(&broadcasters.a2a),
            agui: Arc::clone(&broadcasters.agui),
            context: Arc::clone(&broadcasters.context),
        }
    }

    pub async fn open_a2a(
        &self,
        user_id: &UserId,
        connection: &ConnectionId,
        sender: EventSender,
    ) -> Option<ConnectionGuard<A2AEvent>> {
        open(&self.a2a, user_id, connection, sender).await
    }

    pub async fn open_agui(
        &self,
        user_id: &UserId,
        connection: &ConnectionId,
        sender: EventSender,
    ) -> Option<ConnectionGuard<AgUiEvent>> {
        open(&self.agui, user_id, connection, sender).await
    }

    pub async fn open_context(
        &self,
        user_id: &UserId,
        connection: &ConnectionId,
        sender: EventSender,
    ) -> Option<ConnectionGuard<ContextEvent>> {
        open(&self.context, user_id, connection, sender).await
    }
}
