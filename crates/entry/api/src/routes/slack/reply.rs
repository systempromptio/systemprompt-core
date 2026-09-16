//! The post-ack reply pipeline: run [`dispatch_messaging`] off the request
//! path, optionally attach the sender's confirmed workspace email, render the
//! outcome to Block Kit and post it back through the captured reply target.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::SlackUserId;
use systemprompt_models::services::SlackAppConfig;
use systemprompt_runtime::AppContext;
use systemprompt_slack::client::SlackClient;
use systemprompt_traits::{FederatedIdentityClaims, SenderIdentity};

use super::verify::bot_token;
use crate::routes::messaging::{
    DispatchOutcome, MessagingInbound, ReplyTarget, dispatch_messaging, guarded_http_client,
};

// Why: Slack requires acknowledgment within three seconds.
pub(super) fn spawn_reply(ctx: AppContext, inbound: MessagingInbound, app: &SlackAppConfig) {
    let bot_token = bot_token(app);
    let link_by_email = app.authz.link_by_workspace_email;
    tokio::spawn(async move {
        let mut inbound = inbound;
        if link_by_email && let Some(token) = bot_token.clone() {
            inbound.sender =
                workspace_sender(&token, &SlackUserId::new(inbound.external_user_id.clone())).await;
        }
        let (text, ephemeral) = match dispatch_messaging(&ctx, inbound.clone()).await {
            Ok(DispatchOutcome::Replied(reply)) => (non_empty(reply), false),
            Ok(DispatchOutcome::Denied(reason)) => (format!("⛔ {reason}"), true),
            Err(err) => {
                tracing::error!(error = %err, "slack dispatch failed");
                (err.user_message(), true)
            },
        };
        let blocks = systemprompt_slack::blockkit::render_blocks(&text);
        let Some(http) = guarded_http_client() else {
            tracing::error!("no guarded http client; cannot post slack reply");
            return;
        };
        let result = match &inbound.reply {
            ReplyTarget::Channel { id } => {
                let Some(token) = bot_token else {
                    tracing::warn!(channel = %id, "no slack bot token configured; cannot post reply");
                    return;
                };
                SlackClient::new(http, token).post_message(id, blocks).await
            },
            ReplyTarget::Url { url } => {
                SlackClient::new(http, String::new())
                    .respond(url, blocks, ephemeral)
                    .await
            },
        };
        if let Err(err) = result {
            tracing::error!(error = %err, "failed to post slack reply");
        }
    });
}

async fn workspace_sender(bot_token: &str, slack_user_id: &SlackUserId) -> SenderIdentity {
    let Some(http) = guarded_http_client() else {
        tracing::error!("no guarded http client; cannot read slack profile");
        return SenderIdentity::Unlinked;
    };
    match SlackClient::new(http, bot_token.to_owned())
        .user_info(slack_user_id)
        .await
    {
        Ok(profile) if profile.email_confirmed => SenderIdentity::Linked(FederatedIdentityClaims {
            email: profile.email,
            email_verified: true,
            name: profile.display_name,
            preferred_username: None,
            roles: Vec::new(),
        }),
        Ok(_) => {
            tracing::debug!(
                slack_user_id = slack_user_id.as_str(),
                "slack profile carries no confirmed email"
            );
            SenderIdentity::Unlinked
        },
        Err(err) => {
            tracing::warn!(error = %err, slack_user_id = slack_user_id.as_str(), "slack users.info lookup failed");
            SenderIdentity::Unlinked
        },
    }
}

fn non_empty(text: String) -> String {
    if text.trim().is_empty() {
        "_(no response)_".to_owned()
    } else {
        text
    }
}
