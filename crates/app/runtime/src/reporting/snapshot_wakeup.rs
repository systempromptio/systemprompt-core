//! One `PostgreSQL` `LISTEN` relay per process wakes the bounded feedback
//! snapshot streams; the context owns it and joins it on shutdown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgListener;
use systemprompt_database::DbPool;
use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;

const CHANNEL: &str = "feedback_snapshots";
const TICK: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Lazily started relay from the `feedback_snapshots` channel to every
/// subscribed stream.
///
/// The relay is spawned on the first subscription, idles without a listener
/// while nobody is subscribed, and runs until [`SnapshotWakeup::shutdown`]
/// joins it.
#[derive(Debug)]
pub struct SnapshotWakeup {
    hints: watch::Sender<u64>,
    relay: Mutex<Option<JoinHandle<()>>>,
}

impl Default for SnapshotWakeup {
    fn default() -> Self {
        Self {
            hints: watch::channel(0).0,
            relay: Mutex::const_new(None),
        }
    }
}

impl SnapshotWakeup {
    pub async fn subscribe(&self, db: &DbPool) -> watch::Receiver<u64> {
        let receiver = self.hints.subscribe();
        let mut relay = self.relay.lock().await;
        if relay.as_ref().is_none_or(JoinHandle::is_finished) {
            let pool = db.write_pool().as_ref().clone();
            let hints = self.hints.clone();
            *relay = Some(tokio::spawn(run(pool, hints)));
        }
        receiver
    }

    pub async fn shutdown(&self) {
        let Some(handle) = self.relay.lock().await.take() else {
            return;
        };
        handle.abort();
        if let Err(error) = handle.await
            && !error.is_cancelled()
        {
            tracing::warn!(error = %error, "Snapshot wakeup relay ended abnormally");
        }
    }
}

async fn run(pool: PgPool, hints: watch::Sender<u64>) {
    let mut interval = tokio::time::interval(TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut listener = None;
    let mut failing = false;
    loop {
        tokio::select! {
            _tick = interval.tick() => {},
            () = notification(&mut listener) => {
                hints.send_modify(|generation| *generation = generation.wrapping_add(1));
            },
        }
        if hints.receiver_count() == 0 {
            listener = None;
            continue;
        }
        if listener.is_some() {
            continue;
        }
        match tokio::time::timeout(CONNECT_TIMEOUT, connect(&pool)).await {
            Ok(Ok(connected)) => {
                if failing {
                    tracing::info!("Snapshot wakeup listener recovered");
                    failing = false;
                }
                listener = Some(connected);
            },
            Ok(Err(error)) => {
                if !failing {
                    tracing::warn!(
                        error = %error,
                        "Snapshot wakeup listener unavailable; streams fall back to polling"
                    );
                    failing = true;
                }
            },
            Err(_elapsed) => {
                if !failing {
                    tracing::warn!(
                        "Snapshot wakeup listener connect timed out; streams fall back to polling"
                    );
                    failing = true;
                }
            },
        }
    }
}

async fn notification(listener: &mut Option<PgListener>) {
    match listener {
        Some(connection) => {
            if let Err(error) = connection.recv().await {
                tracing::warn!(error = %error, "Snapshot wakeup listener dropped; reconnecting");
                *listener = None;
            }
        },
        None => std::future::pending::<()>().await,
    }
}

async fn connect(pool: &PgPool) -> Result<PgListener, sqlx::Error> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen(CHANNEL).await?;
    Ok(listener)
}
