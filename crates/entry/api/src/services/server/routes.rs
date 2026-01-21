use axum::Router;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

use crate::services::middleware::{
    ip_ban_middleware, ContextMiddleware, JwtContextExtractor, RouterExt,
};
use crate::services::static_content::{
    serve_homepage, serve_vite_app, smart_fallback_handler, StaticContentMatcher,
    StaticContentState,
};
use axum::routing::get;
use std::sync::Arc;
use systemprompt_users::BannedIpRepository;
use systemprompt_extension::LoaderError;
use systemprompt_models::AppPaths;
use systemprompt_traits::{StartupEvent, StartupEventSender};

pub fn configure_routes(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<Router, LoaderError> {
    let mut router = Router::new();

    let rate_config = &ctx.config().rate_limits;

    let jwt_extractor = JwtContextExtractor::new(
        systemprompt_models::SecretsBootstrap::jwt_secret().map_err(|e| {
            LoaderError::InitializationFailed {
                extension: "jwt".to_string(),
                message: e.to_string(),
            }
        })?,
        ctx.db_pool(),
    );

    let public_middleware = ContextMiddleware::public(jwt_extractor.clone());
    let user_middleware = ContextMiddleware::user_only(jwt_extractor.clone());
    let full_middleware = ContextMiddleware::full(jwt_extractor.clone());
    let mcp_middleware = ContextMiddleware::mcp(jwt_extractor.clone());

    router = router.nest(
        ApiPaths::OAUTH_BASE,
        systemprompt_oauth::api::public_router(ctx)
            .with_rate_limit(rate_config, rate_config.oauth_public_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::OAUTH_BASE,
        systemprompt_oauth::api::authenticated_router(ctx)
            .with_rate_limit(rate_config, rate_config.oauth_auth_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::CORE_CONTEXTS,
        systemprompt_agent::api::contexts_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.contexts_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::WEBHOOK,
        systemprompt_agent::api::webhook_router()
            .with_state(ctx.clone())
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::CORE_TASKS,
        systemprompt_agent::api::tasks_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.tasks_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::CORE_ARTIFACTS,
        systemprompt_agent::api::artifacts_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.artifacts_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::AGENTS_REGISTRY,
        systemprompt_agent::api::registry_router(ctx)
            .with_rate_limit(rate_config, rate_config.agent_registry_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::AGENTS_BASE,
        crate::routes::proxy::agents::router(ctx)
            .with_rate_limit(rate_config, rate_config.agents_per_second)
            .with_auth_middleware(full_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::MCP_REGISTRY,
        systemprompt_mcp::api::registry_router(ctx)
            .with_rate_limit(rate_config, rate_config.mcp_registry_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::MCP_BASE,
        crate::routes::proxy::mcp::router(ctx)
            .with_rate_limit(rate_config, rate_config.mcp_per_second)
            .with_auth_middleware(mcp_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::STREAM_BASE,
        crate::routes::stream::stream_router(ctx)
            .with_rate_limit(rate_config, rate_config.stream_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::CONTENT_BASE,
        systemprompt_content::api::router(ctx.db_pool())
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.merge(
        systemprompt_content::api::redirect_router(ctx.db_pool())
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.nest(
        "/api/v1/sync",
        crate::routes::sync::router().with_state(ctx.clone()),
    );

    router = router.nest(
        "/api/v1/analytics",
        crate::routes::analytics::router(ctx)
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        "/api/v1/engagement",
        crate::routes::engagement::router(ctx)
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        "/api/v1/admin",
        crate::routes::admin::router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, 10)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = mount_extension_routes(router, ctx, &user_middleware, events)?;

    let paths = match AppPaths::get() {
        Ok(p) => p,
        Err(e) => {
            if let Some(tx) = events {
                let _ = tx.send(StartupEvent::Warning {
                    message: format!("Failed to load paths: {e}"),
                    context: Some("Static content matching will be disabled".to_string()),
                });
            }
            return Ok(router);
        },
    };
    let path = paths.system().content_config().to_path_buf();
    let content_matcher = if let Some(path_str) = path.to_str() {
        match StaticContentMatcher::from_config(path_str) {
            Ok(matcher) => Arc::new(matcher),
            Err(e) => {
                if let Some(tx) = events {
                    let _ = tx.send(StartupEvent::Warning {
                        message: format!("Failed to load content config: {e}"),
                        context: Some("Static content matching will be disabled".to_string()),
                    });
                }
                Arc::new(StaticContentMatcher::empty())
            },
        }
    } else {
        if let Some(tx) = events {
            let _ = tx.send(StartupEvent::Warning {
                message: "CONTENT_CONFIG_PATH contains invalid UTF-8".to_string(),
                context: None,
            });
        }
        Arc::new(StaticContentMatcher::empty())
    };

    let static_state = StaticContentState {
        ctx: Arc::new(ctx.clone()),
        matcher: content_matcher,
        route_classifier: ctx.route_classifier().clone(),
    };

    // Merge discovery and wellknown routes BEFORE static router
    // This ensures API routes take precedence over the static fallback
    router = router.merge(discovery_router(ctx).with_auth_middleware(public_middleware.clone()));

    router = router.merge(wellknown_router(ctx).with_auth_middleware(public_middleware.clone()));

    let static_router = Router::new()
        .route("/", get(serve_homepage))
        .route("/agent", get(serve_vite_app))
        .route("/agent/{*path}", get(serve_vite_app))
        .fallback(smart_fallback_handler)
        .with_state(static_state)
        .with_auth_middleware(public_middleware.clone());

    router = router.merge(static_router);

    let banned_ip_repo = Arc::new(BannedIpRepository::new(ctx.db_pool()).map_err(|e| {
        LoaderError::InitializationFailed {
            extension: "ip_ban_middleware".to_string(),
            message: e.to_string(),
        }
    })?);

    Ok(router.layer(axum::middleware::from_fn(move |req, next| {
        let repo = banned_ip_repo.clone();
        async move { ip_ban_middleware(req, next, repo).await }
    })))
}

fn discovery_router(ctx: &AppContext) -> Router {
    super::builder::discovery_router(ctx)
}

fn wellknown_router(ctx: &AppContext) -> Router {
    systemprompt_oauth::api::wellknown::wellknown_routes(ctx)
        .merge(crate::routes::wellknown_router(ctx))
}

fn mount_extension_routes(
    mut router: Router,
    ctx: &AppContext,
    user_middleware: &ContextMiddleware<JwtContextExtractor>,
    events: Option<&StartupEventSender>,
) -> Result<Router, LoaderError> {
    let api_extensions = ctx.extension_registry().api_extensions(ctx);

    if api_extensions.is_empty() {
        return Ok(router);
    }

    let profile = systemprompt_models::ProfileBootstrap::get().map_err(|e| {
        LoaderError::InitializationFailed {
            extension: "profile".to_string(),
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
                extension: ext_id.to_string(),
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
                .with_auth_middleware(user_middleware.clone())
        } else {
            ext_router_config.router
        };

        if let Some(tx) = events {
            let _ = tx.send(StartupEvent::ExtensionRouteMounted {
                name: ext_name.to_string(),
                path: base_path.to_string(),
                auth_required: requires_auth,
            });
        }

        router = router.nest(base_path, ext_router);
    }

    Ok(router)
}
