//! Outbound Bot Connector client.
//!
//! Replies are posted to the activity's `serviceUrl` at
//! `/v3/conversations/{id}/activities`, authorized with a client-credentials
//! token from [`TokenProvider`].
//!
//! `serviceUrl` is read off the inbound activity payload, so it is chosen by
//! whoever sent the request rather than by the operator. The reply travels on
//! the injected [`GuardedClient`], whose request path rejects a bad scheme, a
//! loopback host or a literal address in a blocked range before the request is
//! built, and whose resolver re-checks every address the hostname resolves to,
//! on the initial connection and on each redirect hop. Production wiring
//! passes a client built from the untrusted policy.
//!
//! Token acquisition is a separate path on a separate client. Its URL is
//! operator-configured —
//! [`BOT_FRAMEWORK_TOKEN_URL`](systemprompt_manifest::services::teams::BOT_FRAMEWORK_TOKEN_URL)
//! or an explicit override
//! via [`TeamsClient::with_endpoints`] — never caller-supplied, so it stays on
//! the plain client the caller injects.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};
use systemprompt_client::GuardedClient;
use systemprompt_identifiers::{TeamsAppId, TeamsConversationId};

use crate::error::{TeamsError, TeamsResult};
use crate::token::TokenProvider;

#[derive(Debug)]
pub struct TeamsClient {
    reply_http: GuardedClient,
    tokens: TokenProvider,
}

impl TeamsClient {
    #[must_use]
    pub fn new(
        http: reqwest::Client,
        reply_http: GuardedClient,
        app_id: TeamsAppId,
        app_password: impl Into<String>,
    ) -> Self {
        let tokens = TokenProvider::new(http, app_id, app_password);
        Self { reply_http, tokens }
    }

    #[must_use]
    pub fn with_endpoints(
        http: reqwest::Client,
        reply_http: GuardedClient,
        app_id: TeamsAppId,
        app_password: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        let tokens = TokenProvider::with_token_url(http, app_id, app_password, token_url);
        Self { reply_http, tokens }
    }

    // JSON: Bot Framework `attachments` — Adaptive Card JSON is the vendor schema.
    pub async fn reply(
        &self,
        service_url: &str,
        conversation_id: &TeamsConversationId,
        attachments: Value,
        now_unix: i64,
    ) -> TeamsResult<()> {
        let url = reply_url(service_url, conversation_id);
        let request = self.reply_http.post(&url)?;
        let token = self.tokens.token(now_unix).await?;
        let body = json!({ "type": "message", "attachments": attachments });
        let resp = request.bearer_auth(token).json(&body).send().await?;
        if resp.status().is_success() {
            return Ok(());
        }
        let status = resp.status();
        let detail = resp
            .text()
            .await
            .unwrap_or_else(|e| format!("<unreadable body: {e}>"));
        Err(TeamsError::Outbound {
            status,
            body: detail,
        })
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
