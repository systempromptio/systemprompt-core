//! Slack inbound HTTP surface.
//!
//! Three endpoints, one per Slack inbound surface: `/events` (Events API JSON),
//! `/commands` (slash commands, form-encoded), and `/interactivity` (Block Kit
//! interactions, a JSON `payload` form field). Each handler verifies the
//! request signature over the **raw** body, normalizes the payload, resolves
//! the agent from `services/slack/*.yaml`, acks Slack within its 3-second
//! timeout, and spawns the blocking [`dispatch_messaging`] pipeline — whose
//! reply is rendered to Block Kit and posted back via `chat.postMessage`
//! (events) or the captured `response_url` (commands/interactivity).
//!
//! Config and secrets resolve on demand (the MCP-registry pattern): the app is
//! looked up by workspace id, and the signing secret / bot token are read from
//! the profile secret store. No `AppContext` wiring, no registry struct, no DB.

use std::collections::HashMap;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use systemprompt_config::SecretsBootstrap;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::services::SlackAppConfig;
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::EntityRef;
use systemprompt_slack::client::SlackClient;
use systemprompt_slack::events::{EventsApiEnvelope, InteractionPayload, SlashCommand};
use systemprompt_slack::signature::verify_slack_signature;

use crate::routes::messaging::{
    DispatchOutcome, MessagingInbound, ReplyTarget, dispatch_messaging, http_client,
};

const ISSUER: &str = "https://slack.com";

/// Router for the Slack inbound surface, mounted under `ApiPaths::SLACK_BASE`
/// with no JWT middleware (requests are authenticated by Slack signature).
pub fn slack_router() -> Router<AppContext> {
    Router::new()
        .route("/events", post(handle_events))
        .route("/commands", post(handle_commands))
        .route("/interactivity", post(handle_interactivity))
}

