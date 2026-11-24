//! The `notifications` plane's error constructors.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::de::DeserializeOwned;
use systemprompt_extension::prelude::{ConfigPlaneError, ConfigPlaneResult};

use super::NOTIFICATIONS_PLANE;

pub(super) fn unreadable(
    path: &str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> ConfigPlaneError {
    ConfigPlaneError::Unreadable {
        plane: NOTIFICATIONS_PLANE.to_owned(),
        path: path.to_owned(),
        source: Box::new(error),
    }
}

pub(super) fn invalid(path: &str, message: &dyn std::fmt::Display) -> ConfigPlaneError {
    ConfigPlaneError::Invalid {
        plane: NOTIFICATIONS_PLANE.to_owned(),
        path: path.to_owned(),
        message: message.to_string(),
    }
}

pub(super) fn backend(error: impl std::error::Error + Send + Sync + 'static) -> ConfigPlaneError {
    ConfigPlaneError::Backend {
        plane: NOTIFICATIONS_PLANE.to_owned(),
        source: Box::new(error),
    }
}

pub(super) fn decode<T: DeserializeOwned>(key: &str, value: &str) -> ConfigPlaneResult<T> {
    serde_json::from_str(value).map_err(|e| invalid(key, &e))
}
