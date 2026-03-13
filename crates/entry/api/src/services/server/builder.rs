use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::Router;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEvent, StartupEventExt, StartupEventSender};

use super::routes::configure_routes;
use crate::models::ServerConfig;
use crate::services::middleware::{
    AnalyticsMiddleware, ContextMiddleware, CorsMiddleware, JwtContextExtractor, SessionMiddleware,
    inject_security_headers, inject_trace_header, remove_trailing_slash,
};

pub use super::discovery::*;
pub use super::health::handle_health;

#[derive(Debug)]
pub struct ApiServer {
    router: Router,
    _config: ServerConfig,
    events: Option<StartupEventSender>,
}

impl ApiServer {
    pub fn new(router: Router, events: Option<StartupEventSender>) -> Self {
        Self::with_config(router, ServerConfig::default(), events)
    }

    pub const fn with_config(
        router: Router,
        config: ServerConfig,
        events: Option<StartupEventSender>,
    ) -> Self {
        Self {
            router,
            _config: config,
            events,
        }
    }

    pub async fn serve(self, addr: &str) -> Result<()> {
        if let Some(ref tx) = self.events {
            if tx
                .unbounded_send(StartupEvent::ServerBinding {
                    address: addr.to_string(),
                })
                .is_err()
            {
                tracing::debug!("Startup event receiver dropped");
            }
        }

        let listener = self.create_listener(addr).await?;

        if let Some(ref tx) = self.events {
            tx.server_listening(addr, std::process::id());
        }

        axum::serve(
            listener,
            self.router
                .into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await?;
        Ok(())
    }

    async fn create_listener(&self, addr: &str) -> Result<tokio::net::TcpListener> {
        tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to bind to {addr}: {e}"))
    }
}

pub fn setup_api_server(ctx: &AppContext, events: Option<StartupEventSender>) -> Result<ApiServer> {
    let rate_config = &ctx.config().rate_limits;

    if rate_config.disabled {
        if let Some(ref tx) = events {
            tx.warning("Rate limiting disabled - development mode only");
        }
    }

    let router = configure_routes(ctx, events.as_ref())?;
    let router = apply_global_middleware(router, ctx)?;

    Ok(ApiServer::new(router, events))
}

fn apply_global_middleware(router: Router, ctx: &AppContext) -> Result<Router> {
    let mut router = router;

    router = router.layer(DefaultBodyLimit::max(100 * 1024 * 1024));

    let analytics_middleware = AnalyticsMiddleware::new(ctx)?;
    router = router.layer(axum::middleware::from_fn({
        let middleware = analytics_middleware;
        move |req, next| {
            let middleware = middleware.clone();
            async move { middleware.track_request(req, next).await }
        }
    }));

    let jwt_extractor = JwtContextExtractor::new(
        systemprompt_models::SecretsBootstrap::jwt_secret()?,
        ctx.db_pool(),
    );
    let global_context_middleware = ContextMiddleware::public(jwt_extractor);
    router = router.layer(axum::middleware::from_fn({
        let middleware = global_context_middleware;
        move |req, next| {
            let middleware = middleware.clone();
            async move { middleware.handle(req, next).await }
        }
    }));

    let session_middleware = SessionMiddleware::new(ctx)?;
    router = router.layer(axum::middleware::from_fn({
        let middleware = session_middleware;
        move |req, next| {
            let middleware = middleware.clone();
            async move { middleware.handle(req, next).await }
        }
    }));

    let cors = CorsMiddleware::build_layer(ctx.config())?;
    router = router.layer(cors);

    router = router.layer(axum::middleware::from_fn(remove_trailing_slash));

    router = router.layer(axum::middleware::from_fn(inject_trace_header));

    if ctx.config().content_negotiation.enabled {
        router = router.layer(axum::middleware::from_fn(
            crate::services::middleware::content_negotiation_middleware,
        ));
    }

    if ctx.config().security_headers.enabled {
        let security_config = ctx.config().security_headers.clone();
        router = router.layer(axum::middleware::from_fn(move |req, next| {
            let config = security_config.clone();
            inject_security_headers(config, req, next)
        }));
    }

    Ok(router)
}
