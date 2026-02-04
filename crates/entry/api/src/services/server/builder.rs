use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use systemprompt_database::DatabaseQuery;
use systemprompt_models::api::SingleResponse;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::AppPaths;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEvent, StartupEventExt, StartupEventSender};

use super::routes::configure_routes;
use crate::models::ServerConfig;
use crate::services::middleware::{
    inject_trace_header, remove_trailing_slash, AnalyticsMiddleware, ContextMiddleware,
    CorsMiddleware, JwtContextExtractor, SessionMiddleware,
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

    router = router.layer(axum::middleware::from_fn(inject_trace_header));

    Ok(router)
}

pub async fn handle_root_discovery(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    let base = &ctx.config().api_external_url;
    let data = json!({
        "name": format!("{} API", ctx.config().sitename),
        "version": "1.0.0",
        "description": "systemprompt.io OS API Gateway",
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

#[cfg(target_os = "linux")]
fn parse_proc_status_kb(content: &str, key: &str) -> Option<u64> {
    content
        .lines()
        .find(|line| line.starts_with(key))
        .and_then(|line| {
            line.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
        })
}

#[cfg(target_os = "linux")]
fn get_process_memory() -> Option<serde_json::Value> {
    let content = std::fs::read_to_string("/proc/self/status").ok()?;

    let rss_kb = parse_proc_status_kb(&content, "VmRSS:");
    let virt_kb = parse_proc_status_kb(&content, "VmSize:");
    let peak_kb = parse_proc_status_kb(&content, "VmPeak:");

    Some(json!({
        "rss_mb": rss_kb.map(|kb| kb / 1024),
        "virtual_mb": virt_kb.map(|kb| kb / 1024),
        "peak_mb": peak_kb.map(|kb| kb / 1024)
    }))
}

#[cfg(not(target_os = "linux"))]
fn get_process_memory() -> Option<serde_json::Value> {
    None
}

pub async fn handle_health(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use systemprompt_database::{DatabaseProvider, ServiceRepository};

    let start = std::time::Instant::now();

    let (db_status, db_latency_ms) = {
        let db_start = std::time::Instant::now();
        let status = match ctx.db_pool().fetch_optional(&HEALTH_CHECK_QUERY, &[]).await {
            Ok(_) => "healthy",
            Err(_) => "unhealthy",
        };
        (status, db_start.elapsed().as_millis())
    };

    let service_repo = ServiceRepository::new(ctx.db_pool().clone());

    let (agent_count, agent_status) = match service_repo.count_running_services("agent").await {
        Ok(count) if count > 0 => (count, "healthy"),
        Ok(_) => (0, "none"),
        Err(_) => (0, "error"),
    };

    let (mcp_count, mcp_status) = match service_repo.count_running_services("mcp").await {
        Ok(count) if count > 0 => (count, "healthy"),
        Ok(_) => (0, "none"),
        Err(_) => (0, "error"),
    };

    let web_dir = AppPaths::get()
        .map(|p| p.web().dist().to_path_buf())
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "Failed to get web dist path, using default");
            std::path::PathBuf::from("/var/www/html/dist")
        });
    let sitemap_exists = web_dir.join("sitemap.xml").exists();
    let index_exists = web_dir.join("index.html").exists();

    let db_healthy = db_status == "healthy";
    let services_ok = agent_status != "error" && mcp_status != "error";
    let content_ok = sitemap_exists && index_exists;

    let (overall_status, http_status) = match (db_healthy, services_ok && content_ok) {
        (false, _) => ("unhealthy", StatusCode::SERVICE_UNAVAILABLE),
        (true, false) => ("degraded", StatusCode::OK),
        (true, true) => ("healthy", StatusCode::OK),
    };

    let check_duration_ms = start.elapsed().as_millis();
    let memory = get_process_memory();

    let data = json!({
        "status": overall_status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "checks": {
            "database": {
                "status": db_status,
                "latency_ms": db_latency_ms
            },
            "agents": {
                "status": agent_status,
                "count": agent_count
            },
            "mcp": {
                "status": mcp_status,
                "count": mcp_count
            },
            "static_content": {
                "status": if content_ok { "healthy" } else { "degraded" },
                "index_html": index_exists,
                "sitemap_xml": sitemap_exists
            }
        },
        "memory": memory,
        "response_time_ms": check_duration_ms
    });

    (http_status, Json(data))
}

pub async fn handle_core_discovery(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    let base = &ctx.config().api_external_url;
    let data = json!({
        "name": "Core Services",
        "description": "Core conversation, task, and artifact management APIs",
        "endpoints": {
            "contexts": {
                "href": format!("{}{}", base, ApiPaths::CORE_CONTEXTS),
                "description": "Conversation context management",
                "methods": ["GET", "POST", "DELETE"]
            },
            "tasks": {
                "href": format!("{}{}", base, ApiPaths::CORE_TASKS),
                "description": "Task management for agent operations",
                "methods": ["GET", "POST", "PUT", "DELETE"]
            },
            "artifacts": {
                "href": format!("{}{}", base, ApiPaths::CORE_ARTIFACTS),
                "description": "Artifact storage and retrieval",
                "methods": ["GET", "POST", "DELETE"]
            },
            "oauth": {
                "href": format!("{}{}", base, ApiPaths::OAUTH_BASE),
                "description": "OAuth2/OIDC authentication endpoints"
            }
        }
    });
    Json(SingleResponse::new(data))
}

pub async fn handle_agents_discovery(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    let base = &ctx.config().api_external_url;
    let data = json!({
        "name": "Agent Services",
        "description": "A2A protocol agent registry and proxy",
        "endpoints": {
            "registry": {
                "href": format!("{}{}", base, ApiPaths::AGENTS_REGISTRY),
                "description": "List and discover available agents",
                "methods": ["GET"]
            },
            "proxy": {
                "href": format!("{}{}/<agent_id>/", base, ApiPaths::AGENTS_BASE),
                "description": "Proxy requests to specific agents",
                "methods": ["GET", "POST"]
            }
        }
    });
    Json(SingleResponse::new(data))
}

pub async fn handle_mcp_discovery(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    let base = &ctx.config().api_external_url;
    let data = json!({
        "name": "MCP Services",
        "description": "Model Context Protocol server registry and proxy",
        "endpoints": {
            "registry": {
                "href": format!("{}{}", base, ApiPaths::MCP_REGISTRY),
                "description": "List and discover available MCP servers",
                "methods": ["GET"]
            },
            "proxy": {
                "href": format!("{}{}/<server_name>/mcp", base, ApiPaths::MCP_BASE),
                "description": "Proxy requests to specific MCP servers",
                "methods": ["GET", "POST"]
            }
        }
    });
    Json(SingleResponse::new(data))
}

pub fn discovery_router(ctx: &AppContext) -> Router {
    Router::new()
        .route(ApiPaths::DISCOVERY, get(handle_root_discovery))
        .route(ApiPaths::HEALTH, get(handle_health))
        .route("/health", get(handle_health))
        .route(ApiPaths::CORE_BASE, get(handle_core_discovery))
        .route(ApiPaths::AGENTS_BASE, get(handle_agents_discovery))
        .route(ApiPaths::MCP_BASE, get(handle_mcp_discovery))
        .with_state(ctx.clone())
}
