//! [`NotificationsHttpExtension`]: declares the notification route groups and
//! registers the bridge inbox into the gateway surface.
//!
//! It declares `/api/v1/notifications` behind the user gate and
//! `/api/v1/admin/notifications` behind the admin gate, both in the
//! content-and-misc role group under the `notifications` rate-limit scope
//! and budget, and registers `/v1/bridge/notifications*` into the gateway
//! surface under the `bridge_notifications` scope and the same budget.
//!
//! Every group reads the notification service from the
//! `NotificationsServiceSlot` the `notifications` extension provides and the
//! host binds. When that extension is disabled in the profile the slot is
//! absent: the user and admin groups are not declared and the gateway skips
//! the inbox, so their paths answer 404.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_api::kernel::register_gateway_route;
use systemprompt_extension::prelude::*;
use systemprompt_extension::RouteGroup;
use systemprompt_models::modules::ApiPaths;

use crate::http::{NotificationsState, admin_router, bridge, router};

pub const NOTIFICATIONS_EXTENSION: &str = "notifications";

const RATE_LIMIT: RateLimit = RateLimit::new("notifications", RateBudget::Notifications);

#[derive(Debug, Clone, Copy, Default)]
pub struct NotificationsHttpExtension;

impl Extension for NotificationsHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "notifications-http",
            name: "Notifications HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn routers(&self, ctx: &dyn ExtensionContext) -> Vec<ExtensionRouter> {
        let state = match NotificationsState::from_extension_state(&ctx.extension_state()) {
            Ok(state) => state,
            Err(missing) => {
                tracing::info!(
                    extension = NOTIFICATIONS_EXTENSION,
                    missing = %missing,
                    "Notification routes not declared"
                );
                return Vec::new();
            },
        };
        vec![
            ExtensionRouter::new(
                router(state.clone()),
                ApiPaths::NOTIFICATIONS,
                AuthzPolicy::user(),
                ContextKind::User,
            )
            .in_group(RouteGroup::ContentAndMisc)
            .with_rate_limit(RATE_LIMIT),
            ExtensionRouter::new(
                admin_router(state),
                ApiPaths::ADMIN_NOTIFICATIONS,
                AuthzPolicy::admin(),
                ContextKind::User,
            )
            .in_group(RouteGroup::ContentAndMisc)
            .with_rate_limit(RATE_LIMIT),
        ]
    }
}

register_extension!(NotificationsHttpExtension);

register_gateway_route!(
    extension = NOTIFICATIONS_EXTENSION,
    scope = "bridge_notifications",
    budget = RateBudget::Notifications,
    factory = bridge::routes,
);
