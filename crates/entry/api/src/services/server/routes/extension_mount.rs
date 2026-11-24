//! Mounts the admitted composition's route sets onto the server router.
//!
//! Every admitted route set is mounted through the enumerating mount
//! ([`mount_admitted`]), which first re-checks the whole handoff against the
//! kernel's reserved constants, so a route set reaches the router only with
//! a shape the conferral table lists for its owner. Each set is mounted on
//! the roles that serve its route group.
//!
//! Every composition route receives the process's one governance engine as
//! an axum extension: a route that enforces policy must charge the same
//! rate-limiter budget as the gateway. The same layer carries a
//! `ServicesRefresh` handle, so a console that authorises a caller by its own
//! rule can run the services refresh in-process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::RouteMountError;
use super::protocol::MountCtx;
use axum::{Extension, Router};
use systemprompt_extension::router::full_path;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_traits::{ConfigWatch, StartupEvent, StartupEventSender};

use crate::routes::admin::services::{RefreshLock, ServicesRefresh};
use crate::services::middleware::mount_admitted;

pub(super) fn mount_extension_routes(
    router: Router,
    mount: &MountCtx<'_>,
    refresh_lock: &RefreshLock,
    events: Option<&StartupEventSender>,
) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let governance = ctx
        .kernel()
        .state()
        .get::<ConfigWatch<GovernanceEngine>>()?
        .clone();
    let decorate = |routes: Router| {
        routes
            .layer(Extension(governance.clone()))
            .layer(Extension(ServicesRefresh::new(ctx, refresh_lock.clone())))
    };
    let admitted = mount.composed.admitted;
    let router = mount_admitted(
        router,
        mount.composed,
        mount.layers,
        mount.serves,
        &decorate,
    )?;
    if let Some(tx) = events {
        for set in admitted.routes() {
            if !(mount.serves)(set.set.group()) {
                continue;
            }
            let auth_required = set.set.defs().iter().any(|def| {
                !def.spec
                    .policy
                    .permits(systemprompt_models::auth::UserType::Anon)
            });
            if tx
                .unbounded_send(StartupEvent::ExtensionRouteMounted {
                    name: set.owner.as_str().to_owned(),
                    path: full_path(set.set.base(), "/"),
                    auth_required,
                })
                .is_err()
            {
                tracing::debug!("Startup event receiver dropped");
            }
        }
    }
    Ok(router)
}
