use systemprompt_models::{AgUiEventBuilder, AgUiMessageRole};

use crate::services::a2a_server::streaming::webhook_client::WebhookContext;

pub struct TextStreamState {
    message_started: bool,
    webhook_context: Option<WebhookContext>,
}

impl TextStreamState {
    pub fn new() -> Self {
        Self {
            message_started: false,
            webhook_context: None,
        }
    }

    pub fn with_webhook_context(mut self, context: WebhookContext) -> Self {
        self.webhook_context = Some(context);
        self
    }

    pub async fn handle_text(&mut self, text: String, message_id: &str) {
        let Some(ref webhook_context) = self.webhook_context else {
            return;
        };

        if !self.message_started {
            let start_event =
                AgUiEventBuilder::text_message_start(message_id, AgUiMessageRole::Assistant);
            if let Err(e) = webhook_context.broadcast_agui(start_event).await {
                tracing::error!(error = %e, "Failed to broadcast TEXT_MESSAGE_START");
            }
            self.message_started = true;
        }

        let content_event = AgUiEventBuilder::text_message_content(message_id, &text);
        if let Err(e) = webhook_context.broadcast_agui(content_event).await {
            tracing::error!(error = %e, "Failed to broadcast TEXT_MESSAGE_CONTENT");
        }
    }

    pub async fn finalize(&self, message_id: &str) {
        if self.message_started {
            if let Some(ref webhook_context) = self.webhook_context {
                let end_event = AgUiEventBuilder::text_message_end(message_id);
                if let Err(e) = webhook_context.broadcast_agui(end_event).await {
                    tracing::error!(error = %e, "Failed to broadcast TEXT_MESSAGE_END");
                }
            }
        }
    }
}

impl Default for TextStreamState {
    fn default() -> Self {
        Self::new()
    }
}
