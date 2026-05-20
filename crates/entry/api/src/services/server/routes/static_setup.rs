use axum::Router;
use axum::routing::get;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEvent, StartupEventSender};

use crate::services::middleware::authz::AuthzPolicy;
use crate::services::middleware::{
    ContextMiddleware, JwtContextExtractor, RouterExt, site_auth_gate,
};
use crate::services::static_content::{
    StaticContentMatcher, StaticContentState, serve_homepage, smart_fallback_handler,
};

pub(super) fn build_static_router(
    ctx: &AppContext,
    public_middleware: ContextMiddleware<JwtContextExtractor>,
    events: Option<&StartupEventSender>,
) -> Router {
    let path = ctx.app_paths().system().content_config().to_path_buf();
    #[allow(clippy::option_if_let_else)]
    let content_matcher = if let Some(path_str) = path.to_str() {
        match StaticContentMatcher::from_config(path_str) {
            Ok(matcher) => Arc::new(matcher),
            Err(e) => {
                if let Some(tx) = events {
                    if tx
                        .unbounded_send(StartupEvent::Warning {
                            message: format!("Failed to load content config: {e}"),
                            context: Some("Static content matching will be disabled".to_string()),
                        })
                        .is_err()
                    {
                        tracing::debug!("Startup event receiver dropped");
                    }
                }
                Arc::new(StaticContentMatcher::empty())
            },
        }
    } else {
        if let Some(tx) = events {
            if tx
                .unbounded_send(StartupEvent::Warning {
                    message: "CONTENT_CONFIG_PATH contains invalid UTF-8".to_string(),
                    context: None,
                })
                .is_err()
            {
                tracing::debug!("Startup event receiver dropped");
            }
        }
        Arc::new(StaticContentMatcher::empty())
    };

    let static_state = StaticContentState {
        ctx: Arc::new(ctx.clone()),
        matcher: content_matcher,
        route_classifier: Arc::clone(ctx.route_classifier()),
    };

    let static_router = Router::new()
        .route("/", get(serve_homepage))
        .fallback(smart_fallback_handler)
        .with_state(static_state)
        .with_auth(public_middleware, AuthzPolicy::public());

    let site_auth_config = ctx
        .extension_registry()
        .extensions()
        .iter()
        .find_map(|ext| ext.site_auth());

    if let Some(auth_config) = site_auth_config {
        let secret = systemprompt_config::SecretsBootstrap::jwt_secret()
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "JWT secret not available for site auth gate");
                ""
            })
            .to_string();
        static_router.layer(axum::middleware::from_fn(move |req, next| {
            let config = auth_config;
            let secret = secret.clone();
            async move { site_auth_gate(req, next, config, secret).await }
        }))
    } else {
        static_router
    }
}
