//! The governance hooks: the per-user hook client, its
//! `client_credentials` token, telemetry and the scripted verdict.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Form, Json};
use serde_json::{Map, Value, json};
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::hook_events::{GovernAnswer, HookSpecificOutput};
use systemprompt_models::bridge::session::{
    CLIENT_CREDENTIALS_GRANT, HOOK_TOKEN_AUDIENCE, HOOK_TOKEN_SCOPE,
};

use super::{hook, refuse, session};
use crate::state::{Admitted, InstanceState};
use crate::token::{HOOK_USE, TOKEN_TTL_SECS};
use crate::verdict::Verdict;

const HOOK_CLIENT_ID: &str = "sp_scripted_hooks";
const ALLOW: &str = "allow";
const DENY: &str = "deny";

pub(crate) async fn client(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(refused) = session(&state, &headers) {
        return refused;
    }
    let scopes: Vec<&str> = HOOK_TOKEN_SCOPE.split(' ').collect();
    Json(json!({
        "client_id": HOOK_CLIENT_ID,
        "client_secret": state.hook_secret,
        "scopes": scopes,
        "token_endpoint": format!("{}{}", state.base_url, ApiPaths::OAUTH_TOKEN),
    }))
    .into_response()
}

pub(crate) async fn token(
    State(state): State<Arc<InstanceState>>,
    Form(form): Form<BTreeMap<String, String>>,
) -> Response {
    let field = |name: &str| form.get(name).map(String::as_str);
    let granted = field("grant_type") == Some(CLIENT_CREDENTIALS_GRANT)
        && field("client_id") == Some(HOOK_CLIENT_ID)
        && field("client_secret") == Some(state.hook_secret.as_str());
    if !granted {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "invalid_client" })),
        )
            .into_response();
    }
    let mut claims = Map::new();
    claims.insert("sub".to_owned(), json!(state.user.user_id));
    claims.insert("aud".to_owned(), json!(HOOK_TOKEN_AUDIENCE));
    claims.insert("scope".to_owned(), json!(HOOK_TOKEN_SCOPE));
    claims.insert("client_id".to_owned(), json!(HOOK_CLIENT_ID));
    if let Some(plugin) = field("plugin_id") {
        claims.insert("plugin_id".to_owned(), json!(plugin));
    }
    Json(json!({
        "access_token": state.keys.mint(HOOK_USE, claims),
        "token_type": "Bearer",
        "expires_in": TOKEN_TTL_SECS,
        "scope": HOOK_TOKEN_SCOPE,
    }))
    .into_response()
}

pub(crate) async fn track(State(state): State<Arc<InstanceState>>, headers: HeaderMap) -> Response {
    if let Err(refused) = hook(&state, &headers) {
        return refused;
    }
    Json(json!({})).into_response()
}

pub(crate) async fn govern(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Extension(admitted): Extension<Admitted>,
) -> Response {
    if let Err(refused) = hook(&state, &headers) {
        return refused;
    }
    let tool = admitted
        .json()
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match state.verdict.verdict_for(tool) {
        Verdict::Allow => decision(ALLOW, None),
        Verdict::Deny { reason } => decision(DENY, Some(reason.clone())),
        Verdict::Delay { millis } => {
            tokio::time::sleep(Duration::from_millis(*millis)).await;
            decision(ALLOW, None)
        },
        Verdict::Error { status } => refuse(
            StatusCode::from_u16(*status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            "the scripted verdict is an error",
        ),
    }
}

fn decision(permission: &str, reason: Option<String>) -> Response {
    Json(GovernAnswer {
        hook_specific_output: HookSpecificOutput {
            permission_decision: Some(permission.to_owned()),
            permission_decision_reason: reason,
        },
    })
    .into_response()
}
