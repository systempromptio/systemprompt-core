//! [`AguiChannel`]: `notification.created` as an AG-UI `CUSTOM` event on the
//! recipient's own stream, relayed to other replicas. It is never published
//! on a topic, so a document's live viewers do not see who was notified.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_events::{EventRouter, RelayOutcome};
use systemprompt_extension::notifications::{ChannelError, NotificationChannel};
use systemprompt_models::agui::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};
use systemprompt_models::notifications::{
    Attention, NOTIFICATION_CREATED, Notification, NotificationCreatedPayload, SURFACE_AGUI,
};

/// Delivers every notification to its recipient's AG-UI stream.
#[derive(Debug, Clone)]
pub struct AguiChannel {
    router: Arc<EventRouter>,
}

impl AguiChannel {
    #[must_use]
    pub const fn new(router: Arc<EventRouter>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl NotificationChannel for AguiChannel {
    fn surface(&self) -> &'static str {
        SURFACE_AGUI
    }

    fn delivers(&self, _attention: Attention) -> bool {
        true
    }

    async fn deliver(&self, notification: &Notification, unread: i64) -> Result<(), ChannelError> {
        let value = serde_json::to_value(NotificationCreatedPayload {
            notification: notification.clone(),
            unread,
        })?;
        let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
            name: NOTIFICATION_CREATED.to_owned(),
            value,
        }));
        let outcome = self.router.route_agui(&notification.recipient, event).await;
        match outcome.relay {
            RelayOutcome::Failed(error) => Err(ChannelError::Other {
                surface: SURFACE_AGUI,
                source: Box::new(error),
            }),
            RelayOutcome::NotInstalled | RelayOutcome::Relayed => Ok(()),
        }
    }
}
