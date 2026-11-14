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
//! host binds; this extension `requires` that slot. All three groups belong
//! to this extension's id, `notifications-http`, including the gateway
//! registration:
//!
//! | profile disables | user | admin | bridge inbox |
//! |---|---|---|---|
//! | `notifications-http` | 404 | 404 | 404 |
//! | `notifications` | boot refused (unmet `NotificationsServiceSlot`) | | |
//! | both | 404 | 404 | 404 |
//!
//! A slot that is present but unbound is a boot error. The admin group sits
//! under the reserved `/api/v1/admin` prefix by the `NotificationsAdmin` row of
//! the API's core allowance table, keyed on this extension's id, that exact
//! path and its role group.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_api::kernel::register_gateway_route;
use systemprompt_extension::RouteGroup;
use systemprompt_extension::prelude::*;
use systemprompt_models::modules::ApiPaths;
use systemprompt_notifications::NotificationsServiceSlot;

use crate::http::{NotificationsState, admin_router, bridge, router};

pub const NOTIFICATIONS_HTTP_EXTENSION: &str = "notifications-http";

const RATE_LIMIT: RateLimit = RateLimit::new("notifications", RateBudget::Notifications);

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

    fn requires(&self) -> Vec<StateRequirement> {
        vec![StateRequirement::of::<NotificationsServiceSlot>()]
    }

    fn routers(
        &self,
        ctx: &dyn ExtensionContext,
    ) -> Result<Vec<ExtensionRouter>, ExtensionStateError> {
        let state = NotificationsState::from_extension_state(&ctx.extension_state())?;
        Ok(vec![
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
        ])
    }
}

register_extension!(NotificationsHttpExtension);

register_gateway_route!(
    extension = NOTIFICATIONS_HTTP_EXTENSION,
    path = ApiPaths::BRIDGE_NOTIFICATIONS,
    scope = "bridge_notifications",
    budget = RateBudget::Notifications,
    factory = bridge::routes,
);
