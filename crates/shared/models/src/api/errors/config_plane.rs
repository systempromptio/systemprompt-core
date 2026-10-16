//! The `ConfigPlaneError` → [`ApiError`] mapping.
//!
//! A write refused because the plane is file-managed answers 409 with the
//! `config_plane_file_managed` key, an unknown plane 404, an invalid
//! declaration 400, and every backend failure 500 with the cause logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::ConfigPlaneError;

use super::ApiError;

const PLANE_FAILURE: &str = "Config plane operation failed";

impl From<ConfigPlaneError> for ApiError {
    fn from(err: ConfigPlaneError) -> Self {
        match err {
            ConfigPlaneError::FileManaged { .. } => {
                Self::conflict(err.to_string()).with_error_key("config_plane_file_managed")
            },
            ConfigPlaneError::UnknownPlane(_) => Self::not_found(err.to_string()),
            ConfigPlaneError::Invalid { .. } => Self::bad_request(err.to_string()),
            other => Self::internal(PLANE_FAILURE, other),
        }
    }
}
