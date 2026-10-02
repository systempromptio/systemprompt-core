//! Microsoft Teams inbound HTTP surface.
//!
//! One endpoint, `/messages`, receives every Bot Framework activity. The
//! handler validates the activity's `Authorization` bearer (an RS256 JWT from
//! the Bot Connector, bound to the activity's `serviceUrl`), normalizes the
//! activity, resolves the agent from `services/teams/*.yaml`, acks the Bot
//! Service, and spawns the blocking [`dispatch_messaging`] pipeline — whose
//! reply is rendered to an Adaptive Card and posted back to the conversation
//! via the Bot Connector.
//!
//! Config and secrets resolve on demand (the MCP-registry pattern): the app is
//! looked up by tenant id, and the app password is read from the profile secret
//! store. The router state ([`TeamsState`]) holds the one outbound HTTP client
//! and a verifier per app, so JWKS fetches are cached rather than repeated on
//! every activity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod state;

pub use state::TeamsState;

use anyhow::Context as _;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use systemprompt_config::SecretsBootstrap;
use systemprompt_identifiers::{TeamsConversationId, TeamsTenantId};
use systemprompt_loader::ConfigLoader;
use systemprompt_models::services::TeamsAppConfig;
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::EntityRef;
use systemprompt_teams::activities::Activity;
use systemprompt_teams::client::TeamsClient;

use crate::routes::messaging::{
    DispatchOutcome, MessagingConversation, MessagingInbound, ReplyTarget, dispatch_messaging,
};

const ISSUER: &str = "https://api.botframework.com";

pub fn teams_router(ctx: &AppContext) -> Result<Router, reqwest::Error> {
    Ok(Router::new()
        .route("/messages", post(handle_messages))
        .with_state(TeamsState::new(ctx)?))
}

async fn handle_messages(
    State(state): State<TeamsState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Ok(activity) = serde_json::from_slice::<Activity>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    // Why: Bot Service retries unacknowledged activities, including typing and
    // event activities.
    let Ok(normalized) = activity.normalize() else {
        return StatusCode::OK.into_response();
    };

    let app = match resolve_app(&normalized.tenant_id) {
        Ok(Some(app)) => app,
        Ok(None) => return StatusCode::OK.into_response(),
        Err(err) => {
            tracing::error!(error = %err, "teams app config unavailable");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        },
    };

    let Some(token) = bearer(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if state
        .verifier(&app)
        .verify(
            token,
            &normalized.service_url,
            chrono::Utc::now().timestamp(),
        )
        .await
        .is_err()
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let Some(agent) = app.agent_for(&normalized.routing_key).cloned() else {
        return StatusCode::OK.into_response();
    };
    let app_password = match app_password(&app) {
        Ok(password) => password,
        Err(err) => {
            tracing::error!(
                tenant = %normalized.tenant_id.as_str(),
                error = %err,
                "teams app password unavailable"
            );
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        },
    };

    let inbound = MessagingInbound {
        issuer: ISSUER.to_owned(),
        conversation: MessagingConversation::Teams {
            tenant_id: normalized.tenant_id.clone(),
            conversation_id: normalized.conversation_id.clone(),
            user_id: normalized.teams_user_id,
        },
        text: normalized.text,
        agent_name: agent,
        entity: EntityRef::TeamsTenant(normalized.tenant_id),
        reply: ReplyTarget::Channel {
            id: normalized.conversation_id.as_str().to_owned(),
        },
        sender: systemprompt_traits::SenderIdentity::Unlinked,
    };
    let reply = TeamsReply {
        service_url: normalized.service_url,
        conversation_id: normalized.conversation_id,
        app_id: app.app_id,
        app_password,
        token_url: app.endpoints.token_url,
    };
    spawn_reply(state, inbound, reply);
    StatusCode::OK.into_response()
}

struct TeamsReply {
    service_url: String,
    conversation_id: TeamsConversationId,
    app_id: String,
    app_password: String,
    token_url: String,
}

fn spawn_reply(state: TeamsState, inbound: MessagingInbound, reply: TeamsReply) {
    let background = state.ctx.background_tasks().clone();
    background.spawn("teams_reply", async move {
        let text = match dispatch_messaging(&state.ctx, inbound).await {
            Ok(DispatchOutcome::Replied(reply)) => non_empty(reply),
            Ok(DispatchOutcome::Denied(reason)) => format!("⛔ {reason}"),
            Err(err) => {
                tracing::error!(error = ?err, "teams dispatch failed");
                err.user_message()
            },
        };
        let attachments = systemprompt_teams::cards::render_card(&text);
        let client = TeamsClient::with_endpoints(
            state.http.clone(),
            reply.app_id,
            reply.app_password,
            reply.token_url,
        );
        if let Err(err) = client
            .reply(
                &reply.service_url,
                &reply.conversation_id,
                attachments,
                chrono::Utc::now().timestamp(),
            )
            .await
        {
            tracing::error!(error = %err, "failed to post teams reply");
        }
    });
}

fn non_empty(text: String) -> String {
    if text.trim().is_empty() {
        "(no response)".to_owned()
    } else {
        text
    }
}

fn resolve_app(tenant_id: &TeamsTenantId) -> anyhow::Result<Option<TeamsAppConfig>> {
    let config = ConfigLoader::load().context("services config")?;
    Ok(config
        .teams_apps
        .into_values()
        .find(|app| app.enabled && app.tenant_id == *tenant_id))
}

fn app_password(app: &TeamsAppConfig) -> anyhow::Result<String> {
    SecretsBootstrap::get()
        .context("secrets store")?
        .get(app.app_password_ref.as_str())
        .cloned()
        .with_context(|| format!("secret {} is not configured", app.app_password_ref))
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}
