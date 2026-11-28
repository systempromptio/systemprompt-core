//! What an enrolled device calls: enrolment, plugin bundle files,
//! installation plans, receipts and session bindings, over an in-memory
//! store.
//!
//! Enrolment and bundle files take the session token. Plans, receipts and
//! session bindings take the device credential enrolment issued. A plan is
//! served for a published plugin fixture, for the harness host only. A receipt
//! posted again and a binding posted again answer as they did the first time.

use std::sync::Arc;

use axum::Json;
use std::collections::BTreeMap;

use axum::extract::{Path, Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use systemprompt_device::protocol::secret::DEVICE_CREDENTIAL_PREFIX;
use systemprompt_identifiers::{
    ConsumerInstallationId, DeviceId, InstallationReceiptId, InstallationSessionBindingId,
};
use systemprompt_models::bridge::consumer::SessionBinding;
use systemprompt_models::bridge::gateway::{SelfEnrollRequest, SelfEnrollResponse};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::feedback::receipts::{
    ConsumerReceiptRequest, ConsumerReceiptResponse, ReceiptAcknowledgement, SessionBindingRequest,
};

use super::{bearer, refuse, session, unauthorized};
use crate::state::{InstanceState, lock};

const DEVICE_ID_PREFIX: &str = "scripted-device-";
const DEVICE_ID_FINGERPRINT_CHARS: usize = 16;
const OCTET_STREAM: &str = "application/octet-stream";
const HOST_QUERY: &str = "host";

#[derive(Debug, Default)]
pub(crate) struct ConsumerStore {
    credentials: Vec<String>,
    receipts: Vec<(ConsumerInstallationId, InstallationReceiptId)>,
    bindings: Vec<(SessionBindingRequest, SessionBinding)>,
}

fn enrolled(state: &InstanceState, headers: &HeaderMap) -> Result<(), Response> {
    let held = bearer(headers).is_some_and(|credential| {
        lock(&state.consumer)
            .credentials
            .iter()
            .any(|issued| issued == credential)
    });
    if held {
        Ok(())
    } else {
        Err(unauthorized(
            "a device credential this instance issued is required",
        ))
    }
}

pub(crate) async fn enrol(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Json(request): Json<SelfEnrollRequest>,
) -> Response {
    if let Err(refused) = session(&state, &headers) {
        return refused;
    }
    let fingerprint: String = request
        .fingerprint
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(DEVICE_ID_FINGERPRINT_CHARS)
        .collect();
    let Ok(device_id) = DeviceId::try_new(format!("{DEVICE_ID_PREFIX}{fingerprint}")) else {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "a device id could not be formed",
        );
    };
    let credential = format!("{DEVICE_CREDENTIAL_PREFIX}{:032x}", rand::random::<u128>());
    lock(&state.consumer).credentials.push(credential.clone());
    Json(SelfEnrollResponse {
        device_id,
        consumer_id: state.user.user_id.clone(),
        credential,
    })
    .into_response()
}

pub(crate) async fn plugin_file(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Path((plugin, path)): Path<(String, String)>,
) -> Response {
    if let Err(refused) = session(&state, &headers) {
        return refused;
    }
    match state.manifest.plugin_file(&plugin, &path) {
        Some(bytes) => ([(CONTENT_TYPE, OCTET_STREAM)], bytes).into_response(),
        None => refuse(
            StatusCode::NOT_FOUND,
            "the manifest fixture has no such bundle file",
        ),
    }
}

pub(crate) async fn plan(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Path((resource, publication)): Path<(String, String)>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    if let Err(refused) = enrolled(&state, &headers) {
        return refused;
    }
    if query.get(HOST_QUERY).map(String::as_str) != Some(HostKind::Harness.as_str()) {
        return refuse(
            StatusCode::NOT_FOUND,
            "the scripted instance serves plans for the harness host only",
        );
    }
    let Some(plugin) = state.manifest.publication(&resource, &publication) else {
        return refuse(
            StatusCode::NOT_FOUND,
            "the manifest fixture publishes no such resource",
        );
    };
    match plugin.plan() {
        Ok(plan) => Json(plan).into_response(),
        Err(error) => {
            tracing::error!(%error, "a published fixture does not form a plan");
            refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the published fixture does not form a plan",
            )
        },
    }
}

pub(crate) async fn receipt(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Json(receipt): Json<ConsumerReceiptRequest>,
) -> Response {
    if let Err(refused) = enrolled(&state, &headers) {
        return refused;
    }
    if receipt.validate().is_err() {
        return refuse(
            StatusCode::UNPROCESSABLE_ENTITY,
            "the receipt is outside the receipt contract",
        );
    }
    let (receipt_id, acknowledgement) = {
        let mut store = lock(&state.consumer);
        let held = store
            .receipts
            .iter()
            .find(|(installation, _)| *installation == receipt.installation_id)
            .map(|(_, id)| id.clone());
        match held {
            Some(id) => (id, ReceiptAcknowledgement::IdenticalRetry),
            None => {
                let id = InstallationReceiptId::generate();
                store
                    .receipts
                    .push((receipt.installation_id.clone(), id.clone()));
                (id, ReceiptAcknowledgement::Accepted)
            },
        }
    };
    Json(ConsumerReceiptResponse {
        receipt_id,
        acknowledgement,
        acknowledged_at: Utc::now(),
        fully_verified: receipt.fully_verified(),
    })
    .into_response()
}

pub(crate) async fn session_binding(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Json(request): Json<SessionBindingRequest>,
) -> Response {
    if let Err(refused) = enrolled(&state, &headers) {
        return refused;
    }
    let mut store = lock(&state.consumer);
    if !store
        .receipts
        .iter()
        .any(|(_, id)| *id == request.receipt_id)
    {
        return refuse(
            StatusCode::NOT_FOUND,
            "the binding names a receipt this instance did not acknowledge",
        );
    }
    let held = store
        .bindings
        .iter()
        .find(|(bound, _)| *bound == request)
        .map(|(_, binding)| binding.clone());
    let binding = held.unwrap_or_else(|| {
        let binding = SessionBinding {
            id: InstallationSessionBindingId::generate(),
            bound_at: Utc::now(),
        };
        store.bindings.push((request, binding.clone()));
        binding
    });
    drop(store);
    Json(binding).into_response()
}
