//! Node-scoped schema currency watch.
//!
//! The boot check sees only the ledger as it was when this node started. A
//! newer release that migrates during a rolling update does so after every
//! old node is already serving, so each node re-reads the ledger on a fixed
//! interval and publishes the ahead set on its
//! [`SchemaState`](systemprompt_runtime::SchemaState); `/readyz` answers from
//! it, and turns unready once an ahead migration is `@compat: breaking`. The
//! ledger is shared, so every replica of every role runs the watch and none
//! takes a cluster lock.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_runtime::AppContext;

const SCHEMA_RECHECK_INTERVAL: Duration = Duration::from_secs(60);

pub(in crate::services::server) fn start_schema_watch(ctx: &AppContext) {
    let watched = ctx.clone();
    ctx.background_tasks()
        .spawn_cancellable("schema_watch", move |cancel| async move {
            let mut tick = tokio::time::interval(SCHEMA_RECHECK_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            tick.tick().await;
            loop {
                tokio::select! {
                    () = cancel.cancelled() => break,
                    _ = tick.tick() => {},
                }
                if let Err(error) = systemprompt_runtime::schema_currency::recheck(
                    watched.extension_registry(),
                    watched.db_pool().write(),
                    watched.schema_state(),
                )
                .await
                {
                    tracing::warn!(
                        error = %error,
                        "schema currency recheck failed; keeping the last known state"
                    );
                }
            }
        });
}
