//! [`ApiPaths`] constants as paths relative to the base a route set mounts at.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ApiPaths;

impl ApiPaths {
    #[must_use]
    pub fn notifications_relative(path: &'static str) -> &'static str {
        path.strip_prefix(Self::NOTIFICATIONS).unwrap_or(path)
    }

    #[must_use]
    pub fn admin_notifications_relative(path: &'static str) -> &'static str {
        path.strip_prefix(Self::ADMIN_NOTIFICATIONS).unwrap_or(path)
    }
}
