//! Microsoft Teams inbound HTTP surface.
//!
//! One endpoint, `/messages`, receives every Bot Framework activity. The
//! handler validates the activity's `Authorization` bearer (an RS256 JWT from
//! the Bot Connector, bound to the activity's `serviceUrl`), normalizes the
//! activity, resolves the agent from `services/teams/*.yaml`, acks the Bot
//! Service, and spawns the dispatch through the shared `MessagingDispatcher`,
//! whose reply is rendered to an Adaptive Card and posted back to the
//! conversation via the Bot Connector.
//!
//! Config and secrets resolve on demand: the app is looked up by tenant id in
//! the live services view, and the app password is read by its named ref
//! through the platform secret lookup (built-in and custom names). The router
//! state ([`TeamsState`]) holds the one outbound HTTP client and a verifier per
//! app, so JWKS fetches are cached rather than repeated on every activity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod state;

pub use state::{TeamsState, TeamsStateError};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionContext, ExtensionRouter, HandlerVerifier, RateBudget,
    RateLimit, RouteGroup,
};
use systemprompt_identifiers::{TeamsAppId, TeamsConversationId, TeamsTenantId};
use systemprompt_manifest::services::{ServicesConfig, TeamsAppConfig};
use systemprompt_models::modules::ApiPaths;
use systemprompt_teams::activities::Activity;
use systemprompt_teams::client::TeamsClient;
use systemprompt_traits::{
    DispatchOutcome, LinkPolicy, MessagingConversation, MessagingInbound, SecretLookup,
    SenderIdentity,
};

const ISSUER: &str = "https://api.botframework.com";

pub fn declaration(ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
    let state = match TeamsState::from_context(ctx) {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(error = %err, "teams routes not mounted: router state unavailable");
            return None;
        },
    };
    Some(
        ExtensionRouter::new(
            router(state),
            ApiPaths::TEAMS_BASE,
            AuthzPolicy::handler_verified(HandlerVerifier::TeamsBotToken),
            ContextKind::HandlerResolved,
        )
        .in_group(RouteGroup::Messaging)
        .with_rate_limit(RateLimit::new("agents", RateBudget::Agents)),
    )
}

pub fn router(state: TeamsState) -> Router {
    Router::new()
        .route("/messages", post(handle_messages))
        .with_state(state)
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

    let Some(app) = resolve_app(&state.services().config(), &normalized.tenant_id) else {
        return StatusCode::OK.into_response();
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
    let Some(app_password) = app_password(state.secrets(), &app) else {
        tracing::error!(
            tenant = %normalized.tenant_id.as_str(),
            secret = %app.app_password_ref,
            "teams app password secret is not configured"
        );
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let inbound = MessagingInbound {
        issuer: ISSUER.to_owned(),
        conversation: MessagingConversation::Teams {
            tenant_id: normalized.tenant_id,
            conversation_id: normalized.conversation_id.clone(),
            user_id: normalized.teams_user_id,
        },
        text: normalized.text,
        agent_name: agent,
        sender: SenderIdentity::Unlinked,
        link: LinkPolicy::default(),
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
    app_id: TeamsAppId,
    app_password: String,
    token_url: String,
}

fn spawn_reply(state: TeamsState, inbound: MessagingInbound, reply: TeamsReply) {
    let background = state.background().clone();
    background.spawn("teams_reply", async move {
        let text = match Box::pin(state.dispatch(inbound)).await {
            Ok(DispatchOutcome::Replied(reply)) => non_empty(reply),
            Ok(DispatchOutcome::Denied(reason)) => format!("⛔ {reason}"),
            Err(err) => {
                tracing::error!(error = ?err, "teams dispatch failed");
                err.user_message()
            },
        };
        let attachments = systemprompt_teams::cards::render_card(&text);
        let client = TeamsClient::with_endpoints(
            state.http().clone(),
            state.service_url_client().clone(),
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

fn resolve_app(services: &ServicesConfig, tenant_id: &TeamsTenantId) -> Option<TeamsAppConfig> {
    services
        .teams_apps
        .values()
        .find(|app| app.enabled && app.tenant_id == *tenant_id)
        .cloned()
}

fn app_password(secrets: &dyn SecretLookup, app: &TeamsAppConfig) -> Option<String> {
    secrets
        .lookup(app.app_password_ref.as_str())
        .map(|secret| secret.expose().to_owned())
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}
