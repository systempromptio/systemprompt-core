//! Broadcaster implementations, the static fan-out [`EventRouter`], and the
//! cross-replica [`PostgresEventBridge`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bridge;
mod broadcaster;
mod repository;
mod routing;

pub use bridge::PostgresEventBridge;
pub use broadcaster::{
    A2ABroadcaster, AgUiBroadcaster, AnalyticsBroadcaster, ConnectionGuard, ContextBroadcaster,
    GenericBroadcaster, HEARTBEAT_INTERVAL, HEARTBEAT_JSON, standard_keep_alive,
};
pub use routing::{
    A2A_BROADCASTER, AGUI_BROADCASTER, ANALYTICS_BROADCASTER, CONTEXT_BROADCASTER, EventRouter,
    OUTBOX_CHANNEL, OutboxChannel,
};
