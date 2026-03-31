use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use systemprompt_models::api::SingleResponse;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

use super::health::handle_health;
use super::health_detail::handle_health_detail;

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

pub fn authenticated_discovery_router(ctx: &AppContext) -> Router {
    Router::new()
        .route("/api/v1/health/detail", get(handle_health_detail))
        .with_state(ctx.clone())
}
