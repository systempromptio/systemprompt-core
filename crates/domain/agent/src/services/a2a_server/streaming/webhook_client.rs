//! Webhook delivery for AGUI and A2A streaming events.
//!
//! The free [`broadcast_agui_event`] / [`broadcast_a2a_event`] entry points
//! dispatch through a globally installed [`WebhookBroadcaster`]. Production
//! installs nothing and gets the default [`HttpWebhookBroadcaster`]; tests
//! call [`install_for_test`] with a recording fake. The indirection lets
//! the deep callers in `event_loop`, `complete_handler`, `message_handler`,
//! and `skills` stay as free-function calls while the harness still swaps
//! the network for a deterministic spy.

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use systemprompt_identifiers::UserId;
use systemprompt_models::{A2AEvent, AgUiEvent, Config};

#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Webhook returned error status {status}: {message}")]
    StatusError { status: u16, message: String },
}

/// Pluggable transport for AGUI / A2A webhook delivery. Production wires the
/// `HttpWebhookBroadcaster`; tests install a recording fake via
/// [`install_for_test`].
#[async_trait]
pub trait WebhookBroadcaster: Send + Sync + std::fmt::Debug {
    async fn broadcast_agui(
        &self,
        user_id: &UserId,
        event: AgUiEvent,
        auth_token: &str,
    ) -> Result<usize, WebhookError>;

    async fn broadcast_a2a(
        &self,
        user_id: &UserId,
        event: A2AEvent,
        auth_token: &str,
    ) -> Result<usize, WebhookError>;
}

#[derive(Serialize)]
struct AgUiWebhookPayload {
    #[serde(flatten)]
    event: AgUiEvent,
    user_id: UserId,
}

#[derive(Serialize)]
struct A2AWebhookPayload {
    #[serde(flatten)]
    event: A2AEvent,
    user_id: UserId,
}

fn get_api_url() -> String {
    Config::get().map_or_else(
        |_| "http://localhost:3000".to_owned(),
        |c| c.api_internal_url.clone(),
    )
}

/// Default broadcaster: POSTs JSON to the in-tenant API webhook endpoints.
#[derive(Debug, Default, Clone, Copy)]
pub struct HttpWebhookBroadcaster;

#[async_trait]
impl WebhookBroadcaster for HttpWebhookBroadcaster {
    async fn broadcast_agui(
        &self,
        user_id: &UserId,
        event: AgUiEvent,
        auth_token: &str,
    ) -> Result<usize, WebhookError> {
        let url = format!("{}/api/v1/webhook/agui", get_api_url());
        let event_type = event.event_type();
        if auth_token.is_empty() {
            tracing::warn!(
                event_type = ?event_type,
                user_id = %user_id,
                "AGUI broadcast with empty auth_token"
            );
        }
        let payload = AgUiWebhookPayload {
            event,
            user_id: user_id.clone(),
        };
        post_and_decode(&url, auth_token, &payload, "AGUI").await
    }

    async fn broadcast_a2a(
        &self,
        user_id: &UserId,
        event: A2AEvent,
        auth_token: &str,
    ) -> Result<usize, WebhookError> {
        let url = format!("{}/api/v1/webhook/a2a", get_api_url());
        let payload = A2AWebhookPayload {
            event,
            user_id: user_id.clone(),
        };
        post_and_decode(&url, auth_token, &payload, "A2A").await
    }
}

#[derive(serde::Deserialize)]
struct WebhookResponse {
    connection_count: usize,
}

async fn post_and_decode<T: Serialize + Sync + ?Sized>(
    url: &str,
    auth_token: &str,
    payload: &T,
    kind: &str,
) -> Result<usize, WebhookError> {
    let client = Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {auth_token}"))
        .header("Content-Type", "application/json")
        .json(payload)
        .send()
        .await;
    match response {
        Ok(resp) if resp.status().is_success() => match resp.json::<WebhookResponse>().await {
            Ok(r) => {
                tracing::debug!(
                    kind = kind,
                    connection_count = r.connection_count,
                    "broadcasted"
                );
                Ok(r.connection_count)
            },
            Err(e) => {
                tracing::error!(kind = kind, error = %e, "response parse error");
                Err(WebhookError::Request(e))
            },
        },
        Ok(resp) => {
            let status = resp.status().as_u16();
            let message = resp
                .text()
                .await
                .unwrap_or_else(|e| format!("<error reading response: {e}>"));
            tracing::error!(kind = kind, status, message = %message, "event failed");
            Err(WebhookError::StatusError { status, message })
        },
        Err(e) => {
            tracing::error!(kind = kind, error = %e, "request error");
            Err(WebhookError::Request(e))
        },
    }
}

// -----------------------------------------------------------------------------
// Global dispatch
// -----------------------------------------------------------------------------

static GLOBAL_BROADCASTER: OnceLock<Arc<dyn WebhookBroadcaster>> = OnceLock::new();

fn default_broadcaster() -> Arc<dyn WebhookBroadcaster> {
    Arc::new(HttpWebhookBroadcaster)
}

fn active_broadcaster() -> Arc<dyn WebhookBroadcaster> {
    Arc::clone(GLOBAL_BROADCASTER.get_or_init(default_broadcaster))
}

/// Test-only seam.
///
/// Installs `broadcaster` as the process-wide implementation returned by every
/// subsequent `broadcast_*` call. Subsequent calls to `install_for_test` are
/// no-ops — the first caller wins (matching the
/// `RsaSigningKey::install_for_test` / `Config::install` pattern).
pub fn install_for_test(broadcaster: Arc<dyn WebhookBroadcaster>) {
    drop(GLOBAL_BROADCASTER.set(broadcaster));
}

pub async fn broadcast_agui_event(
    user_id: &UserId,
    event: AgUiEvent,
    auth_token: &str,
) -> Result<usize, WebhookError> {
    active_broadcaster()
        .broadcast_agui(user_id, event, auth_token)
        .await
}

pub async fn broadcast_a2a_event(
    user_id: &UserId,
    event: A2AEvent,
    auth_token: &str,
) -> Result<usize, WebhookError> {
    active_broadcaster()
        .broadcast_a2a(user_id, event, auth_token)
        .await
}

#[derive(Clone, Debug)]
pub struct WebhookContext {
    user_id: UserId,
    auth_token: String,
}

impl WebhookContext {
    pub fn new(user_id: UserId, auth_token: impl Into<String>) -> Self {
        Self {
            user_id,
            auth_token: auth_token.into(),
        }
    }

    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub async fn broadcast_agui(&self, event: AgUiEvent) -> Result<usize, WebhookError> {
        broadcast_agui_event(&self.user_id, event, &self.auth_token).await
    }

    pub async fn broadcast_a2a(&self, event: A2AEvent) -> Result<usize, WebhookError> {
        broadcast_a2a_event(&self.user_id, event, &self.auth_token).await
    }
}
