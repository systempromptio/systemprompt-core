//! Outbound Slack Web API client.
//!
//! Two outbound paths: `chat.postMessage` for Events-API replies (authorized
//! with the app's bot token) and an arbitrary `response_url` POST for slash
//! command / interactivity replies. Every outbound URL passes the shared SSRF
//! guard [`validate_outbound_url`] before a request is made, so a malicious or
//! mistyped `response_url` cannot be turned into an internal request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};
use systemprompt_models::net::validate_outbound_url;

use crate::error::{SlackError, SlackResult};

const CHAT_POST_MESSAGE_URL: &str = "https://slack.com/api/chat.postMessage";

#[derive(Debug, Clone)]
pub struct SlackClient {
    http: reqwest::Client,
    bot_token: String,
    post_message_url: String,
}

impl SlackClient {
    #[must_use]
    pub fn new(http: reqwest::Client, bot_token: impl Into<String>) -> Self {
        Self {
            http,
            bot_token: bot_token.into(),
            post_message_url: CHAT_POST_MESSAGE_URL.to_owned(),
        }
    }

    #[cfg(feature = "test")]
    #[must_use]
    pub fn with_base_url(
        http: reqwest::Client,
        bot_token: impl Into<String>,
        post_message_url: impl Into<String>,
    ) -> Self {
        Self {
            http,
            bot_token: bot_token.into(),
            post_message_url: post_message_url.into(),
        }
    }

    pub async fn post_message(&self, channel: &str, blocks: Value) -> SlackResult<()> {
        validate_outbound_url(&self.post_message_url)
            .map_err(|e| SlackError::OutboundUrl(e.to_string()))?;
        let body = json!({ "channel": channel, "blocks": blocks });
        let resp = self
            .http
            .post(&self.post_message_url)
            .bearer_auth(&self.bot_token)
            .json(&body)
            .send()
            .await?;
        Self::check_ok(resp).await
    }

    pub async fn respond(
        &self,
        response_url: &str,
        blocks: Value,
        ephemeral: bool,
    ) -> SlackResult<()> {
        validate_outbound_url(response_url).map_err(|e| SlackError::OutboundUrl(e.to_string()))?;
        let body = json!({
            "response_type": if ephemeral { "ephemeral" } else { "in_channel" },
            "blocks": blocks,
        });
        let resp = self.http.post(response_url).json(&body).send().await?;
        Self::check_ok(resp).await
    }

    // Why: Slack returns HTTP 200 with `{"ok": false, "error": "..."}` on logical
    // failures; surface those as errors rather than treating 200 as success.
    async fn check_ok(resp: reqwest::Response) -> SlackResult<()> {
        let status = resp.status();
        let payload: Value = resp
            .json()
            .await
            .unwrap_or_else(|_| json!({ "ok": status.is_success() }));
        if payload.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            return Ok(());
        }
        let err = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        Err(SlackError::Outbound(err))
    }
}
