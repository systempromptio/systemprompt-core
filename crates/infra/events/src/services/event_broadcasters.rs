//! The [`EventBroadcasters`] set: one fan-out point per event kind, built at
//! the composition root and shared by the router, the cross-replica bridge
//! and the SSE endpoints; re-exported through `routing`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::{
    A2ABroadcaster, AgUiBroadcaster, AnalyticsBroadcaster, ContextBroadcaster, TopicBroadcaster,
};
use crate::Broadcaster;

/// The fan-out points for every event kind, built once and shared by the
/// router, the cross-replica bridge and the SSE endpoints of one process.
#[derive(Debug, Default)]
pub struct EventBroadcasters {
    pub context: Arc<ContextBroadcaster>,
    pub agui: Arc<AgUiBroadcaster>,
    pub a2a: Arc<A2ABroadcaster>,
    pub analytics: Arc<AnalyticsBroadcaster>,
    pub topics: Arc<TopicBroadcaster>,
}

impl EventBroadcasters {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn total_connections(&self) -> BroadcasterConnections {
        BroadcasterConnections {
            context: self.context.total_connections().await,
            agui: self.agui.total_connections().await,
            a2a: self.a2a.total_connections().await,
            analytics: self.analytics.total_connections().await,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BroadcasterConnections {
    pub context: usize,
    pub agui: usize,
    pub a2a: usize,
    pub analytics: usize,
}
