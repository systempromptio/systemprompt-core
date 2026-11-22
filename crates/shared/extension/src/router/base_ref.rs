//! The gateway paths a gateway route may claim, and a privileged shape's base.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::modules::ApiPaths;

use super::shape::BasePath;

/// The gateway paths a gateway route may claim; an install holds none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GatewayPath {
    BridgeNotifications,
}

impl GatewayPath {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::BridgeNotifications => ApiPaths::BRIDGE_NOTIFICATIONS,
        }
    }
}

/// A privileged shape's base: an API or web base, or a gateway path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseRef {
    Path(BasePath),
    Gateway(GatewayPath),
}
