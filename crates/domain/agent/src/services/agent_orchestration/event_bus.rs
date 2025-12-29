use tokio::sync::broadcast;

use super::events::AgentEvent;

pub struct AgentEventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl AgentEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: AgentEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }

    pub fn sender(&self) -> broadcast::Sender<AgentEvent> {
        self.sender.clone()
    }
}

impl Default for AgentEventBus {
    fn default() -> Self {
        Self::new(100)
    }
}

impl std::fmt::Debug for AgentEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentEventBus")
            .field("sender", &"<broadcast channel>")
            .finish()
    }
}
