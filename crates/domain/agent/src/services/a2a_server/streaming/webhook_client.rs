use reqwest::Client;
use serde::Serialize;
use systemprompt_models::{A2AEvent, AgUiEvent, Config};

#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Webhook returned error status {status}: {message}")]
    StatusError { status: u16, message: String },
}

fn get_api_url() -> String {
    Config::get()
        .map(|c| c.api_internal_url.clone())
        .unwrap_or_else(|_| "http://localhost:3000".to_string())
}

pub async fn broadcast_agui_event(
    user_id: &str,
    event: AgUiEvent,
    auth_token: &str,
) -> Result<usize, WebhookError> {
    let url = format!("{}/api/v1/webhook/agui", get_api_url());
    let event_type = event.event_type();

    tracing::debug!(event_type = ?event_type, url = %url, "Sending AGUI event");

    #[derive(Serialize)]
    struct AgUiWebhookPayload {
        #[serde(flatten)]
        event: AgUiEvent,
        user_id: String,
    }

    let payload = AgUiWebhookPayload {
        event,
        user_id: user_id.to_string(),
    };

    let client = Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", auth_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                #[derive(serde::Deserialize)]
                struct WebhookResponse {
                    connection_count: usize,
                }

                match resp.json::<WebhookResponse>().await {
                    Ok(result) => {
                        tracing::debug!(
                            event_type = ?event_type,
                            connection_count = result.connection_count,
                            "AGUI event broadcasted"
                        );
                        Ok(result.connection_count)
                    },
                    Err(e) => {
                        tracing::error!(
                            event_type = ?event_type,
                            error = %e,
                            "AGUI response parse error"
                        );
                        Err(WebhookError::Request(e))
                    },
                }
            } else {
                let status = resp.status().as_u16();
                let message = resp.text().await.unwrap_or_default();
                tracing::error!(
                    event_type = ?event_type,
                    status = status,
                    message = %message,
                    "AGUI event failed"
                );
                Err(WebhookError::StatusError { status, message })
            }
        },
        Err(e) => {
            tracing::error!(event_type = ?event_type, error = %e, "AGUI request error");
            Err(WebhookError::Request(e))
        },
    }
}

pub async fn broadcast_a2a_event(
    user_id: &str,
    event: A2AEvent,
    auth_token: &str,
) -> Result<usize, WebhookError> {
    let url = format!("{}/api/v1/webhook/a2a", get_api_url());
    let event_type = event.event_type();

    tracing::debug!(event_type = ?event_type, url = %url, "Sending A2A event");

    #[derive(Serialize)]
    struct A2AWebhookPayload {
        #[serde(flatten)]
        event: A2AEvent,
        user_id: String,
    }

    let payload = A2AWebhookPayload {
        event,
        user_id: user_id.to_string(),
    };

    let client = Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", auth_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                #[derive(serde::Deserialize)]
                struct WebhookResponse {
                    connection_count: usize,
                }

                match resp.json::<WebhookResponse>().await {
                    Ok(result) => {
                        tracing::debug!(
                            event_type = ?event_type,
                            connection_count = result.connection_count,
                            "A2A event broadcasted"
                        );
                        Ok(result.connection_count)
                    },
                    Err(e) => {
                        tracing::error!(
                            event_type = ?event_type,
                            error = %e,
                            "A2A response parse error"
                        );
                        Err(WebhookError::Request(e))
                    },
                }
            } else {
                let status = resp.status().as_u16();
                let message = resp.text().await.unwrap_or_default();
                tracing::error!(
                    event_type = ?event_type,
                    status = status,
                    message = %message,
                    "A2A event failed"
                );
                Err(WebhookError::StatusError { status, message })
            }
        },
        Err(e) => {
            tracing::error!(event_type = ?event_type, error = %e, "A2A request error");
            Err(WebhookError::Request(e))
        },
    }
}

#[derive(Clone, Debug)]
pub struct WebhookContext {
    user_id: String,
    auth_token: String,
}

impl WebhookContext {
    pub fn new(user_id: impl Into<String>, auth_token: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            auth_token: auth_token.into(),
        }
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub async fn broadcast_agui(&self, event: AgUiEvent) -> Result<usize, WebhookError> {
        broadcast_agui_event(&self.user_id, event, &self.auth_token).await
    }

    pub async fn broadcast_a2a(&self, event: A2AEvent) -> Result<usize, WebhookError> {
        broadcast_a2a_event(&self.user_id, event, &self.auth_token).await
    }
}
