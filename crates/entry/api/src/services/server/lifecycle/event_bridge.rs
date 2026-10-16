//! Bridges database outbox events into the in-process event bus.
//!
//! The relay takes the **write** pool: `LISTEN`/`NOTIFY`, the outbox insert,
//! and the retention prune all require the primary, and a read pool may be
//! pointed at a standby. `write_pool()` falls back to the read pool when no
//! separate write URL is configured. The `LISTEN` session itself is opened on
//! the session pool, which is the write pool unless `database_session_url`
//! gives a direct connection beside a transaction-mode pooler.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_events::PostgresEventBridge;
use systemprompt_runtime::AppContext;

pub(in crate::services::server) fn start_event_bridge(ctx: &AppContext) {
    let pool = ctx.db_pool().write_pool();

    let instance_id = ctx.config().instance_id.clone();
    let listener_pool = ctx.db_pool().session_pool();
    let handle = PostgresEventBridge::new(pool.as_ref().clone(), instance_id)
        .with_listener_pool(listener_pool.as_ref().clone())
        .start();

    if ctx.event_bridge().set(handle).is_err() {
        tracing::warn!("Event bridge already started; ignoring duplicate start");
    } else {
        tracing::info!("Cross-replica event relay started");
    }
}
