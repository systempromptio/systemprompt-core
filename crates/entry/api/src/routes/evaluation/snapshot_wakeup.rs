//! One shared `PostgreSQL` listener wakes bounded streams and exits after their
//! guards close.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::LazyLock;
use systemprompt_runtime::AppContext;
use tokio::sync::{Mutex, watch};
static HINTS: LazyLock<watch::Sender<u64>> = LazyLock::new(|| watch::channel(0).0);
static RELAY: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::const_new(None);

pub(super) async fn subscribe(ctx: &AppContext) -> watch::Receiver<u64> {
    {
        let mut relay = RELAY.lock().await;
        if relay
            .as_ref()
            .is_none_or(tokio::task::JoinHandle::is_finished)
            && let Some(pool) = ctx.db_pool().write_pool()
        {
            *relay = Some(tokio::spawn(run(pool.as_ref().clone())));
        }
    }
    HINTS.subscribe()
}
async fn run(pool: sqlx::PgPool) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    let mut listener = None;
    loop {
        tokio::select! {_tick=interval.tick()=>{},()=notification(&mut listener)=>{HINTS.send_modify(|generation|*generation=generation.wrapping_add(1));}}
        if super::snapshot_stream::CONNECTIONS
            .connection_info()
            .await
            .1
            == 0
        {
            break;
        }
        if listener.is_none()
            && let Ok(Ok(connected)) =
                tokio::time::timeout(std::time::Duration::from_secs(2), connect(&pool)).await
        {
            listener = Some(connected);
        }
    }
}
async fn notification(listener: &mut Option<sqlx::postgres::PgListener>) {
    match listener {
        Some(connection) => {
            if connection.recv().await.is_err() {
                *listener = None;
            }
        },
        None => std::future::pending::<()>().await,
    }
}

async fn connect(pool: &sqlx::PgPool) -> Result<sqlx::postgres::PgListener, sqlx::Error> {
    let mut listener = sqlx::postgres::PgListener::connect_with(pool).await?;
    listener.listen("feedback_snapshots").await?;
    Ok(listener)
}
