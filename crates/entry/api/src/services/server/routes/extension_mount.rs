use axum::Router;
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
    let api_extensions = ctx.extension_registry().api_extensions(ctx);

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

    for ext in api_extensions {
        let ext_id = ext.metadata().id;
        let ext_name = ext.metadata().name;

        ext.validate_config(&config_json)
            .map_err(|e| LoaderError::ConfigValidationFailed {
                extension: ext_id.to_owned(),
                message: e.to_string(),
            })?;

        let Some(ext_router_config) = ext.router(ctx) else {
            continue;
        };

        let base_path = ext_router_config.base_path;
        let requires_auth = ext_router_config.requires_auth;

        let ext_router = if requires_auth {
            ext_router_config
                .router
                .with_auth(user_middleware.clone(), AuthzPolicy::user())
        } else {
            ext_router_config.router
        };

        if let Some(tx) = events {
            if tx
                .unbounded_send(StartupEvent::ExtensionRouteMounted {
                    name: ext_name.to_owned(),
                    path: base_path.to_owned(),
                    auth_required: requires_auth,
                })
                .is_err()
            {
                tracing::debug!("Startup event receiver dropped");
            }
        }

        if base_path == "/" {
            router = router.merge(ext_router);
        } else {
            router = router.nest(base_path, ext_router);
        }
    }

    Ok(router)
}
