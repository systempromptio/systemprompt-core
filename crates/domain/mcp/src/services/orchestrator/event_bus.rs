use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;

use super::events::McpEvent;
use super::handlers::EventHandler;

pub struct EventBus {
    handlers: Vec<Arc<dyn EventHandler>>,
    sender: broadcast::Sender<McpEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);

        Self {
            handlers: Vec::new(),
            sender,
        }
    }

    pub fn register_handler(&mut self, handler: Arc<dyn EventHandler>) {
        self.handlers.push(handler);
    }

    pub async fn publish(&self, event: McpEvent) -> Result<()> {
        let _ = self.sender.send(event.clone());

        for handler in &self.handlers {
            if handler.handles(&event) {
                handler.handle(&event).await?;
            }
        }

        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<McpEvent> {
        self.sender.subscribe()
    }

    pub fn sender(&self) -> broadcast::Sender<McpEvent> {
        self.sender.clone()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("handlers_count", &self.handlers.len())
            .field("sender", &"<broadcast channel>")
            .finish()
    }
}
