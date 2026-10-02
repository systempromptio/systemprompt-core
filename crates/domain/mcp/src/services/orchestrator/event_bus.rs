//! Event bus dispatching MCP orchestration events to registered subscribers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use std::sync::Arc;
use tokio::sync::broadcast;

use super::events::McpEvent;
use super::subscribers::EventSubscriber;

pub struct EventBus {
    subscribers: Vec<Arc<dyn EventSubscriber>>,
    sender: broadcast::Sender<McpEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);

        Self {
            subscribers: Vec::new(),
            sender,
        }
    }

    pub fn register_subscriber(&mut self, subscriber: Arc<dyn EventSubscriber>) {
        self.subscribers.push(subscriber);
    }

    pub async fn publish(&self, event: McpEvent) -> McpDomainResult<()> {
        if self.sender.send(event.clone()).is_err() {
            tracing::debug!("No broadcast subscribers for event");
        }

        for subscriber in &self.subscribers {
            if subscriber.handles(&event) {
                subscriber.handle(&event).await?;
            }
        }

        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<McpEvent> {
        self.sender.subscribe()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("subscribers_count", &self.subscribers.len())
            .field("sender", &"<broadcast channel>")
            .finish()
    }
}
