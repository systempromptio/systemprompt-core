//! Broadcaster implementations, the fan-out [`EventRouter`], and the
//! cross-replica [`PostgresEventBridge`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bridge;
mod bridge_handle;
mod bridge_replay;
mod broadcaster;
pub mod cache_invalidation;
pub mod durable;
mod repository;
mod routing;
mod topics;

pub use bridge::PostgresEventBridge;
pub use bridge_handle::{EventBridgeHandle, RelayStatus};
pub use broadcaster::{
    A2ABroadcaster, AgUiBroadcaster, AnalyticsBroadcaster, ConnectionGuard, ContextBroadcaster,
    GenericBroadcaster, HEARTBEAT_INTERVAL, HEARTBEAT_JSON, standard_keep_alive,
};
pub use cache_invalidation::{
    CACHE_INVALIDATION_CHANNEL, CACHE_INVALIDATIONS_RECEIVED_TOTAL, invalidate_all_cache_domains,
    receive_cache_invalidation,
};
pub use repository::EventsOwnerReassignment;
pub use routing::{
    BroadcasterConnections, EventBroadcasters, EventRouter, NOTIFICATION_FACTS_CHANNEL,
    OUTBOX_CHANNEL, OutboxChannel,
};
pub use topics::{RELAY_LOST_EVENT, TOPIC_BUFFER, TopicBroadcaster, TopicEvent, relay_lost_event};
