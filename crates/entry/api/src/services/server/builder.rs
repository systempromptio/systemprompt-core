use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use systemprompt_core_database::DatabaseQuery;
use systemprompt_models::api::SingleResponse;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::PathConfig;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEvent, StartupEventExt, StartupEventSender};

use super::routes::configure_routes;
use crate::models::ServerConfig;
use crate::services::middleware::{
    remove_trailing_slash, AnalyticsMiddleware, ContextMiddleware, CorsMiddleware,
    JwtContextExtractor, SessionMiddleware,
};

const HEALTH_CHECK_QUERY: DatabaseQuery = DatabaseQuery::new("SELECT 1");

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
            let _ = tx.send(StartupEvent::ServerBinding {
                address: addr.to_string(),
            });
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

    let analytics_middleware = AnalyticsMiddleware::new(ctx);
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

    Ok(router)
}

pub async fn handle_root_discovery(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    let base = &ctx.config().api_external_url;
    let data = json!({
        "name": format!("{} API", ctx.config().sitename),
        "version": "1.0.0",
        "description": "SystemPrompt OS API Gateway",
        "endpoints": {
            "health": format!("{}{}", base, ApiPaths::HEALTH),
            "oauth": {
                "href": format!("{}{}", base, ApiPaths::OAUTH_BASE),
                "description": "OAuth2/OIDC authentication and WebAuthn",
                "endpoints": {
                    "authorize": format!("{}{}", base, ApiPaths::OAUTH_AUTHORIZE),
                    "token": format!("{}{}", base, ApiPaths::OAUTH_TOKEN),
                    "userinfo": format!("{}{}/userinfo", base, ApiPaths::OAUTH_BASE),
                    "introspect": format!("{}{}/introspect", base, ApiPaths::OAUTH_BASE),
                    "revoke": format!("{}{}/revoke", base, ApiPaths::OAUTH_BASE),
                    "webauthn": format!("{}{}/webauthn", base, ApiPaths::OAUTH_BASE)
                }
            },
            "core": {
                "href": format!("{}{}", base, ApiPaths::CORE_BASE),
                "description": "Core conversation, task, and artifact management",
                "endpoints": {
                    "contexts": format!("{}{}", base, ApiPaths::CORE_CONTEXTS),
                    "tasks": format!("{}{}", base, ApiPaths::CORE_TASKS),
                    "artifacts": format!("{}{}", base, ApiPaths::CORE_ARTIFACTS)
                }
            },
            "agents": {
                "href": format!("{}{}", base, ApiPaths::AGENTS_REGISTRY),
                "description": "A2A protocol agent registry and proxy",
                "endpoints": {
                    "registry": format!("{}{}", base, ApiPaths::AGENTS_REGISTRY),
                    "proxy": format!("{}{}{{agent_id}}", base, ApiPaths::AGENTS_BASE)
                }
            },
            "mcp": {
                "href": format!("{}{}", base, ApiPaths::MCP_REGISTRY),
                "description": "MCP server registry and lifecycle management",
                "endpoints": {
                    "registry": format!("{}{}", base, ApiPaths::MCP_REGISTRY),
                    "proxy": format!("{}{}{{server_name}}", base, ApiPaths::MCP_BASE)
                }
            },
            "stream": {
                "href": format!("{}{}", base, ApiPaths::STREAM_BASE),
                "description": "Server-Sent Events (SSE) for real-time updates",
                "endpoints": {
                    "contexts": format!("{}{}", base, ApiPaths::STREAM_CONTEXTS)
                }
            }
        },
        "wellknown": {
            "oauth": format!("{}{}", base, ApiPaths::WELLKNOWN_OAUTH_SERVER),
            "agent": format!("{}{}", base, ApiPaths::WELLKNOWN_AGENT_CARD)
        }
    });

    Json(SingleResponse::new(data))
}

pub async fn handle_health(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    use systemprompt_core_database::{DatabaseProvider, ServiceRepository};

    let db_status = match ctx.db_pool().fetch_optional(&HEALTH_CHECK_QUERY, &[]).await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    let service_repo = ServiceRepository::new(ctx.db_pool().clone());

    let (agent_count, agent_status) = match service_repo.count_running_services("agent").await {
        Ok(count) if count > 0 => (count, "healthy"),
        Ok(_) => (0, "no_agents"),
        Err(_) => (0, "error"),
    };

    let (mcp_count, mcp_status) = match service_repo.count_running_services("mcp").await {
        Ok(count) if count > 0 => (count, "healthy"),
        Ok(_) => (0, "no_servers"),
        Err(_) => (0, "error"),
    };

    let web_dir = PathConfig::get()
        .map(|c| c.web_dist().clone())
        .unwrap_or_else(|_| std::path::PathBuf::from("/var/www/html/dist"));
    let sitemap_exists = web_dir.join("sitemap.xml").exists();
    let sitemap_status = if sitemap_exists { "present" } else { "missing" };
    let index_exists = web_dir.join("index.html").exists();
    let index_status = if index_exists { "present" } else { "missing" };

    let overall_status = if db_status == "healthy"
        && agent_status != "error"
        && mcp_status != "error"
        && sitemap_exists
        && index_exists
    {
        "healthy"
    } else {
        "degraded"
    };

    let data = json!({
        "status": overall_status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "database": db_status,
        "services": {
            "agents": {
                "status": agent_status,
                "running": agent_count
            },
            "mcp": {
                "status": mcp_status,
                "running": mcp_count
            }
        },
        "content": {
            "sitemap": sitemap_status,
            "index": index_status
        }
    });

    Json(SingleResponse::new(data))
}

pub fn discovery_router(ctx: &AppContext) -> Router {
    Router::new()
        .route(ApiPaths::DISCOVERY, get(handle_root_discovery))
        .route(ApiPaths::HEALTH, get(handle_health))
        .with_state(ctx.clone())
}
