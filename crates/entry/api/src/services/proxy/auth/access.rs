//! Access enforcement for proxied MCP and agent requests.
//!
//! [`AccessValidator`] resolves whether a service requires OAuth, validates the
//! caller's bearer token and scopes, and either returns the authenticated user
//! or converts the failure into an RFC 9728 challenge. For MCP it permits a
//! session-only fallback when a prior authenticated initialize established the
//! identity in the proxy cache.

use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use std::str::FromStr;

use crate::services::proxy::backend::ProxyError;
use systemprompt_agent::services::AgentRegistryProviderService;
use systemprompt_database::ServiceConfig;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AgentRegistryProvider, McpRegistryProvider};

use super::challenge::{AuthValidator, ChallengeRequest, challenge_or_error};

pub(crate) struct AccessValidator;

impl AccessValidator {
    pub(crate) async fn validate(
        headers: &HeaderMap,
        service_name: &str,
        service: &ServiceConfig,
        ctx: &AppContext,
        req_context: Option<&RequestContext>,
    ) -> Result<Option<AuthenticatedUser>, ProxyError> {
        let (oauth_required, required_scopes) =
            lookup_oauth_requirement(service, service_name, ctx).await?;
        if !oauth_required {
            return Ok(None);
        }
        let resource_path = resource_path_for(service, service_name);
        let has_authorization = headers.get(AUTHORIZATION).is_some();
        let authenticated_user =
            match AuthValidator::validate_service_access(headers, service_name, req_context) {
                Ok(user) => user,
                Err(status_code) => {
                    if let Some(outcome) =
                        mcp_session_fallback(service, service_name, headers, status_code)
                    {
                        return outcome;
                    }
                    return Err(challenge_or_error(&ChallengeRequest {
                        service_name,
                        resource_path: &resource_path,
                        headers,
                        ctx,
                        status_code,
                        has_authorization,
                    }));
                },
            };
        ensure_required_scopes(service_name, &required_scopes, &authenticated_user)?;
        Ok(Some(authenticated_user))
    }
}

async fn lookup_oauth_requirement(
    service: &ServiceConfig,
    service_name: &str,
    ctx: &AppContext,
) -> Result<(bool, Vec<String>), ProxyError> {
    if service.module_name == "agent" {
        let registry =
            AgentRegistryProviderService::new().map_err(|e| ProxyError::ServiceNotRunning {
                service: service_name.to_owned(),
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
        let registry = ctx.mcp_registry();
        registry
            .validate()
            .map_err(|e| ProxyError::ServiceNotRunning {
                service: service_name.to_owned(),
                status: format!("Failed to load MCP registry: {e}"),
            })?;
        let info = McpRegistryProvider::get_server(registry, service_name)
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
        "agent" => ApiPaths::agent_endpoint(&systemprompt_identifiers::AgentId::new(service_name)),
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
