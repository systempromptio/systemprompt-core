//! Outbound Bot Connector client.
//!
//! Replies are posted to the activity's `serviceUrl` at
//! `/v3/conversations/{id}/activities`, authorized with a client-credentials
//! token from [`TokenProvider`].
//!
//! `serviceUrl` is read off the inbound activity payload, so it is chosen by
//! whoever sent the request rather than by the operator. It is filtered twice:
//! [`validate_outbound_url`] rejects a bad scheme or a literal address in a
//! blocked range before the request is built, and the reply travels on a
//! guarded client whose resolver re-checks every address the hostname resolves
//! to, on the initial connection and on each redirect hop. A name that
//! resolves into a blocked range is refused at connect time; parse-time
//! validation alone could not see it.
//!
//! Token acquisition is a separate path on a separate client. Its URL is
//! operator-configured —
//! [`BOT_FRAMEWORK_TOKEN_URL`](systemprompt_models::services::teams::BOT_FRAMEWORK_TOKEN_URL)
//! or an explicit override
//! via [`TeamsClient::with_endpoints`] — never caller-supplied, so it stays on
//! the plain client the caller injects.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};
use systemprompt_identifiers::TeamsConversationId;
use systemprompt_models::net::{GuardedClientConfig, guarded_client, validate_outbound_url};

use crate::error::{TeamsError, TeamsResult};
use crate::token::TokenProvider;

#[derive(Debug)]
pub struct TeamsClient {
    reply_http: Option<reqwest::Client>,
    tokens: TokenProvider,
}

fn reply_client() -> Option<reqwest::Client> {
    guarded_client(&GuardedClientConfig::default())
        .inspect_err(|e| tracing::error!(error = %e, "Guarded Teams reply client unavailable"))
        .ok()
}

impl TeamsClient {
    #[must_use]
    pub fn new(
        http: reqwest::Client,
        app_id: impl Into<String>,
        app_password: impl Into<String>,
    ) -> Self {
        let tokens = TokenProvider::new(http, app_id, app_password);
        Self {
            reply_http: reply_client(),
            tokens,
        }
    }

    #[must_use]
    pub fn with_endpoints(
        http: reqwest::Client,
        app_id: impl Into<String>,
        app_password: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        let tokens = TokenProvider::with_token_url(http, app_id, app_password, token_url);
        Self {
            reply_http: reply_client(),
            tokens,
        }
    }

    pub async fn reply(
        &self,
        service_url: &str,
        conversation_id: &TeamsConversationId,
        attachments: Value,
        now_unix: i64,
    ) -> TeamsResult<()> {
        let url = reply_url(service_url, conversation_id);
        validate_outbound_url(&url).map_err(|e| TeamsError::OutboundUrl(e.to_string()))?;
        let http = self
            .reply_http
            .as_ref()
            .ok_or(TeamsError::ClientUnavailable)?;
        let token = self.tokens.token(now_unix).await?;
        let body = json!({ "type": "message", "attachments": attachments });
        let resp = http
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

#[must_use]
pub fn reply_url(service_url: &str, conversation_id: &TeamsConversationId) -> String {
    format!(
        "{}/v3/conversations/{}/activities",
        service_url.trim_end_matches('/'),
        conversation_id.as_str()
    )
}
