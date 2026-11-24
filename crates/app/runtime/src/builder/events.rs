//! Event plane assembly: the router and the idle cross-replica relay.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_events::{EventBridgeHandle, EventRouter, PostgresEventBridge};
use systemprompt_manifest::Config;

pub(crate) fn event_plane(
    configured: Option<EventRouter>,
    database: &systemprompt_database::DbPool,
    config: &Config,
    cache_epochs: &systemprompt_traits::CacheEpochs,
) -> (EventRouter, EventBridgeHandle) {
    let event_router = event_router_or_outbox(configured, database, config);
    // Why: LISTEN/NOTIFY, the outbox insert and the retention prune need the
    // primary, so the relay takes the write pool; it listens through the
    // node's notification hub on the session pool.
    let bridge = PostgresEventBridge::new(
        database.write_pool().as_ref().clone(),
        systemprompt_events::process_origin(&config.instance_id),
        cache_epochs.clone(),
        Arc::clone(event_router.broadcasters()),
        database.notifications().clone(),
    );
    (event_router, bridge.into_idle_handle())
}

fn event_router_or_outbox(
    configured: Option<EventRouter>,
    database: &systemprompt_database::DbPool,
    config: &Config,
) -> EventRouter {
    configured.unwrap_or_else(|| {
        EventRouter::with_outbox(
            database.write_pool().as_ref().clone(),
            systemprompt_events::process_origin(&config.instance_id),
            Arc::new(systemprompt_events::EventBroadcasters::new()),
        )
    })
}
