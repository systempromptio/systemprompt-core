//! Mounts extension-provided routers onto the server router.
//!
//! Each extension returns an `ExtensionRouter` declaration; it is mounted
//! through the same declaration mount as every core route group, so its
//! authorization policy, context middleware, rate limit, body limit,
//! protection layers, frame options, role group and enable predicate are
//! applied exactly as declared.
//!
//! Every extension router receives the process's one governance engine as
//! an axum extension: an extension that enforces policy (the MCP governance
//! webhook, say) must charge the same rate-limiter budget as the gateway, and
//! the only way it can reach that engine is to be handed it here. The same
//! layer carries a `ServicesRefresh` handle, so a console that authorises a
//! caller by its own rule can run the services refresh in-process rather than
//! minting an admin token for the admin route. Capability services (the
//! `AiService`, the `ArtifactIngest`) are not injected: a router reads them
//! from `ExtensionContext::extension_state()` when it is built, and an absent
//! one is a typed `MissingDependency`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::RouteMountError;
use super::protocol::MountCtx;
use axum::{Extension, Router};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_traits::{StartupEvent, StartupEventSender};

use crate::routes::admin::services::{RefreshLock, ServicesRefresh};
use crate::services::middleware::admit_extension;

pub(super) fn mount_extension_routes(
    mut router: Router,
    mount: &MountCtx<'_>,
    refresh_lock: &RefreshLock,
    events: Option<&StartupEventSender>,
) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let registry = ctx.extension_registry();
    let api_extensions = registry.api_routers(ctx, &ctx.profile().extensions.disabled)?;
    ExtensionRegistry::validate_api_paths(&api_extensions)?;

    if api_extensions.is_empty() {
        return Ok(router);
    }

    let config_json = serde_json::json!({
        "paths": ctx.profile().paths,
    });

    for (ext, mut declared) in api_extensions {
        let ext_id = ext.metadata().id;
        let ext_name = ext.metadata().name;

        ext.validate_config(&config_json)
            .map_err(|source| RouteMountError::ConfigValidation {
                extension: ext_id.to_owned(),
                source: Box::new(source),
            })?;

        let base_path = declared.base_path;
        if !(mount.serves)(declared.group) || !declared.is_enabled(ctx) {
            tracing::info!(
                extension = ext_id,
                base_path,
                group = ?declared.group,
                "Extension router not mounted on this node"
            );
            continue;
        }
        admit_extension(&declared)?;
        let auth_required = declared.protection.requires_authentication();
        tracing::debug!(
            extension = ext_id,
            base_path,
            policy = ?declared.protection.policy,
            context = ?declared.protection.context,
            group = ?declared.group,
            "Mounting extension router"
        );

        declared.router = declared
            .router
            .layer(Extension(ctx.governance_watch().clone()))
            .layer(Extension(ServicesRefresh::new(ctx, refresh_lock.clone())));

        router = mount.mount(router, declared)?;

        if let Some(tx) = events
            && tx
                .unbounded_send(StartupEvent::ExtensionRouteMounted {
                    name: ext_name.to_owned(),
                    path: base_path.to_owned(),
                    auth_required,
                })
                .is_err()
        {
            tracing::debug!("Startup event receiver dropped");
        }
    }

    Ok(router)
}
