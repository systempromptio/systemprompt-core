//! Access enforcement for proxied MCP and agent requests.
//!
//! [`AccessValidator`] resolves whether a service requires OAuth, validates the
//! caller's bearer token and scopes, and either returns the authenticated user
//! or converts the failure into an RFC 9728 challenge. For MCP it permits a
//! session-only fallback when a prior authenticated initialize established the
//! identity in the proxy cache.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use std::str::FromStr;

use crate::services::proxy::backend::ProxyError;
use systemprompt_agent::services::AgentRegistryProviderService;
use systemprompt_database::ServiceConfig;
use systemprompt_identifiers::ServiceName;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::services::ServiceModule;
use systemprompt_oauth::services::AuthService;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AgentRegistryProvider, McpRegistryProvider, RegistryError};

use super::challenge::{AuthValidator, ChallengeRequest, challenge_or_error};

#[derive(Debug)]
pub struct OAuthRequirement {
    pub module: ServiceModule,
    pub required: bool,
    pub scopes: Vec<String>,
    pub audience: String,
}

#[derive(Debug, Clone, Copy)]
pub struct AccessValidator;

impl AccessValidator {
    pub(crate) async fn validate(
        headers: &HeaderMap,
        service_name: &ServiceName,
        service: &ServiceConfig,
        ctx: &AppContext,
        req_context: Option<&RequestContext>,
    ) -> Result<Option<AuthenticatedUser>, ProxyError> {
        let requirement = lookup_oauth_requirement(service, service_name, ctx).await?;
        Self::validate_with_requirement(headers, service_name, &requirement, ctx, req_context)
    }

    pub fn validate_with_requirement(
        headers: &HeaderMap,
        service_name: &ServiceName,
        requirement: &OAuthRequirement,
        ctx: &AppContext,
        req_context: Option<&RequestContext>,
    ) -> Result<Option<AuthenticatedUser>, ProxyError> {
        if !requirement.required {
            return Ok(None);
        }
        let resource_path = resource_path_for(requirement.module, service_name);
        let has_authorization = headers.get(AUTHORIZATION).is_some();
        let challenge = |status_code: StatusCode| {
            challenge_or_error(&ChallengeRequest {
                service_name,
                resource_path: &resource_path,
                headers,
                ctx,
                status_code,
                has_authorization,
            })
        };
        let authenticated_user =
            match AuthValidator::validate_service_access(headers, service_name, req_context) {
                Ok(user) => user,
                Err(status_code) => {
                    if let Some(outcome) =
                        mcp_session_fallback(requirement.module, service_name, headers, status_code)
                    {
                        return outcome;
                    }
                    return Err(challenge(status_code));
                },
            };
        if let Err(status_code) =
            enforce_required_audience(headers, service_name, &requirement.audience)
        {
            return Err(challenge(status_code));
        }
        ensure_required_scopes(service_name, &requirement.scopes, &authenticated_user)?;
        Ok(Some(authenticated_user))
    }
}

async fn lookup_oauth_requirement(
    service: &ServiceConfig,
    service_name: &ServiceName,
    ctx: &AppContext,
) -> Result<OAuthRequirement, ProxyError> {
    match service.module_name {
        ServiceModule::Agent => {
            let registry = AgentRegistryProviderService::new()
                .map_err(|error| registry_lookup_error(service_name, error))?;
            let info = registry
                .get_agent(service_name.as_str())
                .await
                .map_err(|error| registry_lookup_error(service_name, error))?;
            Ok(OAuthRequirement {
                module: ServiceModule::Agent,
                required: info.oauth.required,
                scopes: info.oauth.scopes,
                audience: info.oauth.audience,
            })
        },
        ServiceModule::Mcp => mcp_oauth_requirement(ctx, service_name).await,
    }
}

pub(crate) async fn mcp_oauth_requirement(
    ctx: &AppContext,
    service_name: &ServiceName,
) -> Result<OAuthRequirement, ProxyError> {
    let registry = ctx.mcp_registry();
    registry
        .validate()
        .map_err(|source| ProxyError::RegistryUnavailable {
            service: service_name.to_string(),
            source,
        })?;
    let info = McpRegistryProvider::get_server(registry, service_name.as_str())
        .await
        .map_err(|error| registry_lookup_error(service_name, error))?;
    Ok(OAuthRequirement {
        module: ServiceModule::Mcp,
        required: info.oauth.required,
        scopes: info.oauth.scopes,
        audience: info.oauth.audience,
    })
}

fn registry_lookup_error(service_name: &ServiceName, error: RegistryError) -> ProxyError {
    match error {
        RegistryError::NotFound(_) => ProxyError::ServiceNotFound {
            service: service_name.to_string(),
        },
        source => ProxyError::RegistryLookupFailed {
            service: service_name.to_string(),
            source,
        },
    }
}

fn enforce_required_audience(
    headers: &HeaderMap,
    service_name: &ServiceName,
    audience: &str,
) -> Result<(), StatusCode> {
    if audience.is_empty() {
        return Ok(());
    }
    AuthService::authorize_required_audience(headers, audience)
        .map(|_user| ())
        .inspect_err(|status| {
            tracing::warn!(
                service = %service_name,
                audience = %audience,
                status = %status,
                "Token lacks the service's required audience"
            );
        })
}

fn resource_path_for(module: ServiceModule, service_name: &ServiceName) -> String {
    match module {
        ServiceModule::Mcp => ApiPaths::mcp_server_endpoint(service_name.as_str()),
        ServiceModule::Agent => ApiPaths::agent_endpoint(
            &systemprompt_identifiers::AgentName::new(service_name.as_str()),
        ),
    }
}

fn mcp_session_fallback(
    module: ServiceModule,
    service_name: &ServiceName,
    headers: &HeaderMap,
    status_code: StatusCode,
) -> Option<Result<Option<AuthenticatedUser>, ProxyError>> {
    if module != ServiceModule::Mcp || status_code != StatusCode::UNAUTHORIZED {
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
    service_name: &ServiceName,
    required_scopes: &[String],
    user: &AuthenticatedUser,
) -> Result<(), ProxyError> {
    if required_scopes.is_empty() {
        tracing::error!(service = %service_name, "OAuth required but no scopes are declared");
        return Err(ProxyError::Forbidden {
            service: format!("{service_name} requires OAuth but declares no scopes"),
        });
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
