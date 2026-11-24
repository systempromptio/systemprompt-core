//! The hub's `LISTEN` session task: open one session on the session pool
//! for every registered channel, fan its notifications out, add channels as
//! subscriptions name them, and on any loss take the link down and reopen
//! with jittered backoff.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio_util::sync::CancellationToken;

use super::{HubNotification, HubState, ListenLink};

const RETRY_MIN: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(30);

// Why: Postgres rejects LISTEN on a standby with SQLSTATE 25006.
const READ_ONLY_SQL_TRANSACTION: &str = "25006";

fn is_read_only_standby(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => db.code().as_deref() == Some(READ_ONLY_SQL_TRANSACTION),
        _ => false,
    }
}

fn registered(state: &HubState) -> BTreeSet<&'static str> {
    state
        .channels
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

async fn open(pool: &PgPool, channels: &BTreeSet<&'static str>) -> Result<PgListener, sqlx::Error> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.eager_reconnect(false);
    listener.listen_all(channels.iter().copied()).await?;
    Ok(listener)
}

pub(super) async fn run(state: Arc<HubState>, pool: PgPool, cancel: CancellationToken) {
    let mut session = 0u64;
    let mut backoff = RETRY_MIN;
    loop {
        state.link.send_replace(ListenLink::Down);
        let mut channels = registered(&state);
        let mut listener = match open(&pool, &channels).await {
            Ok(listener) => listener,
            Err(error) => {
                report_failure(&error, backoff);
                tokio::select! {
                    () = cancel.cancelled() => break,
                    () = tokio::time::sleep(systemprompt_traits::backoff::jittered(backoff)) => {},
                }
                backoff = (backoff * 2).min(RETRY_MAX);
                continue;
            },
        };
        backoff = RETRY_MIN;
        session += 1;
        state.link.send_replace(ListenLink::Up {
            session,
            channels: Arc::new(channels.clone()),
        });
        tracing::info!(
            session,
            channels = channels.len(),
            "notification hub listening"
        );
        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    state.link.send_replace(ListenLink::Down);
                    return;
                },
                () = state.channels_changed.notified() => {
                    let wanted = registered(&state);
                    let added: Vec<&'static str> = wanted.difference(&channels).copied().collect();
                    if added.is_empty() {
                        continue;
                    }
                    if let Err(error) = listener.listen_all(added.iter().copied()).await {
                        tracing::warn!(%error, "notification hub could not listen on a new channel; reconnecting");
                        break;
                    }
                    channels = wanted;
                    state.link.send_replace(ListenLink::Up {
                        session,
                        channels: Arc::new(channels.clone()),
                    });
                },
                received = listener.try_recv() => match received {
                    Ok(Some(notification)) => {
                        let sent = state.fan_out.send(Arc::new(HubNotification {
                            channel: Arc::from(notification.channel()),
                            payload: Arc::from(notification.payload()),
                        }));
                        if sent.is_err() {
                            tracing::trace!("no subscriber for this notification");
                        }
                    },
                    Ok(None) => {
                        tracing::warn!("notification hub session closed; reconnecting");
                        break;
                    },
                    Err(error) => {
                        tracing::warn!(%error, "notification hub session lost; reconnecting");
                        break;
                    },
                },
            }
        }
    }
    state.link.send_replace(ListenLink::Down);
}

fn report_failure(error: &sqlx::Error, retry_in: Duration) {
    let retry_in_secs = retry_in.as_secs();
    if is_read_only_standby(error) {
        tracing::error!(
            %error,
            retry_in_secs,
            "notification hub: the session pool is a read-only standby; LISTEN requires the \
             primary. Point the session pool (`database_session_url`, else the write pool) at \
             the primary and restart"
        );
    } else {
        tracing::error!(%error, retry_in_secs, "notification hub: failed to open the LISTEN session; retrying");
    }
}
