//! The analytics broadcaster as the subscribe-only kernel handle a conferred
//! stream route is handed.
//!
//! [`AnalyticsSubscriptions`] registers a frame subscriber on the shared
//! analytics broadcaster and returns its pre-rendered frames; dropping the
//! returned stream unregisters the connection. It exposes no broadcast and no
//! enumeration of other users' connections.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use systemprompt_identifiers::{ConnectionId, UserId};
use systemprompt_traits::events::{AnalyticsSubscription, SubscribedStream};

use super::broadcaster::{AnalyticsBroadcaster, ConnectionGuard, Subscriber};
use crate::SSE_BUFFER;

#[derive(Debug, Clone)]
pub struct AnalyticsSubscriptions {
    broadcaster: Arc<AnalyticsBroadcaster>,
}

impl AnalyticsSubscriptions {
    #[must_use]
    pub const fn new(broadcaster: Arc<AnalyticsBroadcaster>) -> Self {
        Self { broadcaster }
    }
}

impl AnalyticsSubscription for AnalyticsSubscriptions {
    fn subscribe<'a>(
        &'a self,
        user_id: &'a UserId,
        connection_id: &'a ConnectionId,
    ) -> Pin<Box<dyn Future<Output = Option<SubscribedStream>> + Send + 'a>> {
        let (tx, rx) = tokio::sync::mpsc::channel(SSE_BUFFER);
        let registered =
            self.broadcaster
                .register_now(user_id, connection_id, Subscriber::Frame(tx));
        let stream = registered.then(|| {
            let guard = ConnectionGuard::new(
                Arc::new(self.broadcaster.shared()),
                user_id.clone(),
                connection_id.clone(),
            );
            SubscribedStream::new(rx, Box::new(guard))
        });
        Box::pin(std::future::ready(stream))
    }
}
