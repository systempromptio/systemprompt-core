use anyhow::{anyhow, Context, Result};
use futures::stream::{Stream, StreamExt};
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use std::pin::Pin;
use systemprompt_models::modules::ApiPaths;

use super::types::{CheckoutEvent, ProvisioningEvent};
use super::CloudApiClient;

impl CloudApiClient {
    pub fn subscribe_provisioning_events(
        &self,
        tenant_id: &str,
    ) -> Pin<Box<dyn Stream<Item = Result<ProvisioningEvent>> + Send + '_>> {
        let url = format!("{}{}", self.api_url(), ApiPaths::tenant_events(tenant_id));
        let token = self.token().to_string();

        let stream = async_stream::stream! {
            let request = Client::new()
                .get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "text/event-stream");

            let mut es = EventSource::new(request).context("Failed to create SSE connection")?;

            while let Some(event) = es.next().await {
                match event {
                    Ok(Event::Open) => {
                        tracing::debug!("SSE connection opened");
                    }
                    Ok(Event::Message(message)) => {
                        if message.event == "provisioning" || message.event == "message" {
                            match serde_json::from_str::<ProvisioningEvent>(&message.data) {
                                Ok(event) => yield Ok(event),
                                Err(e) => {
                                    tracing::warn!(error = %e, data = %message.data, "Failed to parse SSE event");
                                }
                            }
                        } else if message.event == "heartbeat" {
                            tracing::trace!("SSE heartbeat received");
                        }
                    }
                    Err(reqwest_eventsource::Error::StreamEnded) => {
                        tracing::debug!("SSE stream ended normally");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "SSE stream error");
                        yield Err(anyhow!("SSE stream error: {}", e));
                        break;
                    }
                }
            }
        };

        Box::pin(stream)
    }

    pub fn subscribe_checkout_events(
        &self,
        checkout_session_id: &str,
    ) -> Pin<Box<dyn Stream<Item = Result<CheckoutEvent>> + Send + '_>> {
        let url = format!(
            "{}/api/v1/checkout/{}/events",
            self.api_url(),
            checkout_session_id
        );
        let token = self.token().to_string();

        let stream = async_stream::stream! {
            tracing::debug!(url = %url, "Building SSE request");
            let request = Client::new()
                .get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "text/event-stream");

            let mut es = match EventSource::new(request) {
                Ok(es) => es,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create EventSource");
                    yield Err(anyhow!("Failed to create SSE connection: {}", e));
                    return;
                }
            };

            while let Some(event) = es.next().await {
                match event {
                    Ok(Event::Open) => {
                        tracing::debug!("SSE connection opened");
                    }
                    Ok(Event::Message(message)) => {
                        tracing::debug!(event_type = %message.event, "SSE message received");
                        if message.event == "provisioning" {
                            match serde_json::from_str::<CheckoutEvent>(&message.data) {
                                Ok(event) => yield Ok(event),
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to parse checkout event");
                                }
                            }
                        }
                    }
                    Err(reqwest_eventsource::Error::StreamEnded) => {
                        tracing::debug!("SSE stream ended");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "SSE stream error");
                        yield Err(anyhow!("SSE stream error: {}", e));
                        break;
                    }
                }
            }
        };

        Box::pin(stream)
    }
}
