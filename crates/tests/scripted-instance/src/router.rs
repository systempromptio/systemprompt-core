//! The instance's router: every route a device calls, a layer that
//! records each request before its handler runs, and a fallback that
//! records and refuses the rest.

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use systemprompt_models::ApiPaths;

use crate::record::{RecordedBody, Route};
use crate::routes::{self, auth, hooks, model, sync};
use crate::state::{Arrival, InstanceState};

const BODY_LIMIT: usize = 64 * 1024 * 1024;

pub(crate) fn router(state: Arc<InstanceState>) -> Router {
    Router::new()
        .route(ApiPaths::HEALTH, get(auth::health))
        .route(ApiPaths::WELLKNOWN_BRIDGE, get(auth::discovery))
        .route(ApiPaths::BRIDGE_AUTH_CAPABILITIES, get(auth::capabilities))
        .route(
            ApiPaths::BRIDGE_DEVICE_LINK_FALLBACK,
            get(auth::device_link),
        )
        .route(ApiPaths::BRIDGE_AUTH_SESSION_PAT, post(auth::code_to_pat))
        .route(ApiPaths::BRIDGE_AUTH_SESSION, post(auth::code_to_session))
        .route(ApiPaths::BRIDGE_AUTH_PAT, post(auth::pat_to_session))
        .route(ApiPaths::BRIDGE_WHOAMI, get(auth::whoami))
        .route(ApiPaths::BRIDGE_PUBKEY, get(sync::pubkey))
        .route(ApiPaths::BRIDGE_MANIFEST, get(sync::manifest))
        .route(ApiPaths::GATEWAY_MODELS, get(sync::models))
        .route(ApiPaths::BRIDGE_PROFILE, get(sync::profile))
        .route(ApiPaths::BRIDGE_HEARTBEAT, post(sync::heartbeat))
        .route(ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT, post(hooks::client))
        .route(ApiPaths::OAUTH_TOKEN, post(hooks::token))
        .route(ApiPaths::PUBLIC_HOOKS_TRACK, post(hooks::track))
        .route(ApiPaths::PUBLIC_HOOKS_GOVERN, post(hooks::govern))
        .route(ApiPaths::GATEWAY_MESSAGES, post(model::messages))
        .route(ApiPaths::GATEWAY_CHAT_COMPLETIONS, post(model::chat))
        .fallback(unrouted)
        .layer(middleware::from_fn_with_state(Arc::clone(&state), record))
        .with_state(state)
}

async fn record(State(state): State<Arc<InstanceState>>, request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    let Ok(bytes) = to_bytes(body, BODY_LIMIT).await else {
        return routes::refuse(
            StatusCode::PAYLOAD_TOO_LARGE,
            "the request body is over the scripted instance's limit",
        );
    };
    let recorded = if bytes.is_empty() {
        RecordedBody::Empty
    } else {
        serde_json::from_slice(&bytes).map_or_else(
            |_not_json| RecordedBody::Bytes(bytes.to_vec()),
            RecordedBody::Json,
        )
    };
    let path = parts.uri.path().to_owned();
    let wants_turn =
        Route::of(&path).is_model_call() && routes::holds_model_credential(&state, &parts.headers);
    let admitted = state.admit(
        Arrival {
            method: parts.method.clone(),
            path,
            query: parts.uri.query().map(str::to_owned),
            headers: parts.headers.clone(),
            body: recorded,
        },
        wants_turn,
    );
    let mut request = Request::from_parts(parts, Body::from(bytes));
    request.extensions_mut().insert(admitted);
    next.run(request).await
}

async fn unrouted() -> Response {
    routes::refuse(
        StatusCode::NOT_FOUND,
        "the scripted instance does not serve this route",
    )
}
