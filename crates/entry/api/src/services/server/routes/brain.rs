//! Mounts `/api/v1/brain` when the runtime composed a brain. An enabled
//! brain without a user provider fails the mount rather than serving routes
//! that cannot resolve their callers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::AppContext as AppContextTrait;

use super::protocol::MountCtx;

pub(super) fn mount_brain_routes(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, LoaderError> {
    let ctx = mount.ctx;
    let brain_routes = match ctx.brain() {
        Some(brain) => {
            let users = ctx
                .user_provider()
                .ok_or_else(|| LoaderError::InitializationFailed {
                    extension: ExtensionId::new("brain"),
                    message: "UserProvider is required for the brain routes".to_owned(),
                })?;
            let (service, recall, profile) = brain.routes();
            Some(crate::routes::brain::BrainRoutes::new(
                service, recall, profile, users,
            ))
        },
        None => None,
    };
    crate::routes::brain::mount_brain(
        router,
        mount.limits,
        mount.user_middleware,
        ctx.config().rate_limits.brain_per_second,
        brain_routes,
    )
}
