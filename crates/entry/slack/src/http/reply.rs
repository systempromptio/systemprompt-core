//! The post-ack reply pipeline: run the dispatch off the request path,
//! optionally attach the sender's confirmed workspace email, render the
//! outcome to Block Kit and post it back through the captured reply target.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::SlackUserId;
use systemprompt_manifest::services::SlackAppConfig;
use systemprompt_slack::client::SlackClient;
use systemprompt_traits::{
    DispatchOutcome, FederatedIdentityClaims, MessagingConversation, MessagingInbound,
    SenderIdentity,
};

use super::SlackState;
use super::verify::{SlackVerified, bot_token};

#[derive(Debug, Clone)]
pub(super) enum ReplyTarget {
    Channel { id: String },
    Url { url: String },
}

// Why: Slack requires acknowledgment within three seconds.
pub(super) fn spawn_reply(
    state: &SlackState,
    verified: SlackVerified,
    inbound: MessagingInbound,
    reply: ReplyTarget,
    app: &SlackAppConfig,
) {
    let bot_token = bot_token(state.secrets(), app);
    let link_by_email = app.authz.link.link_by_email;
    let state = state.clone();
    let background = state.background().clone();
    background.spawn("slack_reply", async move {
        let http = state.reply_client().clone();
        let mut inbound = inbound;
        if link_by_email
            && let Some(token) = bot_token.clone()
            && let MessagingConversation::Slack { user_id, .. } = &inbound.conversation
        {
            inbound.sender = workspace_sender(&http, &token, user_id).await;
        }
        let (text, ephemeral) = match Box::pin(state.dispatch(verified, inbound)).await {
            Ok(DispatchOutcome::Replied(reply)) => (non_empty(reply), false),
            Ok(DispatchOutcome::Denied(reason)) => (format!("⛔ {reason}"), true),
            Err(err) => {
                tracing::error!(error = ?err, "slack dispatch failed");
                (err.user_message(), true)
            },
        };
        let blocks = systemprompt_slack::blockkit::render_blocks(&text);
        let result = match &reply {
            ReplyTarget::Channel { id } => {
                let Some(token) = bot_token else {
                    tracing::warn!(channel = %id, "no slack bot token configured; cannot post reply");
                    return;
                };
                SlackClient::new(http, token).post_message(id, blocks).await
            },
            ReplyTarget::Url { url } => {
                SlackClient::respond(state.payload_url_client(), url, blocks, ephemeral).await
            },
        };
        if let Err(err) = result {
            tracing::error!(error = %err, "failed to post slack reply");
        }
    });
}

async fn workspace_sender(
    http: &reqwest::Client,
    bot_token: &str,
    slack_user_id: &SlackUserId,
) -> SenderIdentity {
    match SlackClient::new(http.clone(), bot_token.to_owned())
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
