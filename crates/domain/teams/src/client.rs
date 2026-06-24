//! Outbound Bot Connector client.
//!
//! Replies are posted to the activity's `serviceUrl` at
//! `/v3/conversations/{id}/activities`, authorized with a client-credentials
//! token from [`TokenProvider`]. Every outbound URL passes the shared SSRF
//! guard before a request is made, so a `serviceUrl` cannot be turned into an
//! internal request even if token validation were ever bypassed.

use serde_json::{Value, json};
use systemprompt_identifiers::TeamsConversationId;
use systemprompt_models::net::validate_outbound_url;

use crate::error::{TeamsError, TeamsResult};
use crate::token::TokenProvider;

/// A client bound to one app registration's outbound credentials.
#[derive(Debug)]
pub struct TeamsClient {
    http: reqwest::Client,
    tokens: TokenProvider,
}

impl TeamsClient {
    #[must_use]
    pub fn new(
        http: reqwest::Client,
        app_id: impl Into<String>,
        app_password: impl Into<String>,
    ) -> Self {
        let tokens = TokenProvider::new(http.clone(), app_id, app_password);
        Self { http, tokens }
    }

    /// Build a client whose outbound token endpoint is overridden, so a test
    /// can mint tokens against a loopback mock server. The Bot Connector reply
    /// target is the activity's `serviceUrl`, supplied per call.
    #[cfg(feature = "test")]
    #[must_use]
    pub fn with_endpoints(
        http: reqwest::Client,
        app_id: impl Into<String>,
        app_password: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        let tokens = TokenProvider::with_token_url(http.clone(), app_id, app_password, token_url);
        Self { http, tokens }
    }

    /// Post a reply (a rendered Adaptive Card `attachments` array) into a
    /// conversation. `now_unix` is forwarded to the token cache.
    pub async fn reply(
        &self,
        service_url: &str,
        conversation_id: &TeamsConversationId,
        attachments: Value,
        now_unix: i64,
    ) -> TeamsResult<()> {
        let url = reply_url(service_url, conversation_id);
        validate_outbound_url(&url).map_err(|e| TeamsError::OutboundUrl(e.to_string()))?;
        let token = self.tokens.token(now_unix).await?;
        let body = json!({ "type": "message", "attachments": attachments });
        let resp = self
            .http
            .post(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await?;
        if resp.status().is_success() {
            return Ok(());
        }
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        Err(TeamsError::Outbound(format!("{status}: {detail}")))
    }
}

/// Build the Bot Connector reply endpoint for a conversation.
#[must_use]
pub fn reply_url(service_url: &str, conversation_id: &TeamsConversationId) -> String {
    format!(
        "{}/v3/conversations/{}/activities",
        service_url.trim_end_matches('/'),
        conversation_id.as_str()
    )
}
