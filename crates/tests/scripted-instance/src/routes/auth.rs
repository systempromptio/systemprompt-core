//! Discovery, health, the sign-in exchanges and identity.
//!
//! A harness session is the user's own: a session token is minted from
//! the user's personal access token, and the device-link page approves
//! itself, so both the loopback-callback and the paste-code flow complete
//! without a browser.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::{Map, Value, json};
use systemprompt_identifiers::{SessionId, headers as names};
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::clients::DiscoveryClients;
use systemprompt_models::bridge::gateway::{DevicePatResponse, WhoamiResponse};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::instance::{
    BridgeDiscovery, DISCOVERY_CONTRACT, InstanceBranding,
};
use systemprompt_models::bridge::session::{Capabilities, Health};

use super::{bearer, refuse, session, unauthorized};
use crate::state::{Admitted, InstanceState};
use crate::token::{SESSION_USE, TOKEN_TTL_SECS};

const DISPLAY_NAME: &str = "Scripted Instance";
const LOOPBACK_REDIRECTS: [&str; 2] = ["http://127.0.0.1:", "http://localhost:"];

pub(crate) async fn health() -> Json<Health> {
    Json(Health {
        status: Some("healthy".to_owned()),
        version: Some(env!("CARGO_PKG_VERSION").to_owned()),
    })
}

pub(crate) async fn discovery() -> Json<BridgeDiscovery> {
    Json(BridgeDiscovery {
        contract: DISCOVERY_CONTRACT.to_owned(),
        device_link_path: ApiPaths::BRIDGE_DEVICE_LINK_FALLBACK.to_owned(),
        identity_path: ApiPaths::BRIDGE_WHOAMI.to_owned(),
        branding: InstanceBranding {
            display_name: Some(DISPLAY_NAME.to_owned()),
            ..InstanceBranding::default()
        },
        enabled_hosts: vec![HostKind::Harness],
        clients: DiscoveryClients::default(),
    })
}

pub(crate) async fn capabilities() -> Json<Capabilities> {
    Json(Capabilities {
        modes: vec![
            Capabilities::MODE_PAT.to_owned(),
            Capabilities::MODE_SESSION.to_owned(),
            Capabilities::MODE_OAUTH_CLIENT.to_owned(),
        ],
    })
}

pub(crate) async fn device_link(
    State(state): State<Arc<InstanceState>>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    let Some(redirect) = query.get("redirect") else {
        return Json(json!({ "code": state.issue_code() })).into_response();
    };
    if !LOOPBACK_REDIRECTS
        .iter()
        .any(|origin| redirect.starts_with(origin))
    {
        return refuse(
            StatusCode::BAD_REQUEST,
            "the redirect must be a loopback address",
        );
    }
    let joiner = if redirect.contains('?') { '&' } else { '?' };
    let location = format!("{redirect}{joiner}code={}", state.issue_code());
    (StatusCode::SEE_OTHER, [(LOCATION, location)]).into_response()
}

pub(crate) async fn code_to_pat(
    State(state): State<Arc<InstanceState>>,
    Extension(admitted): Extension<Admitted>,
) -> Response {
    if !redeems(&state, &admitted) {
        return unauthorized("the one-time code is unknown or already used");
    }
    Json(DevicePatResponse {
        pat: state.pat.as_str().to_owned(),
        ui_code: None,
    })
    .into_response()
}

pub(crate) async fn code_to_session(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Extension(admitted): Extension<Admitted>,
) -> Response {
    if !redeems(&state, &admitted) {
        return unauthorized("the one-time code is unknown or already used");
    }
    Json(mint_session(&state, &headers)).into_response()
}

pub(crate) async fn pat_to_session(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
) -> Response {
    if bearer(&headers) != Some(state.pat.as_str()) {
        return unauthorized("the personal access token is not this instance's");
    }
    Json(mint_session(&state, &headers)).into_response()
}

pub(crate) async fn whoami(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(refused) = session(&state, &headers) {
        return refused;
    }
    Json(WhoamiResponse {
        user_id: Some(state.user.user_id.clone()),
        tenant_id: state.user.tenant_id.clone(),
        email: Some(state.user.email.clone()),
        display_name: Some(state.user.display_name.clone()),
        roles: state.user.roles.clone(),
        ..WhoamiResponse::default()
    })
    .into_response()
}

fn redeems(state: &InstanceState, admitted: &Admitted) -> bool {
    admitted
        .json()
        .get("code")
        .and_then(Value::as_str)
        .is_some_and(|code| state.redeem_code(code))
}

fn mint_session(state: &InstanceState, headers: &HeaderMap) -> Value {
    let session_id = headers
        .get(names::SESSION_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|raw| SessionId::try_new(raw).ok())
        .unwrap_or_else(SessionId::generate);
    let mut claims = Map::new();
    claims.insert("sub".to_owned(), json!(state.user.user_id));
    claims.insert("session_id".to_owned(), json!(session_id));
    claims.insert("email".to_owned(), json!(state.user.email));
    claims.insert("roles".to_owned(), json!(state.user.roles));
    claims.insert("iss".to_owned(), json!(state.base_url));
    if let Some(tenant) = &state.user.tenant_id {
        claims.insert("tenant_id".to_owned(), json!(tenant));
    }
    json!({
        "token": state.keys.mint(SESSION_USE, claims),
        "ttl": TOKEN_TTL_SECS,
        "headers": {
            names::SESSION_ID: session_id,
            names::USER_ID: state.user.user_id,
        },
    })
}
