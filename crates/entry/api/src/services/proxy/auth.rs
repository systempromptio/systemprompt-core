use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::json;
use std::str::FromStr;
use systemprompt_agent::services::AgentRegistryProviderService;
use systemprompt_database::ServiceConfig;
use systemprompt_mcp::McpServerRegistry;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::RequestContext;
use systemprompt_oauth::services::AuthService;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AgentRegistryProvider, McpRegistryProvider};

use super::backend::ProxyError;

#[derive(Debug, Clone, Copy)]
pub struct AuthValidator;

impl AuthValidator {
    pub async fn validate_service_access(
        headers: &HeaderMap,
        service_name: &str,
        _ctx: &AppContext,
        req_context: Option<&RequestContext>,
    ) -> Result<AuthenticatedUser, StatusCode> {
        let debug_auth = std::env::var("DEBUG_AUTH_LOGGING")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        if debug_auth {
            let trace_id =
                req_context.map_or_else(|| "unknown".to_string(), |rc| rc.trace_id().to_string());
            tracing::info!(service = %service_name, trace_id = %trace_id, "auth validation starting");
        }

        let result = AuthService::authorize_service_access(headers, service_name);

        match &result {
            Ok(user) => {
                if debug_auth {
                    let trace_id = req_context
                        .map_or_else(|| "unknown".to_string(), |rc| rc.trace_id().to_string());
                    tracing::info!(service = %service_name, username = %user.username, trace_id = %trace_id, "auth success");
                }
            },
            Err(status) => {
                let trace_id = req_context
                    .map_or_else(|| "unknown".to_string(), |rc| rc.trace_id().to_string());
                tracing::warn!(service = %service_name, status = %status, trace_id = %trace_id, "auth failed");
            },
        }

        result
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OAuthChallengeBuilder;

impl OAuthChallengeBuilder {
    pub async fn build_challenge_response(
        service_name: &str,
        ctx: &AppContext,
        status_code: StatusCode,
    ) -> Result<Response<Body>, StatusCode> {
        tracing::warn!(service = %service_name, status = %status_code, "Building OAuth challenge");

        let oauth_base_url = &ctx.config().api_server_url;

        let (auth_header_value, error_body) = if status_code == StatusCode::UNAUTHORIZED {
            let header = format!(
                "Bearer realm=\"{service_name}\", \
                 as_uri=\"{oauth_base_url}/.well-known/oauth-authorization-server\", \
                 error=\"invalid_token\""
            );
            let body = json!({
                "error": "invalid_token",
                "error_description": "The access token is missing or invalid",
                "server": service_name,
                "authorization_url": format!("{oauth_base_url}/.well-known/oauth-authorization-server")
            });
            (header, body)
        } else {
            let header = format!(
                "Bearer realm=\"{service_name}\", error=\"insufficient_scope\", \
                 error_description=\"The access token lacks required scope\""
            );
            let body = json!({
                "error": "insufficient_scope",
                "error_description": "The access token does not have the required scope for this resource",
                "server": service_name
            });
            (header, body)
        };

        Response::builder()
            .status(status_code)
            .header("Content-Type", "application/json")
            .header("WWW-Authenticate", auth_header_value)
            .body(Body::from(error_body.to_string()))
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to build OAuth challenge response");
                StatusCode::INTERNAL_SERVER_ERROR
            })
    }
}

pub struct AccessValidator;

impl AccessValidator {
    pub async fn validate(
        headers: &HeaderMap,
        service_name: &str,
        service: &ServiceConfig,
        ctx: &AppContext,
        req_context: Option<&RequestContext>,
    ) -> Result<(), ProxyError> {
        let (oauth_required, required_scopes) = if service.module_name == "agent" {
            match AgentRegistryProviderService::new().await {
                Ok(registry) => match registry.get_agent(service_name).await {
                    Ok(agent_info) => (agent_info.oauth.required, agent_info.oauth.scopes),
                    Err(e) => {
                        return Err(ProxyError::ServiceNotFound {
                            service: format!(
                                "Agent '{}' not found in registry: {}",
                                service_name, e
                            ),
                        });
                    },
                },
                Err(e) => {
                    return Err(ProxyError::ServiceNotRunning {
                        service: service_name.to_string(),
                        status: format!("Failed to load agent registry: {e}"),
                    });
                },
            }
        } else if service.module_name == "mcp" {
            match McpServerRegistry::validate() {
                Ok(()) => {
                    let registry = systemprompt_mcp::services::registry::RegistryManager;
                    match McpRegistryProvider::get_server(&registry, service_name).await {
                        Ok(server_info) => (server_info.oauth.required, server_info.oauth.scopes),
                        Err(e) => {
                            return Err(ProxyError::ServiceNotFound {
                                service: format!(
                                    "MCP server '{}' not found in registry: {}",
                                    service_name, e
                                ),
                            });
                        },
                    }
                },
                Err(e) => {
                    return Err(ProxyError::ServiceNotRunning {
                        service: service_name.to_string(),
                        status: format!("Failed to load MCP registry: {e}"),
                    });
                },
            }
        } else {
            (true, vec![])
        };

        if !oauth_required {
            return Ok(());
        }

        let authenticated_user =
            match AuthValidator::validate_service_access(headers, service_name, ctx, req_context)
                .await
            {
                Ok(user) => user,
                Err(status_code) => {
                    match OAuthChallengeBuilder::build_challenge_response(
                        service_name,
                        ctx,
                        status_code,
                    )
                    .await
                    {
                        Ok(challenge_response) => {
                            return Err(ProxyError::AuthChallenge(challenge_response));
                        },
                        Err(status) => {
                            return Err(if status == StatusCode::UNAUTHORIZED {
                                ProxyError::AuthenticationRequired {
                                    service: service_name.to_string(),
                                }
                            } else {
                                ProxyError::Forbidden {
                                    service: service_name.to_string(),
                                }
                            });
                        },
                    }
                },
            };

        if !required_scopes.is_empty() {
            let has_required_scope = required_scopes.iter().any(|required_scope_str| {
                if let Ok(required_permission) = Permission::from_str(required_scope_str) {
                    authenticated_user
                        .permissions
                        .iter()
                        .any(|user_permission| {
                            *user_permission == required_permission
                                || user_permission.implies(&required_permission)
                        })
                } else {
                    authenticated_user
                        .permissions
                        .iter()
                        .any(|user_permission| user_permission.as_str() == required_scope_str)
                }
            });

            if !has_required_scope {
                return Err(ProxyError::Forbidden {
                    service: format!(
                        "Insufficient permissions for {}. Required: {:?}, User has: {:?}",
                        service_name, required_scopes, authenticated_user.permissions
                    ),
                });
            }
        }

        Ok(())
    }
}
