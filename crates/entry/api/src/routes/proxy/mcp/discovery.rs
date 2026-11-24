//! RFC 9728 protected-resource and authorization-server metadata per MCP
//! service, including the RFC 8693 token-type and EMA advertisements.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use systemprompt_identifiers::McpServerId;
use systemprompt_models::ApiError;
use systemprompt_models::mcp::McpExtensionId;
use systemprompt_models::oauth::ProtectedResourceMetadata;
use systemprompt_oauth_issuance::AuthorizationServerVocabulary;
use systemprompt_traits::McpRegistryProvider;

use super::McpState;
use crate::kernel::ApiHttpError;

#[derive(Debug, Serialize)]
struct McpAuthorizationServerMetadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    registration_endpoint: Option<String>,
    scopes_supported: Vec<String>,
    response_types_supported: Vec<String>,
    grant_types_supported: Vec<String>,
    code_challenge_methods_supported: Vec<String>,
    token_endpoint_auth_methods_supported: Vec<String>,
    authorization_response_iss_parameter_supported: bool,
    subject_token_types_supported: Vec<String>,
    issued_token_types_supported: Vec<String>,
    authorization_grant_profiles_supported: Vec<String>,
}

pub(super) async fn handle_mcp_protected_resource(
    State(state): State<McpState>,
    Path(raw_server): Path<String>,
) -> Result<Response, ApiHttpError> {
    let server = McpServerId::try_new(raw_server).map_err(ApiError::from)?;
    let base_url = state.ctx.config().api_external_url.clone();

    let registry = match crate::services::proxy::mcp_registry(&state.ctx) {
        Ok(registry) => Some(registry),
        Err(absent) => {
            tracing::debug!(error = %absent, "No MCP registry; advertising the default scope");
            None
        },
    };
    let scopes = match &registry {
        Some(registry) => registry.oauth_scopes(&server).await,
        None => None,
    }
    .unwrap_or_else(|| vec!["user".to_owned()]);

    let requires_ema = match &registry {
        Some(registry) => mcp_server_requires_ema(registry.as_ref(), &server).await,
        None => false,
    };
    let mcp_extensions_supported = if requires_ema {
        vec![McpExtensionId::EnterpriseManagedAuth]
    } else {
        Vec::new()
    };

    let resource_url = format!("{}/api/v1/mcp/{}/mcp", base_url, server);

    let metadata = ProtectedResourceMetadata {
        resource: resource_url,
        authorization_servers: vec![base_url.clone()],
        scopes_supported: scopes,
        bearer_methods_supported: vec!["header".to_owned()],
        resource_documentation: Some(base_url.clone()),
        mcp_extensions_supported,
    };

    Ok((StatusCode::OK, Json(metadata)).into_response())
}

pub(super) async fn handle_mcp_authorization_server(
    State(state): State<McpState>,
) -> Result<Response, ApiHttpError> {
    let config = state.ctx.config();
    let (base_url, allow_dcr) = (
        config.api_external_url.clone(),
        config.allow_dynamic_client_registration,
    );

    let vocabulary = AuthorizationServerVocabulary::served();
    let metadata = McpAuthorizationServerMetadata {
        issuer: base_url.clone(),
        authorization_endpoint: format!("{}/api/v1/core/oauth/authorize", base_url),
        token_endpoint: format!("{}/api/v1/core/oauth/token", base_url),
        registration_endpoint: allow_dcr
            .then(|| format!("{}/api/v1/core/oauth/register", base_url)),
        scopes_supported: vec!["user".to_owned(), "admin".to_owned()],
        response_types_supported: vocabulary.response_types,
        grant_types_supported: vocabulary.grant_types,
        code_challenge_methods_supported: vocabulary.code_challenge_methods,
        token_endpoint_auth_methods_supported: vocabulary.token_endpoint_auth_methods,
        authorization_response_iss_parameter_supported: true,
        subject_token_types_supported: vocabulary.subject_token_types,
        issued_token_types_supported: vocabulary.issued_token_types,
        authorization_grant_profiles_supported: vocabulary.grant_profiles,
    };

    Ok((StatusCode::OK, Json(metadata)).into_response())
}

async fn mcp_server_requires_ema(registry: &dyn McpRegistryProvider, server: &McpServerId) -> bool {
    match registry.get_server(server.as_str()).await {
        Ok(info) => info.oauth.required && info.oauth.ema,
        Err(_) => false,
    }
}
