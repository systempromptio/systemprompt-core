//! Webhook delivery for AG-UI, A2A and task-lifecycle events.
//!
//! The active [`WebhookBroadcaster`] is owned by the [`AgentState`] the
//! composition root builds and reaches every caller through a
//! [`WebhookContext`], which binds it to the user and bearer token of one
//! request. Production wires [`HttpWebhookBroadcaster`]; the test harness
//! injects a recording fake through the same constructor.
//!
//! [`AgentState`]: crate::state::AgentState
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use systemprompt_identifiers::{JwtToken, UserId};
use systemprompt_manifest::Config;
use systemprompt_models::{A2AEvent, AgUiEvent};

pub use super::lifecycle_event::LifecycleEvent;

const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Webhook returned error status {status}: {message}")]
    StatusError { status: u16, message: String },
}

/// `#[async_trait]` is required: the broadcaster is held as
/// `Arc<dyn WebhookBroadcaster>` on the agent state so the harness can inject
/// a recording fake, so the trait must be `dyn`-compatible.
#[async_trait]
pub trait WebhookBroadcaster: Send + Sync + std::fmt::Debug {
    async fn broadcast_agui(
        &self,
        user_id: &UserId,
        event: AgUiEvent,
        auth_token: Option<&JwtToken>,
    ) -> Result<usize, WebhookError>;

    async fn broadcast_a2a(
        &self,
        user_id: &UserId,
        event: A2AEvent,
        auth_token: Option<&JwtToken>,
    ) -> Result<usize, WebhookError>;

    async fn broadcast_lifecycle(
        &self,
        event: LifecycleEvent,
        auth_token: Option<&JwtToken>,
    ) -> Result<(), WebhookError>;
}

pub type DynWebhookBroadcaster = Arc<dyn WebhookBroadcaster>;

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

#[derive(Debug, Clone)]
pub struct HttpWebhookBroadcaster {
    client: Client,
    api_url: String,
}

impl HttpWebhookBroadcaster {
    pub fn new(api_internal_url: &str) -> Result<Self, WebhookError> {
        let client = Client::builder().timeout(WEBHOOK_TIMEOUT).build()?;
        Ok(Self {
            client,
            api_url: api_internal_url.trim_end_matches('/').to_owned(),
        })
    }

    pub fn from_config(config: &Config) -> Result<Self, WebhookError> {
        Self::new(&config.api_internal_url)
    }
}

#[async_trait]
impl WebhookBroadcaster for HttpWebhookBroadcaster {
    async fn broadcast_agui(
        &self,
        user_id: &UserId,
        event: AgUiEvent,
        auth_token: Option<&JwtToken>,
    ) -> Result<usize, WebhookError> {
        let url = format!("{}/api/v1/webhook/agui", self.api_url);
        let event_type = event.event_type();
        if auth_token.is_none() {
            tracing::warn!(
                event_type = ?event_type,
                user_id = %user_id,
                "AGUI broadcast without a bearer token"
            );
        }
        let payload = AgUiWebhookPayload {
            event,
            user_id: user_id.clone(),
        };
        post_and_decode(&self.client, &url, auth_token, &payload, "AGUI").await
    }

    async fn broadcast_a2a(
        &self,
        user_id: &UserId,
        event: A2AEvent,
        auth_token: Option<&JwtToken>,
    ) -> Result<usize, WebhookError> {
        let url = format!("{}/api/v1/webhook/a2a", self.api_url);
        let payload = A2AWebhookPayload {
            event,
            user_id: user_id.clone(),
        };
        post_and_decode(&self.client, &url, auth_token, &payload, "A2A").await
    }

    async fn broadcast_lifecycle(
        &self,
        event: LifecycleEvent,
        auth_token: Option<&JwtToken>,
    ) -> Result<(), WebhookError> {
        let url = format!("{}/api/v1/webhook/broadcast", self.api_url);
        let response = post(&self.client, &url, auth_token, &event).await?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(status_error(response).await)
    }
}

#[derive(serde::Deserialize)]
struct WebhookResponse {
    connection_count: usize,
}

async fn post<T: Serialize + Sync + ?Sized>(
    client: &Client,
    url: &str,
    auth_token: Option<&JwtToken>,
    payload: &T,
) -> Result<reqwest::Response, WebhookError> {
    let request = client.post(url);
    let request = match auth_token {
        Some(token) => request.bearer_auth(token.as_str()),
        None => request,
    };
    Ok(request.json(payload).send().await?)
}

async fn status_error(response: reqwest::Response) -> WebhookError {
    let status = response.status().as_u16();
    let message = response
        .text()
        .await
        .unwrap_or_else(|e| format!("<error reading response: {e}>"));
    WebhookError::StatusError { status, message }
}

async fn post_and_decode<T: Serialize + Sync + ?Sized>(
    client: &Client,
    url: &str,
    auth_token: Option<&JwtToken>,
    payload: &T,
    kind: &str,
) -> Result<usize, WebhookError> {
    let response = post(client, url, auth_token, payload).await?;
    if !response.status().is_success() {
        let error = status_error(response).await;
        tracing::error!(kind, error = %error, "event failed");
        return Err(error);
    }
    let decoded = response.json::<WebhookResponse>().await?;
    tracing::debug!(
        kind,
        connection_count = decoded.connection_count,
        "broadcasted"
    );
    Ok(decoded.connection_count)
}

#[derive(Clone, Debug)]
pub struct WebhookContext {
    broadcaster: DynWebhookBroadcaster,
    user_id: UserId,
    auth_token: Option<JwtToken>,
}

impl WebhookContext {
    pub const fn new(
        broadcaster: DynWebhookBroadcaster,
        user_id: UserId,
        auth_token: Option<JwtToken>,
    ) -> Self {
        Self {
            broadcaster,
            user_id,
            auth_token,
        }
    }

    pub fn for_request(
        broadcaster: DynWebhookBroadcaster,
        context: &systemprompt_models::RequestContext,
    ) -> Self {
        Self::new(
            broadcaster,
            context.user_id().clone(),
            context.auth_token().cloned(),
        )
    }

    pub const fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub const fn auth_token(&self) -> Option<&JwtToken> {
        self.auth_token.as_ref()
    }

    pub async fn broadcast_agui(&self, event: AgUiEvent) -> Result<usize, WebhookError> {
        self.broadcaster
            .broadcast_agui(&self.user_id, event, self.auth_token.as_ref())
            .await
    }

    pub async fn broadcast_a2a(&self, event: A2AEvent) -> Result<usize, WebhookError> {
        self.broadcaster
            .broadcast_a2a(&self.user_id, event, self.auth_token.as_ref())
            .await
    }

    pub async fn broadcast_lifecycle(&self, event: LifecycleEvent) -> Result<(), WebhookError> {
        self.broadcaster
            .broadcast_lifecycle(event, self.auth_token.as_ref())
            .await
    }
}
