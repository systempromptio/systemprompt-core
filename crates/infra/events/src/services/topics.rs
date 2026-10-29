//! [`TopicBroadcaster`]: fan-out keyed by a topic rather than a user.
//!
//! A topic names a shared resource (`document:<id>`), and every subscriber
//! of that topic on this process receives each [`TopicEvent`] published to
//! it, whoever they are; callers gate who may subscribe. Each topic holds
//! one bounded `tokio::sync::broadcast` channel, created on first subscribe
//! and dropped once a publish finds no receiver left. A slow subscriber
//! that falls a full buffer behind sees `Lagged` and must resynchronise.
//! A topic subscribed with [`TopicBroadcaster::subscribe_resumable`] carries
//! resumable events: when the cross-replica relay lost events it cannot
//! replay, every such topic receives one [`RELAY_LOST_EVENT`], and its
//! subscribers must resynchronise the same way. Topics only ever subscribed
//! with [`TopicBroadcaster::subscribe`] never see that event.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{PoisonError, RwLock};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

pub const TOPIC_BUFFER: usize = 512;

pub const RELAY_LOST_EVENT: &str = "topic.relay_lost";

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

#[derive(Debug)]
struct TopicChannel {
    sender: broadcast::Sender<TopicEvent>,
    resumable: bool,
}

#[derive(Debug, Default)]
pub struct TopicBroadcaster {
    topics: RwLock<HashMap<String, TopicChannel>>,
}

impl TopicBroadcaster {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, topic: &str) -> broadcast::Receiver<TopicEvent> {
        self.subscribe_as(topic, false)
    }

    pub fn subscribe_resumable(&self, topic: &str) -> broadcast::Receiver<TopicEvent> {
        self.subscribe_as(topic, true)
    }

    fn subscribe_as(&self, topic: &str, resumable: bool) -> broadcast::Receiver<TopicEvent> {
        let mut topics = self.topics.write().unwrap_or_else(PoisonError::into_inner);
        let channel = topics
            .entry(topic.to_owned())
            .or_insert_with(|| TopicChannel {
                sender: broadcast::channel(TOPIC_BUFFER).0,
                resumable,
            });
        channel.resumable |= resumable;
        let receiver = channel.sender.subscribe();
        drop(topics);
        receiver
    }

    pub fn publish(&self, event: TopicEvent) -> usize {
        let sender = self
            .topics
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&event.topic)
            .map(|channel| channel.sender.clone());
        let Some(sender) = sender else {
            return 0;
        };
        let topic = event.topic.clone();
        sender.send(event).unwrap_or_else(|_| {
            self.drop_if_idle(&topic);
            0
        })
    }

    pub fn publish_relay_lost(&self) -> usize {
        self.topics
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|(_, channel)| channel.resumable)
            .map(|(topic, channel)| channel.sender.send(relay_lost_event(topic)).unwrap_or(0))
            .sum()
    }

    #[must_use]
    pub fn subscriber_count(&self, topic: &str) -> usize {
        self.topics
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(topic)
            .map_or(0, |channel| channel.sender.receiver_count())
    }

    fn drop_if_idle(&self, topic: &str) {
        let mut topics = self.topics.write().unwrap_or_else(PoisonError::into_inner);
        if topics
            .get(topic)
            .is_some_and(|channel| channel.sender.receiver_count() == 0)
        {
            topics.remove(topic);
        }
    }
}

#[must_use]
pub fn relay_lost_event(topic: &str) -> TopicEvent {
    TopicEvent {
        topic: topic.to_owned(),
        name: RELAY_LOST_EVENT.to_owned(),
        resume_id: None,
        value: serde_json::Value::Null,
    }
}
