//! Recording fake for `systemprompt_agent::services::a2a_server::streaming::
//! webhook_client::WebhookBroadcaster`. Captures every AGUI / A2A / lifecycle
//! broadcast into an `Arc<Mutex<Vec<…>>>` so tests can assert on emitted
//! events without standing up an HTTP receiver.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::{
    DynWebhookBroadcaster, LifecycleEvent, WebhookBroadcaster, WebhookError,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::{A2AEvent, AgUiEvent};

#[derive(Debug, Clone)]
pub enum RecordedBroadcast {
    AgUi {
        user_id: UserId,
        auth_token: String,
        event: AgUiEvent,
    },
    A2A {
        user_id: UserId,
        auth_token: String,
        event: A2AEvent,
    },
    Lifecycle {
        auth_token: String,
        event: LifecycleEvent,
    },
}

#[derive(Debug, Default)]
pub struct RecordingWebhookBroadcaster {
    records: Mutex<Vec<RecordedBroadcast>>,
    connection_count: usize,
    lifecycle_down: bool,
}

impl RecordingWebhookBroadcaster {
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            connection_count: 1,
            lifecycle_down: false,
        }
    }

    #[must_use]
    pub fn with_connection_count(count: usize) -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            connection_count: count,
            lifecycle_down: false,
        }
    }

    #[must_use]
    pub fn with_lifecycle_down() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            connection_count: 1,
            lifecycle_down: true,
        }
    }

    pub fn lifecycle_events(&self) -> Vec<LifecycleEvent> {
        self.records()
            .into_iter()
            .filter_map(|r| match r {
                RecordedBroadcast::Lifecycle { event, .. } => Some(event),
                _ => None,
            })
            .collect()
    }

    pub fn records(&self) -> Vec<RecordedBroadcast> {
        self.records.lock().expect("lock poisoned").clone()
    }

    pub fn record_count(&self) -> usize {
        self.records.lock().expect("lock poisoned").len()
    }
}

#[async_trait]
impl WebhookBroadcaster for RecordingWebhookBroadcaster {
    async fn broadcast_agui(
        &self,
        user_id: &UserId,
        event: AgUiEvent,
        auth_token: &str,
    ) -> Result<usize, WebhookError> {
        self.records
            .lock()
            .expect("lock poisoned")
            .push(RecordedBroadcast::AgUi {
                user_id: user_id.clone(),
                auth_token: auth_token.to_owned(),
                event,
            });
        Ok(self.connection_count)
    }

    async fn broadcast_a2a(
        &self,
        user_id: &UserId,
        event: A2AEvent,
        auth_token: &str,
    ) -> Result<usize, WebhookError> {
        self.records
            .lock()
            .expect("lock poisoned")
            .push(RecordedBroadcast::A2A {
                user_id: user_id.clone(),
                auth_token: auth_token.to_owned(),
                event,
            });
        Ok(self.connection_count)
    }

    async fn broadcast_lifecycle(
        &self,
        event: LifecycleEvent,
        auth_token: &str,
    ) -> Result<(), WebhookError> {
        self.records
            .lock()
            .expect("lock poisoned")
            .push(RecordedBroadcast::Lifecycle {
                auth_token: auth_token.to_owned(),
                event,
            });
        if self.lifecycle_down {
            return Err(WebhookError::StatusError {
                status: 503,
                message: "broadcast webhook down".to_owned(),
            });
        }
        Ok(())
    }
}

#[must_use]
pub fn arc_recording_broadcaster() -> (DynWebhookBroadcaster, Arc<RecordingWebhookBroadcaster>) {
    let inner = Arc::new(RecordingWebhookBroadcaster::new());
    let dyn_arc: DynWebhookBroadcaster = inner.clone();
    (dyn_arc, inner)
}

#[must_use]
pub fn recording_webhooks() -> DynWebhookBroadcaster {
    arc_recording_broadcaster().0
}
