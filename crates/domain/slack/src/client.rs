//! Outbound Slack Web API client.
//!
//! Three outbound paths: `chat.postMessage` for Events-API replies (authorized
//! with the app's bot token), an arbitrary `response_url` POST for slash
//! command / interactivity replies, and `users.info` to read the sender's
//! workspace profile when an app opts into email-based identity linking. Every
//! outbound URL passes the shared SSRF guard [`validate_outbound_url`] before a
//! request is made, so a malicious or mistyped `response_url` cannot be turned
//! into an internal request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};
use systemprompt_models::net::validate_outbound_url;

use crate::error::{SlackError, SlackResult};

const CHAT_POST_MESSAGE_URL: &str = "https://slack.com/api/chat.postMessage";
const USERS_INFO_URL: &str = "https://slack.com/api/users.info";

/// The subset of a Slack user's workspace profile the identity mapping needs.
#[derive(Debug, Clone, Default)]
pub struct SlackUserProfile {
    pub email: Option<String>,
    pub email_confirmed: bool,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SlackClient {
    http: reqwest::Client,
    bot_token: String,
    post_message_url: String,
    users_info_url: String,
}

impl SlackClient {
    #[must_use]
    pub fn new(http: reqwest::Client, bot_token: impl Into<String>) -> Self {
        Self {
            http,
            bot_token: bot_token.into(),
            post_message_url: CHAT_POST_MESSAGE_URL.to_owned(),
            users_info_url: USERS_INFO_URL.to_owned(),
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
            users_info_url: USERS_INFO_URL.to_owned(),
        }
    }

    #[cfg(feature = "test")]
    #[must_use]
    pub fn with_users_info_url(mut self, users_info_url: impl Into<String>) -> Self {
        self.users_info_url = users_info_url.into();
        self
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

    // Why: a sender whose workspace profile cannot be read is simply unlinked —
    // the caller degrades to empty claims. Returning the profile rather than a
    // resolved identity keeps this crate out of the user model.
    pub async fn user_info(&self, user_id: &str) -> SlackResult<SlackUserProfile> {
        validate_outbound_url(&self.users_info_url)
            .map_err(|e| SlackError::OutboundUrl(e.to_string()))?;
        let resp = self
            .http
            .get(&self.users_info_url)
            .bearer_auth(&self.bot_token)
            .query(&[("user", user_id)])
            .send()
            .await?;
        let payload = Self::parse_ok(resp).await?;
        let user = payload.get("user");
        let profile = user.and_then(|u| u.get("profile"));
        Ok(SlackUserProfile {
            email: profile
                .and_then(|p| p.get("email"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            email_confirmed: user
                .and_then(|u| u.get("is_email_confirmed"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            display_name: profile
                .and_then(|p| p.get("real_name").or_else(|| p.get("display_name")))
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .map(str::to_owned),
        })
    }

    async fn check_ok(resp: reqwest::Response) -> SlackResult<()> {
        Self::parse_ok(resp).await.map(|_| ())
    }

    // Why: Slack returns HTTP 200 with `{"ok": false, "error": "..."}` on logical
    // failures; surface those as errors rather than treating 200 as success.
    async fn parse_ok(resp: reqwest::Response) -> SlackResult<Value> {
        let status = resp.status();
        let payload: Value = resp
            .json()
            .await
            .unwrap_or_else(|_| json!({ "ok": status.is_success() }));
        if payload.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            return Ok(payload);
        }
        let err = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        Err(SlackError::Outbound(err))
    }
}
