//! [`TopicBroadcaster`]: fan-out keyed by a topic rather than a user.
//!
//! A topic names a shared resource (`document:<id>`), and every subscriber
//! of that topic on this process receives each [`TopicEvent`] published to
//! it, whoever they are; callers gate who may subscribe. Each topic holds
//! one bounded `tokio::sync::broadcast` channel, created on first subscribe
//! and dropped once a publish finds no receiver left. A slow subscriber
//! that falls a full buffer behind sees `Lagged` and must resynchronise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{PoisonError, RwLock};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

pub const TOPIC_BUFFER: usize = 512;

/// One event on a topic: its name and JSON payload, plus an optional
/// resume token that SSE endpoints surface as the event id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopicEvent {
    pub topic: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_id: Option<String>,
    // JSON: the payload's shape belongs to the event named by `name`.
    pub value: serde_json::Value,
}

#[derive(Debug, Default)]
pub struct TopicBroadcaster {
    topics: RwLock<HashMap<String, broadcast::Sender<TopicEvent>>>,
}

impl TopicBroadcaster {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, topic: &str) -> broadcast::Receiver<TopicEvent> {
        let mut topics = self.topics.write().unwrap_or_else(PoisonError::into_inner);
        topics
            .entry(topic.to_owned())
            .or_insert_with(|| broadcast::channel(TOPIC_BUFFER).0)
            .subscribe()
    }

    pub fn publish(&self, event: TopicEvent) -> usize {
        let sender = self
            .topics
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&event.topic)
            .cloned();
        let Some(sender) = sender else {
            return 0;
        };
        let topic = event.topic.clone();
        sender.send(event).unwrap_or_else(|_| {
            self.drop_if_idle(&topic);
            0
        })
    }

    #[must_use]
    pub fn subscriber_count(&self, topic: &str) -> usize {
        self.topics
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(topic)
            .map_or(0, broadcast::Sender::receiver_count)
    }

    fn drop_if_idle(&self, topic: &str) {
        let mut topics = self.topics.write().unwrap_or_else(PoisonError::into_inner);
        if topics
            .get(topic)
            .is_some_and(|sender| sender.receiver_count() == 0)
        {
            topics.remove(topic);
        }
    }
}
