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
use axum::http::HeaderMap;
use systemprompt_models::bridge::gateway::{SelfEnrollRequest, SelfEnrollResponse};
use systemprompt_runtime::AppContext;
use systemprompt_users::{
    DEVICE_FINGERPRINT_FOREIGN_USER, DeviceCertService, EnrollDeviceCertServiceParams, UserError,
};

use super::bridge_error::{BridgeError, authenticate_bridge};
use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;

pub async fn enroll_self(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Json(payload): Json<SelfEnrollRequest>,
) -> Result<Json<SelfEnrollResponse>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;

    let cert = DeviceCertService::new(Arc::clone(ctx.user_repository()))
        .enroll_or_reuse(EnrollDeviceCertServiceParams {
            user_id: &claims.user_id,
            fingerprint: &payload.fingerprint,
            label: &payload.label,
        })
        .await
        .map_err(classify_enrolment_error)?;

    let issued = ctx
        .managed_repository()
        .issue_consumer_credential(&cert.id)
        .await
        .map_err(|e| BridgeError::unavailable("device credential issue failed", e))?;

    Ok(Json(SelfEnrollResponse {
        device_id: issued.device_id,
        consumer_id: issued.consumer_id,
        credential: issued.credential,
    }))
}

fn classify_enrolment_error(err: UserError) -> BridgeError {
    match err {
        UserError::Validation(message) if message == DEVICE_FINGERPRINT_FOREIGN_USER => {
            BridgeError::DeviceForeignUser(message)
        },
        UserError::Validation(message) => BridgeError::DeviceRejected(message),
        other => BridgeError::internal("device enrolment failed", other),
    }
}
