//! [`DevicesHttpError`], the error the device handlers return, and its
//! mapping onto the HTTP error envelope: a missing or unreadable fingerprint
//! header is 400 `device_fingerprint_required`, a malformed one 400
//! `invalid_device_fingerprint`, client info the provider or the record
//! cannot take 400 `invalid_client_info`, and a fingerprint that names no
//! active device of the caller 403 `device_not_enrolled`. Every other
//! failure answers as the users member's error does in the API error model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::header::ToStrError;
use systemprompt_extension::capability_error_response;
use systemprompt_extension::router::IntoApiError;
use systemprompt_models::api::ApiError;
use systemprompt_users::DevicePushError;

/// A device handler's failure, answered through the API error model.
#[derive(Debug, thiserror::Error)]
pub enum DevicesHttpError {
    #[error("the device fingerprint header is required")]
    MissingFingerprint,
    #[error("the device fingerprint header is not visible ASCII")]
    UnreadableFingerprint(#[source] ToStrError),
    #[error(transparent)]
    Device(#[from] DevicePushError),
}

impl IntoApiError for DevicesHttpError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::MissingFingerprint => ApiError::bad_request(self.to_string())
                .with_error_key("device_fingerprint_required"),
            Self::UnreadableFingerprint(source) => {
                ApiError::bad_request("the device fingerprint header is not visible ASCII")
                    .with_error_key("device_fingerprint_required")
                    .with_source(source)
            },
            Self::Device(error) => device(error),
        }
    }
}

capability_error_response!(DevicesHttpError);

fn device(error: DevicePushError) -> ApiError {
    match error {
        DevicePushError::InvalidFingerprint(source) => {
            ApiError::bad_request("device fingerprint is not a SHA-256 hex digest")
                .with_error_key("invalid_device_fingerprint")
                .with_source(source)
        },
        DevicePushError::DeviceNotEnrolled => {
            ApiError::forbidden(error.to_string()).with_error_key("device_not_enrolled")
        },
        DevicePushError::ProviderMismatch { .. } | DevicePushError::InvalidOsVersion => {
            ApiError::bad_request(error.to_string()).with_error_key("invalid_client_info")
        },
        DevicePushError::Users(users) => ApiError::from(users),
    }
}
