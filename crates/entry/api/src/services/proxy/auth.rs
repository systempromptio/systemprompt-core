use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::json;
use std::str::FromStr;
use systemprompt_agent::services::AgentRegistryProviderService;
use systemprompt_database::ServiceConfig;
use systemprompt_mcp::McpServerRegistry;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::services::AuthService;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AgentRegistryProvider, McpRegistryProvider};

use super::backend::ProxyError;

#[derive(Debug, Clone, Copy)]
pub struct AuthValidator;

impl AuthValidator {
    pub fn validate_service_access(
        headers: &HeaderMap,
        service_name: &str,
        _ctx: &AppContext,
        req_context: Option<&RequestContext>,
    ) -> Result<AuthenticatedUser, StatusCode> {
        let result = AuthService::authorize_service_access(headers, service_name);

        if let Err(status) = &result {
            let trace_id =
                req_context.map_or_else(|| "unknown".to_string(), |rc| rc.trace_id().to_string());
            tracing::warn!(service = %service_name, status = %status, trace_id = %trace_id, "auth failed");
        }

        result
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OAuthChallengeBuilder;

impl OAuthChallengeBuilder {
    pub fn build_challenge_response(
        service_name: &str,
        resource_path: &str,
        ctx: &AppContext,
        status_code: StatusCode,
    ) -> Result<Response<Body>, StatusCode> {
        tracing::warn!(service = %service_name, status = %status_code, "Building OAuth challenge");

        let oauth_base_url = &ctx.config().api_external_url;
        let resource_metadata_url =
            format!("{oauth_base_url}/.well-known/oauth-protected-resource{resource_path}");

        let (auth_header_value, error_body) = if status_code == StatusCode::UNAUTHORIZED {
            let header = format!(
                "Bearer realm=\"{service_name}\", resource_metadata=\"{resource_metadata_url}\", \
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
    ) -> Result<Option<AuthenticatedUser>, ProxyError> {
        let (oauth_required, required_scopes) =
            lookup_oauth_requirement(service, service_name).await?;
        if !oauth_required {
            return Ok(None);
        }
        let resource_path = resource_path_for(service, service_name);
        let authenticated_user =
            match AuthValidator::validate_service_access(headers, service_name, ctx, req_context) {
                Ok(user) => user,
                Err(status_code) => {
                    if let Some(outcome) =
                        mcp_session_fallback(service, service_name, headers, status_code)
                    {
                        return outcome;
                    }
                    return Err(challenge_or_error(
                        service_name,
                        &resource_path,
                        ctx,
                        status_code,
                    ));
                },
            };
        ensure_required_scopes(service_name, &required_scopes, &authenticated_user)?;
        Ok(Some(authenticated_user))
    }
}

async fn lookup_oauth_requirement(
    service: &ServiceConfig,
    service_name: &str,
) -> Result<(bool, Vec<String>), ProxyError> {
    if service.module_name == "agent" {
        let registry =
            AgentRegistryProviderService::new().map_err(|e| ProxyError::ServiceNotRunning {
                service: service_name.to_string(),
                status: format!("Failed to load agent registry: {e}"),
            })?;
        let info =
            registry
                .get_agent(service_name)
                .await
                .map_err(|e| ProxyError::ServiceNotFound {
                    service: format!("Agent '{}' not found in registry: {}", service_name, e),
                })?;
        Ok((info.oauth.required, info.oauth.scopes))
    } else if service.module_name == "mcp" {
        McpServerRegistry::validate().map_err(|e| ProxyError::ServiceNotRunning {
            service: service_name.to_string(),
            status: format!("Failed to load MCP registry: {e}"),
        })?;
        let registry = systemprompt_mcp::services::registry::RegistryManager;
        let info = McpRegistryProvider::get_server(&registry, service_name)
            .await
            .map_err(|e| ProxyError::ServiceNotFound {
                service: format!("MCP server '{}' not found in registry: {}", service_name, e),
            })?;
        Ok((info.oauth.required, info.oauth.scopes))
    } else {
        Ok((true, vec![]))
    }
}

fn resource_path_for(service: &ServiceConfig, service_name: &str) -> String {
    match service.module_name.as_str() {
        "mcp" => ApiPaths::mcp_server_endpoint(service_name),
        "agent" => ApiPaths::agent_endpoint(service_name),
        _ => String::new(),
    }
}

fn mcp_session_fallback(
    service: &ServiceConfig,
    service_name: &str,
    headers: &HeaderMap,
    status_code: StatusCode,
) -> Option<Result<Option<AuthenticatedUser>, ProxyError>> {
    if service.module_name != "mcp" || status_code != StatusCode::UNAUTHORIZED {
        return None;
    }
    let has_session = headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| !v.is_empty());
    if !has_session {
        return None;
    }
    let has_bearer_token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("Bearer "));
    if has_bearer_token {
        tracing::info!(
            service = %service_name,
            session_id = ?headers.get("mcp-session-id"),
            "MCP request has expired/invalid Bearer token — returning 401 for client token refresh"
        );
        return None;
    }
    tracing::info!(
        service = %service_name,
        session_id = ?headers.get("mcp-session-id"),
        "Allowing MCP request with session ID (session-based auth, identity from proxy cache)"
    );
    Some(Ok(None))
}

fn challenge_or_error(
    service_name: &str,
    resource_path: &str,
    ctx: &AppContext,
    status_code: StatusCode,
) -> ProxyError {
    match OAuthChallengeBuilder::build_challenge_response(
        service_name,
        resource_path,
        ctx,
        status_code,
    ) {
        Ok(challenge_response) => ProxyError::AuthChallenge(Box::new(challenge_response)),
        Err(status) if status == StatusCode::UNAUTHORIZED => ProxyError::AuthenticationRequired {
            service: service_name.to_string(),
        },
        Err(_) => ProxyError::Forbidden {
            service: service_name.to_string(),
        },
    }
}

fn ensure_required_scopes(
    service_name: &str,
    required_scopes: &[String],
    user: &AuthenticatedUser,
) -> Result<(), ProxyError> {
    if required_scopes.is_empty() {
        return Ok(());
    }
    let has_required_scope = required_scopes.iter().any(|required_scope_str| {
        Permission::from_str(required_scope_str).map_or_else(
            |_| {
                user.permissions
                    .iter()
                    .any(|p| p.as_str() == required_scope_str)
            },
            |required_permission| {
                user.permissions
                    .iter()
                    .any(|p| *p == required_permission || p.implies(&required_permission))
            },
        )
    });
    if !has_required_scope {
        return Err(ProxyError::Forbidden {
            service: format!(
                "Insufficient permissions for {}. Required: {:?}, User has: {:?}",
                service_name, required_scopes, user.permissions
            ),
        });
    }
    Ok(())
}
