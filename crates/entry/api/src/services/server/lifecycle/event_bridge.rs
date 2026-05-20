use systemprompt_events::PostgresEventBridge;
use systemprompt_runtime::AppContext;

pub fn start_event_bridge(ctx: &AppContext) {
    let Some(pool) = ctx.db_pool().get_postgres_pool() else {
        tracing::info!("No Postgres pool; cross-replica event relay disabled");
        return;
    };

    let handle = PostgresEventBridge::new(pool.as_ref().clone()).start();

    if ctx.event_bridge().set(handle).is_err() {
        tracing::warn!("Event bridge already started; ignoring duplicate start");
    } else {
        tracing::info!("Cross-replica event relay started");
    }
}
