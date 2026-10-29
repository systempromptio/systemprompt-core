//! Re-injecting relayed rows into this replica's broadcasters, and
//! recovering the cross-replica events a bridge missed while its listener
//! was down.
//!
//! `NOTIFY` reaches only a connected listener, so every reconnect replays
//! the outbox rows other replicas wrote since the bridge last knew it was
//! current, less a skew window for rows whose transaction committed after a
//! later row's: an outbox row is stamped with its transaction's start time,
//! so the window is wide enough for a long transaction to commit inside it.
//! Rows already delivered are skipped by id; an id is remembered until its
//! row falls out of every window a later replay can read. When the rows may
//! have been pruned (the gap outlasts the outbox retention), the replay
//! would exceed [`REPLAY_MAX`] rows, or the outbox cannot be read, every
//! resumable topic receives
//! [`RELAY_LOST_EVENT`](super::topics::RELAY_LOST_EVENT) instead, so no gap
//! goes unannounced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{EventOutboxId, UserId};
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, SystemEvent};
use tracing::{error, info, warn};

use super::bridge::PostgresEventBridge;
use super::repository::OutboxRow;
use super::routing::{EventRouter, OutboxChannel};
use super::topics::TopicEvent;

const REPLAY_SKEW: Duration = Duration::from_secs(120);
const REPLAY_MAX: i64 = 10_000;

#[derive(Debug, Default)]
pub(super) struct ReplayCursor {
    since: Option<DateTime<Utc>>,
    connected: bool,
    by_time: BTreeMap<DateTime<Utc>, Vec<EventOutboxId>>,
    seen: HashSet<EventOutboxId>,
}

impl ReplayCursor {
    pub(super) fn first_sighting(&mut self, row: &OutboxRow) -> bool {
        self.since = self.since.max(Some(row.created_at));
        if !self.seen.insert(row.id.clone()) {
            return false;
        }
        self.by_time
            .entry(row.created_at)
            .or_default()
            .push(row.id.clone());
        self.forget_before_window();
        true
    }

    fn advance(&mut self, now: DateTime<Utc>) {
        self.since = self.since.max(Some(now));
        self.forget_before_window();
    }

    fn forget_before_window(&mut self) {
        let Some(since) = self.since else {
            return;
        };
        let kept = self.by_time.split_off(&(since - skew() - skew()));
        for id in std::mem::replace(&mut self.by_time, kept)
            .into_values()
            .flatten()
        {
            self.seen.remove(&id);
        }
    }
}

fn skew() -> chrono::Duration {
    chrono::Duration::from_std(REPLAY_SKEW).unwrap_or_else(|_| chrono::Duration::seconds(120))
}

impl PostgresEventBridge {
    pub(super) async fn mark_current(&self, cursor: &mut ReplayCursor) {
        match self.outbox().now().await {
            Ok(now) => cursor.advance(now - skew()),
            Err(e) => warn!(error = %e, "event bridge: could not read the database clock"),
        }
    }

    pub(super) async fn recover(&self, cursor: &mut ReplayCursor, retention: Duration) {
        let first = !std::mem::replace(&mut cursor.connected, true);
        let Some(since) = cursor.since else {
            if !first {
                self.relay_lost("no replay position");
            }
            self.mark_current(cursor).await;
            return;
        };
        let from = since - skew();
        let horizon =
            chrono::Duration::from_std(retention).unwrap_or_else(|_| chrono::Duration::hours(1));
        let rows = match self.outbox().now().await {
            Ok(now) if now - from < horizon => self.outbox().list_since(from, REPLAY_MAX + 1).await,
            Ok(_) => {
                self.relay_lost("gap outlasted the outbox retention");
                return;
            },
            Err(e) => Err(e),
        };
        let rows = match rows {
            Ok(rows) if i64::try_from(rows.len()).unwrap_or(i64::MAX) <= REPLAY_MAX => rows,
            Ok(_) => {
                self.relay_lost("too many events to replay");
                return;
            },
            Err(e) => {
                warn!(error = %e, "event bridge: could not read the outbox to replay");
                self.relay_lost("outbox unreadable");
                return;
            },
        };
        let mut replayed = 0usize;
        for row in rows {
            if cursor.first_sighting(&row) {
                replayed += 1;
                self.relay_row(row).await;
            }
        }
        info!(
            replayed,
            "event bridge: replayed events missed while reconnecting"
        );
    }

    pub(super) async fn fan_in(
        &self,
        channel: OutboxChannel,
        user_id: &UserId,
        // JSON: outbox payload is polymorphic by channel; decoded into the
        // matching typed event immediately below.
        payload: serde_json::Value,
    ) {
        let local = EventRouter::local_only(Arc::clone(self.broadcasters()));
        match channel {
            OutboxChannel::AgUi => match serde_json::from_value::<AgUiEvent>(payload) {
                Ok(event) => {
                    local.route_agui_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode AG-UI event"),
            },
            OutboxChannel::A2A => match serde_json::from_value::<A2AEvent>(payload) {
                Ok(event) => {
                    local.route_a2a_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode A2A event"),
            },
            OutboxChannel::System => match serde_json::from_value::<SystemEvent>(payload) {
                Ok(event) => {
                    local.route_system_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode system event"),
            },
            OutboxChannel::Analytics => match serde_json::from_value::<AnalyticsEvent>(payload) {
                Ok(event) => {
                    local.route_analytics_local(user_id, event).await;
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode analytics event"),
            },
            OutboxChannel::Topic => match serde_json::from_value::<TopicEvent>(payload) {
                Ok(event) => {
                    local.route_topic_local(event);
                },
                Err(e) => error!(error = %e, "event bridge: failed to decode topic event"),
            },
            OutboxChannel::Fact => {},
        }
    }

    pub(super) fn relay_lost(&self, reason: &'static str) {
        let notified = self.broadcasters().topics.publish_relay_lost();
        warn!(
            reason,
            notified,
            "event bridge: missed events cannot be replayed; topics told to resynchronise"
        );
    }
}
