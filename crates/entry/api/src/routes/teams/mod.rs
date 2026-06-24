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
//! store. No `AppContext` wiring, no registry struct, no DB.

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use systemprompt_config::SecretsBootstrap;
use systemprompt_identifiers::TeamsConversationId;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::services::TeamsAppConfig;
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::EntityRef;
use systemprompt_teams::activities::Activity;
use systemprompt_teams::auth::ActivityTokenVerifier;
use systemprompt_teams::client::TeamsClient;

use crate::routes::messaging::{
    DispatchOutcome, MessagingInbound, ReplyTarget, dispatch_messaging, http_client,
};

/// The Teams (Azure AD / Entra) token issuer namespacing federated user ids.
const ISSUER: &str = "https://api.botframework.com";

/// Router for the Teams inbound surface, mounted under `ApiPaths::TEAMS_BASE`
/// with no JWT middleware (requests are authenticated by the Bot Connector
/// activity token).
pub fn teams_router() -> Router<AppContext> {
    Router::new().route("/messages", post(handle_messages))
}

async fn handle_messages(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Ok(activity) = serde_json::from_slice::<Activity>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    // A non-dispatchable surface (e.g. a typing/event activity) is acked so the
    // Bot Service does not retry.
    let Ok(normalized) = activity.normalize() else {
        return StatusCode::OK.into_response();
    };

    let Some(app) = resolve_app(normalized.tenant_id.as_str()) else {
        return StatusCode::OK.into_response();
    };

    let Some(token) = bearer(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let verifier = activity_verifier(&app.app_id);
    if verifier
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
    let Some(app_password) = app_password(&app) else {
        tracing::warn!(tenant = %normalized.tenant_id.as_str(), "no teams app password configured");
        return StatusCode::OK.into_response();
    };

    let inbound = MessagingInbound {
        platform: "teams",
        issuer: ISSUER.to_owned(),
        org_id: normalized.tenant_id.as_str().to_owned(),
        channel_id: normalized.conversation_id.as_str().to_owned(),
        external_user_id: normalized.teams_user_id.as_str().to_owned(),
        text: normalized.text,
        agent_name: agent,
        entity: EntityRef::TeamsTenant(normalized.tenant_id),
        reply: ReplyTarget::Channel {
            id: normalized.conversation_id.as_str().to_owned(),
        },
    };
    let reply = TeamsReply {
        service_url: normalized.service_url,
        conversation_id: normalized.conversation_id,
        app_id: app.app_id,
        app_password,
    };
    spawn_reply(ctx, inbound, reply);
    StatusCode::OK.into_response()
}

/// Outbound-reply credentials and target captured for the spawned task.
struct TeamsReply {
    service_url: String,
    conversation_id: TeamsConversationId,
    app_id: String,
    app_password: String,
}

fn spawn_reply(ctx: AppContext, inbound: MessagingInbound, reply: TeamsReply) {
    tokio::spawn(async move {
        let text = match dispatch_messaging(&ctx, inbound).await {
            Ok(DispatchOutcome::Replied(reply)) => non_empty(reply),
            Ok(DispatchOutcome::Denied(reason)) => format!("⛔ {reason}"),
            Err(err) => {
                tracing::error!(error = %err, "teams dispatch failed");
                "Sorry — something went wrong handling that.".to_owned()
            },
        };
        let attachments = systemprompt_teams::cards::render_card(&text);
        let client = teams_client(reply.app_id, reply.app_password);
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

fn resolve_app(tenant_id: &str) -> Option<TeamsAppConfig> {
    let config = ConfigLoader::load().ok()?;
    config
        .teams_apps
        .into_values()
        .find(|app| app.enabled && app.tenant_id.as_str() == tenant_id)
}

fn app_password(app: &TeamsAppConfig) -> Option<String> {
    SecretsBootstrap::get()
        .ok()?
        .get(app.app_password_ref.as_str())
        .cloned()
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// Build the inbound activity-token verifier. Under `test-api`, an
/// `SYSTEMPROMPT_TEST_TEAMS_OPENID_URL` override redirects the JWKS fetch to a
/// loopback mock server so a full signed happy-path can be driven without the
/// live Bot Connector metadata. Production always uses the hardcoded endpoint.
fn activity_verifier(app_id: &str) -> ActivityTokenVerifier {
    #[cfg(feature = "test-api")]
    if let Ok(openid_url) = std::env::var("SYSTEMPROMPT_TEST_TEAMS_OPENID_URL") {
        return ActivityTokenVerifier::with_openid_url(http_client(), app_id.to_owned(), openid_url);
    }
    ActivityTokenVerifier::new(http_client(), app_id.to_owned())
}

/// Build the outbound Bot Connector client. Under `test-api`, an
/// `SYSTEMPROMPT_TEST_TEAMS_TOKEN_URL` override redirects client-credentials
/// token acquisition to a loopback mock server. Production always uses the
/// hardcoded login authority.
fn teams_client(app_id: String, app_password: String) -> TeamsClient {
    #[cfg(feature = "test-api")]
    if let Ok(token_url) = std::env::var("SYSTEMPROMPT_TEST_TEAMS_TOKEN_URL") {
        return TeamsClient::with_endpoints(http_client(), app_id, app_password, token_url);
    }
    TeamsClient::new(http_client(), app_id, app_password)
}