async fn handle_events(State(ctx): State<AppContext>, headers: HeaderMap, body: Bytes) -> Response {
    let Ok(envelope) = serde_json::from_slice::<EventsApiEnvelope>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    match envelope {
        EventsApiEnvelope::UrlVerification { challenge } => {
            if verify_any_app(&headers, &body) {
                challenge.into_response()
            } else {
                StatusCode::UNAUTHORIZED.into_response()
            }
        },
        EventsApiEnvelope::EventCallback { team_id, event } => {
            let Some(app) = resolve_app(team_id.as_str()) else {
                return StatusCode::OK.into_response();
            };
            if !verify_app(&app, &headers, &body) {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            // Drop the bot's own echoes and non-message events to avoid loops.
            if event.bot_id.is_some() || !matches!(event.kind.as_str(), "message" | "app_mention") {
                return StatusCode::OK.into_response();
            }
            let (Some(channel), Some(user)) = (event.channel, event.user) else {
                return StatusCode::OK.into_response();
            };
            let Some(agent) = app.agent_for(channel.as_str()).cloned() else {
                return StatusCode::OK.into_response();
            };
            let inbound = MessagingInbound {
                platform: "slack",
                issuer: ISSUER.to_owned(),
                org_id: team_id.as_str().to_owned(),
                channel_id: channel.as_str().to_owned(),
                external_user_id: user.as_str().to_owned(),
                text: event.text.unwrap_or_default(),
                agent_name: agent,
                entity: EntityRef::SlackWorkspace(team_id),
                reply: ReplyTarget::Channel {
                    id: channel.as_str().to_owned(),
                },
            };
            spawn_reply(ctx, inbound, bot_token(&app));
            StatusCode::OK.into_response()
        },
    }
}

async fn handle_commands(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form = parse_form(&body);
    let Some(cmd) = slash_command_from_form(&form) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(app) = resolve_app(cmd.team_id.as_str()) else {
        return StatusCode::OK.into_response();
    };
    if !verify_app(&app, &headers, &body) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let normalized = cmd.normalize();
    let Some(agent) = app.agent_for(&normalized.routing_key).cloned() else {
        return StatusCode::OK.into_response();
    };
    let inbound = MessagingInbound {
        platform: "slack",
        issuer: ISSUER.to_owned(),
        org_id: normalized.workspace_id.as_str().to_owned(),
        channel_id: normalized.channel_id.as_str().to_owned(),
        external_user_id: normalized.slack_user_id.as_str().to_owned(),
        text: normalized.text,
        agent_name: agent,
        entity: EntityRef::SlackWorkspace(normalized.workspace_id),
        reply: normalized.response_url.map_or_else(
            || ReplyTarget::Channel {
                id: normalized.channel_id.as_str().to_owned(),
            },
            |url| ReplyTarget::Url { url },
        ),
    };
    spawn_reply(ctx, inbound, bot_token(&app));
    StatusCode::OK.into_response()
}

async fn handle_interactivity(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form = parse_form(&body);
    let Some(payload_json) = form.get("payload") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let payload: InteractionPayload = match serde_json::from_str(payload_json) {
        Ok(p) => p,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let Some(app) = resolve_app(payload.team.id.as_str()) else {
        return StatusCode::OK.into_response();
    };
    if !verify_app(&app, &headers, &body) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let channel_id = payload
        .channel
        .as_ref()
        .map_or_else(String::new, |c| c.id.as_str().to_owned());
    let routing_key = if channel_id.is_empty() {
        payload.team.id.as_str().to_owned()
    } else {
        channel_id.clone()
    };
    let Some(agent) = app.agent_for(&routing_key).cloned() else {
        return StatusCode::OK.into_response();
    };
    let text = payload
        .actions
        .first()
        .and_then(|a| a.value.clone())
        .unwrap_or_default();
    let reply = payload.response_url.clone().map_or_else(
        || ReplyTarget::Channel {
            id: channel_id.clone(),
        },
        |url| ReplyTarget::Url { url },
    );
    let inbound = MessagingInbound {
        platform: "slack",
        issuer: ISSUER.to_owned(),
        org_id: payload.team.id.as_str().to_owned(),
        channel_id,
        external_user_id: payload.user.id.as_str().to_owned(),
        text,
        agent_name: agent,
        entity: EntityRef::SlackWorkspace(payload.team.id),
        reply,
    };
    spawn_reply(ctx, inbound, bot_token(&app));
    StatusCode::OK.into_response()
}

/// Dispatch in the background and post the rendered reply. Spawned so the route
/// can ack Slack within its 3-second timeout.
fn spawn_reply(ctx: AppContext, inbound: MessagingInbound, bot_token: Option<String>) {
    tokio::spawn(async move {
        let (text, ephemeral) = match dispatch_messaging(&ctx, inbound.clone()).await {
            Ok(DispatchOutcome::Replied(reply)) => (non_empty(reply), false),
            Ok(DispatchOutcome::Denied(reason)) => (format!("⛔ {reason}"), true),
            Err(err) => {
                tracing::error!(error = %err, "slack dispatch failed");
                (
                    "Sorry — something went wrong handling that.".to_owned(),
                    true,
                )
            },
        };
        let blocks = systemprompt_slack::blockkit::render_blocks(&text);
        let result = match &inbound.reply {
            ReplyTarget::Channel { id } => {
                let Some(token) = bot_token else {
                    tracing::warn!(channel = %id, "no slack bot token configured; cannot post reply");
                    return;
                };
                SlackClient::new(http_client(), token)
                    .post_message(id, blocks)
                    .await
            },
            ReplyTarget::Url { url } => {
                SlackClient::new(http_client(), String::new())
                    .respond(url, blocks, ephemeral)
                    .await
            },
        };
        if let Err(err) = result {
            tracing::error!(error = %err, "failed to post slack reply");
        }
    });
}

fn non_empty(text: String) -> String {
    if text.trim().is_empty() {
        "_(no response)_".to_owned()
    } else {
        text
    }
}

fn resolve_app(workspace_id: &str) -> Option<SlackAppConfig> {
    let config = ConfigLoader::load().ok()?;
    config
        .slack_apps
        .into_values()
        .find(|app| app.enabled && app.workspace_id.as_str() == workspace_id)
}

fn signing_secret(app: &SlackAppConfig) -> Option<String> {
    SecretsBootstrap::get()
        .ok()?
        .get(app.signing_secret_ref.as_str())
        .cloned()
}

fn bot_token(app: &SlackAppConfig) -> Option<String> {
    SecretsBootstrap::get()
        .ok()?
        .get(app.bot_token_ref.as_str())
        .cloned()
}

fn verify_app(app: &SlackAppConfig, headers: &HeaderMap, body: &[u8]) -> bool {
    let Some(secret) = signing_secret(app) else {
        return false;
    };
    verify_signature(headers, body, secret.as_bytes())
}

/// Verify against any configured app's signing secret — used for the
/// `url_verification` handshake, which carries no workspace id to disambiguate.
fn verify_any_app(headers: &HeaderMap, body: &[u8]) -> bool {
    let Ok(config) = ConfigLoader::load() else {
        return false;
    };
    config
        .slack_apps
        .values()
        .filter(|app| app.enabled)
        .filter_map(signing_secret)
        .any(|secret| verify_signature(headers, body, secret.as_bytes()))
}

fn verify_signature(headers: &HeaderMap, body: &[u8], secret: &[u8]) -> bool {
    let timestamp = header_str(headers, "x-slack-request-timestamp");
    let signature = header_str(headers, "x-slack-signature");
    let (Some(timestamp), Some(signature)) = (timestamp, signature) else {
        return false;
    };
    verify_slack_signature(secret, timestamp, signature, body, now_unix()).is_ok()
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

fn parse_form(body: &[u8]) -> HashMap<String, String> {
    url::form_urlencoded::parse(body)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

fn slash_command_from_form(form: &HashMap<String, String>) -> Option<SlashCommand> {
    let json = serde_json::json!({
        "command": form.get("command")?,
        "text": form.get("text").cloned().unwrap_or_default(),
        "user_id": form.get("user_id")?,
        "channel_id": form.get("channel_id")?,
        "team_id": form.get("team_id")?,
        "response_url": form.get("response_url")?,
    });
    serde_json::from_value(json).ok()
}

/// Test-only re-exports of the pure form-parsing helpers, forwarded to their
/// private definitions above. Compiled only under `test-api`; the
/// router-driven handlers stay black-boxed in production builds.
#[cfg(feature = "test-api")]
pub mod test_api {
    use std::collections::HashMap;

    use systemprompt_slack::events::SlashCommand;

    #[must_use]
    pub fn parse_form(body: &[u8]) -> HashMap<String, String> {
        super::parse_form(body)
    }

    #[must_use]
    pub fn slash_command_from_form(form: &HashMap<String, String>) -> Option<SlashCommand> {
        super::slash_command_from_form(form)
    }
}
