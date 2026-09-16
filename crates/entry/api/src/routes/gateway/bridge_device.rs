//! `POST /v1/bridge/device` — bridge self-enrolment of a device credential.
//!
//! A bridge that can already authenticate as a user presents a fingerprint
//! derived from its install id and that user; the gateway enrols it (or
//! reuses the active cert holding it) and issues the `sp_device_` consumer
//! credential that attributes installation feedback to the device.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DeviceId, JwtToken, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_users::{
    DEVICE_FINGERPRINT_FOREIGN_USER, DeviceCertService, EnrollDeviceCertServiceParams, UserError,
};

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelfEnrollRequest {
    pub fingerprint: String,
    pub label: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SelfEnrollResponse {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub credential: String,
}

pub async fn enroll_self(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Json(payload): Json<SelfEnrollRequest>,
) -> Result<Json<SelfEnrollResponse>, (StatusCode, String)> {
    let credential = extract_credential(&headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;
    let (claims, _user) = jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let cert = DeviceCertService::new(Arc::clone(ctx.user_repository()))
        .enroll_or_reuse(EnrollDeviceCertServiceParams {
            user_id: &claims.user_id,
            fingerprint: &payload.fingerprint,
            label: &payload.label,
        })
        .await
        .map_err(map_user_error)?;

    let issued = ctx
        .managed_repository()
        .issue_consumer_credential(&cert.id)
        .await
        .map_err(|e| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("device credential issue failed: {e}"),
            )
        })?;

    Ok(Json(SelfEnrollResponse {
        device_id: issued.device_id,
        consumer_id: issued.consumer_id,
        credential: issued.credential,
    }))
}

fn map_user_error(err: UserError) -> (StatusCode, String) {
    match err {
        UserError::Validation(message) if message == DEVICE_FINGERPRINT_FOREIGN_USER => {
            (StatusCode::CONFLICT, message)
        },
        UserError::Validation(message) => (StatusCode::BAD_REQUEST, message),
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("device enrolment failed: {other}"),
        ),
    }
}
