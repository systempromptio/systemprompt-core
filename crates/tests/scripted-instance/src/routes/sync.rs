//! What a signed-in device reads and reports: the manifest and its key,
//! the model list, the profile and the heartbeat.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::header::{ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use systemprompt_models::bridge::heartbeat::HeartbeatResponse;
use systemprompt_models::bridge::instance::WindowStatus;
use systemprompt_models::bridge::profile::{AdvertisedLimits, BridgeProfileResponse};
use systemprompt_models::bridge::session::PubkeyResponse;

use super::{holds_model_credential, refuse, session, unauthorized};
use crate::state::InstanceState;
use crate::{manifest, token};

const OPENAI_FORMAT: &str = "openai";
const MIN_CLIENT_VERSION: &str = "0.0.0";
const LIMITS: AdvertisedLimits = AdvertisedLimits {
    context_window: 200_000,
    max_output_tokens: 8_192,
};

pub(crate) async fn pubkey(State(state): State<Arc<InstanceState>>) -> Json<PubkeyResponse> {
    let key = state.keys.manifest_key(manifest::key_is_rotated(&state));
    Json(PubkeyResponse {
        pubkey: token::public_key(key),
    })
}

pub(crate) async fn manifest(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(refused) = session(&state, &headers) {
        return refused;
    }
    let served = match manifest::serve(&state) {
        Ok(served) => served,
        Err(error) => {
            tracing::error!(%error, "the manifest fixture does not form a manifest");
            return refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the manifest fixture does not form a manifest",
            );
        },
    };
    let unchanged = headers
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|tag| tag == served.etag);
    if unchanged {
        return (StatusCode::NOT_MODIFIED, [(ETAG, served.etag)]).into_response();
    }
    ([(ETAG, served.etag)], Json(served.envelope)).into_response()
}

pub(crate) async fn models(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    if !holds_model_credential(&state, &headers) {
        return unauthorized("a session token or the personal access token is required");
    }
    if query.get("format").map(String::as_str) == Some(OPENAI_FORMAT) {
        let data: Vec<Value> = state
            .models
            .iter()
            .map(|model| json!({ "id": model.id, "object": "model", "created": 0, "owned_by": "scripted" }))
            .collect();
        return Json(json!({ "object": "list", "data": data })).into_response();
    }
    let data: Vec<Value> = state
        .models
        .iter()
        .map(|model| {
            json!({
                "type": "model",
                "id": model.id,
                "display_name": model.display_name.as_deref().unwrap_or(model.id.as_str()),
                "max_input_tokens": LIMITS.context_window,
                "max_tokens": LIMITS.max_output_tokens,
            })
        })
        .collect();
    Json(json!({
        "data": data,
        "has_more": false,
        "first_id": state.models.first().map(|model| &model.id),
        "last_id": state.models.last().map(|model| &model.id),
    }))
    .into_response()
}

pub(crate) async fn profile(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(refused) = session(&state, &headers) {
        return refused;
    }
    Json(BridgeProfileResponse {
        inference_gateway_base_url: state.base_url.clone(),
        auth_scheme: "bearer".to_owned(),
        models: state.models.iter().map(|model| model.id.clone()).collect(),
        default_model: state
            .models
            .first()
            .map(|model| model.id.as_str().to_owned()),
        organization_uuid: None,
        providers: Vec::new(),
        model_limits: state
            .models
            .iter()
            .map(|model| (model.id.clone(), LIMITS))
            .collect(),
        model_names: state
            .models
            .iter()
            .filter_map(|model| Some((model.id.clone(), model.display_name.clone()?)))
            .collect(),
    })
    .into_response()
}

pub(crate) async fn heartbeat(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(refused) = session(&state, &headers) {
        return refused;
    }
    Json(HeartbeatResponse {
        min_bridge_version: MIN_CLIENT_VERSION.to_owned(),
        compatible: true,
        window_status: Some(WindowStatus::Within),
    })
    .into_response()
}
