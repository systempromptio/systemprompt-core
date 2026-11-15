//! [`NotificationsHttpExtension`] (`notifications-http`): the notification
//! route sets and the bridge inbox's gateway route.
//!
//! `/api/v1/notifications` sits behind the user gate and
//! `/api/v1/admin/notifications` behind the admin gate, both with a user
//! context in the content-and-misc role group under the `notifications`
//! rate-limit scope and budget. `/v1/bridge/notifications*` is one gateway
//! route: handler-verified by the gateway credential with a handler-resolved
//! context, in the gateway role group under the `bridge_notifications` scope,
//! the notifications budget and the bearer window. Each shape equals the
//! reference composition's conferral for `notifications-http` at its base.
//! The kernel's gateway verifier attaches the caller's
//! [`GatewayPrincipal`](systemprompt_extension::router::GatewayPrincipal) to
//! the bridge requests only, and the bridge handlers read the caller from it.
//!
//! Every set reads the notification service from the
//! `NotificationsServiceSlot` the `notifications` member provides and the
//! composition root binds; this member `requires` that handle:
//!
//! | profile disables | user | admin | bridge inbox |
//! |---|---|---|---|
//! | `notifications-http` | 404 | 404 | 404 |
//! | `notifications` | refused (unmet `NotificationsServiceSlot`) | | |
//!
//! A handle that is present but unbound refuses the mount.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const NOTIFICATIONS_HTTP_EXTENSION: &str = "notifications-http";

#[derive(Debug, Clone, Copy, Default)]
pub struct NotificationsHttpExtension;

impl Extension for NotificationsHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: NOTIFICATIONS_HTTP_EXTENSION,
            name: "Notifications HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        routes::contribute(c);
        #[cfg(not(feature = "http"))]
        let _ = c;
        Ok(())
    }
}

#[cfg(feature = "http")]
mod routes {
    use systemprompt_extension::prelude::*;
    use systemprompt_extension::router::{GatewayPath, GatewayRouteDecl};
    use systemprompt_notifications::NotificationsServiceSlot;

    use crate::http::{BRIDGE_LIMITS, admin_routes, bridge_routes, user_routes};

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<NotificationsServiceSlot>()
            .routes(user_routes())
            .routes(admin_routes())
            .gateway_route(GatewayRouteDecl {
                path: GatewayPath::BridgeNotifications,
                limits: BRIDGE_LIMITS,
                router: bridge_routes(),
            });
    }
}
