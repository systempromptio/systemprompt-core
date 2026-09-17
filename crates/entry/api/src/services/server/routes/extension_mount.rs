//! Mounts extension-provided routers onto the server router.
//!
//! Every extension router receives the process's one governance engine as
//! an axum extension: an extension that enforces policy (the MCP governance
//! webhook, say) must charge the same rate-limiter budget as the gateway, and
//! the only way it can reach that engine is to be handed it here. The same
//! layer carries the process's `Option<Arc<AiService>>`, so an extension
//! handler that needs inference (a console-triggered evaluation) shares the
//! one service instead of assembling its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::{Extension, Router};
use systemprompt_extension::LoaderError;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEvent, StartupEventSender};

use crate::services::middleware::authz::AuthzPolicy;
use crate::services::middleware::{RouterExt, UserOnlyContextMiddleware};

pub(super) fn mount_extension_routes(
    mut router: Router,
    ctx: &AppContext,
    user_middleware: &UserOnlyContextMiddleware,
    events: Option<&StartupEventSender>,
) -> Result<Router, LoaderError> {
    let registry = ctx.extension_registry();
    registry.validate_api_paths(ctx)?;
    let api_extensions = registry.api_routers(ctx);

    if api_extensions.is_empty() {
        return Ok(router);
    }

    let profile = systemprompt_config::ProfileBootstrap::get().map_err(|e| {
        LoaderError::InitializationFailed {
            extension: "profile".to_owned(),
            message: e.to_string(),
        }
    })?;

    let config_json = serde_json::json!({
        "paths": profile.paths,
    });

    for (ext, ext_router_config) in api_extensions {
        let ext_id = ext.metadata().id;
        let ext_name = ext.metadata().name;

        ext.validate_config(&config_json)
            .map_err(|e| LoaderError::ConfigValidationFailed {
                extension: ext_id.to_owned(),
                message: e.to_string(),
            })?;

        let base_path = ext_router_config.base_path;
        let requires_auth = ext_router_config.requires_auth;

        let mut ext_router = if requires_auth {
            ext_router_config
                .router
                .with_auth(user_middleware.clone(), AuthzPolicy::user())
        } else {
            ext_router_config.router
        }
        .layer(Extension(ctx.governance_arc()))
        .layer(Extension(ctx.ai_service_arc()))
        .layer(Extension(ctx.artifact_ingest_arc()));

        if let Some(frame_options) = ext_router_config.frame_options {
            tracing::debug!(
                extension = ext_id,
                base_path,
                ?frame_options,
                "Applying frame-options override"
            );
            ext_router = ext_router.layer(axum::middleware::from_fn(move |request, next| {
                systemprompt_extension::stamp_frame_options(frame_options, request, next)
            }));
        }

        if let Some(tx) = events
            && tx
                .unbounded_send(StartupEvent::ExtensionRouteMounted {
                    name: ext_name.to_owned(),
                    path: base_path.to_owned(),
                    auth_required: requires_auth,
                })
                .is_err()
        {
            tracing::debug!("Startup event receiver dropped");
        }

        if base_path == systemprompt_extension::registry::WEB_ROOT_BASE_PATH {
            router = router.merge(ext_router);
        } else {
            router = router.nest(base_path, ext_router);
        }
    }

    Ok(router)
}
