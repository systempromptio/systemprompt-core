//! The calling device's push token and the admin device list.
//!
//! `PUT /push-token` registers or refreshes the token of the caller's own
//! device and records what the device reported about itself;
//! `DELETE /push-token` removes it, which a device does when it signs out.
//! Neither names a device id. The authenticated principal carries a user and
//! no device, so the caller proves which device it is with the
//! [`DEVICE_FINGERPRINT`] header: the fingerprint that device was
//! enrolled under. The users member resolves it to the active device record
//! of the caller and refuses an unknown fingerprint, a revoked device and
//! another user's device alike with 403 `device_not_enrolled`.
//!
//! `GET /devices` on the admin surface lists device records, newest first,
//! optionally for one user, with platform, versions and the provider of the
//! live token. A token is never listed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;

pub use error::DevicesHttpError;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;
use systemprompt_identifiers::headers::DEVICE_FINGERPRINT;
use systemprompt_models::RequestContext;
use systemprompt_models::devices::AdminDeviceRow;
use systemprompt_models::notifications::{
    PushTokenRegistration, PushTokenRevocation, RegisterPushTokenRequest,
};

use super::NotificationsState;

pub(super) const PUSH_TOKEN_ROUTE: &str = "/push-token";
pub(super) const ADMIN_DEVICES_ROUTE: &str = "/devices";

/// The admin device list reply.
#[derive(Debug, Serialize)]
pub struct AdminDeviceList {
    pub devices: Vec<AdminDeviceRow>,
}

/// The admin device list filter: every device, or one user's.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminDeviceQuery {
    #[serde(default)]
    pub user_id: Option<UserId>,
}

fn fingerprint(headers: &HeaderMap) -> Result<&str, DevicesHttpError> {
    headers
        .get(DEVICE_FINGERPRINT)
        .ok_or(DevicesHttpError::MissingFingerprint)?
        .to_str()
        .map_err(DevicesHttpError::UnreadableFingerprint)
}

pub(super) async fn register(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    headers: HeaderMap,
    Json(body): Json<RegisterPushTokenRequest>,
) -> Result<Json<PushTokenRegistration>, DevicesHttpError> {
    let registration = state
        .devices
        .register(req_ctx.user_id(), fingerprint(&headers)?, &body)
        .await?;
    Ok(Json(registration))
}

pub(super) async fn revoke(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    headers: HeaderMap,
) -> Result<Json<PushTokenRevocation>, DevicesHttpError> {
    let revocation = state
        .devices
        .revoke(req_ctx.user_id(), fingerprint(&headers)?)
        .await?;
    Ok(Json(revocation))
}

pub(super) async fn list(
    State(state): State<NotificationsState>,
    Query(query): Query<AdminDeviceQuery>,
) -> Result<Json<AdminDeviceList>, DevicesHttpError> {
    let devices = state.devices.list_devices(query.user_id.as_ref()).await?;
    Ok(Json(AdminDeviceList { devices }))
}
