//! RFC 9728 protected-resource and authorization-server metadata per MCP
//! service, including the RFC 8693 token-type and EMA advertisements.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use systemprompt_models::Config;
use systemprompt_models::mcp::McpExtensionId;
use systemprompt_models::oauth::ProtectedResourceMetadata;
use systemprompt_oauth::services::validation::id_jag::{ID_JAG_GRANT_PROFILE, ID_JAG_TOKEN_TYPE};
use systemprompt_oauth::{GrantType, PkceMethod, ResponseType, TokenAuthMethod};
use systemprompt_traits::McpRegistryProvider;

use super::{McpState, get_mcp_server_scopes};

const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";
const ID_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:id_token";

#[derive(Debug, Serialize)]
struct McpAuthorizationServerMetadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
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
    Path(service_name): Path<String>,
) -> impl IntoResponse {
    let base_url = match Config::get() {
        Ok(c) => c.api_external_url.clone(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get config");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Configuration unavailable"})),
            )
                .into_response();
        },
    };

    let scopes = get_mcp_server_scopes(state.ctx.mcp_registry(), &service_name)
        .await
        .unwrap_or_else(|| vec!["user".to_owned()]);

    let mcp_extensions_supported =
        if mcp_server_requires_ema(state.ctx.mcp_registry(), &service_name).await {
            vec![McpExtensionId::EnterpriseManagedAuth]
        } else {
            Vec::new()
        };

    let resource_url = format!("{}/api/v1/mcp/{}/mcp", base_url, service_name);

    let metadata = ProtectedResourceMetadata {
        resource: resource_url,
        authorization_servers: vec![base_url.clone()],
        scopes_supported: scopes,
        bearer_methods_supported: vec!["header".to_owned()],
        resource_documentation: Some(base_url.clone()),
        mcp_extensions_supported,
    };

    (StatusCode::OK, Json(metadata)).into_response()
}

pub(super) async fn handle_mcp_authorization_server(
    Path(_service_name): Path<String>,
) -> impl IntoResponse {
    let base_url = match Config::get() {
        Ok(c) => c.api_external_url.clone(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get config");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Configuration unavailable"})),
            )
                .into_response();
        },
    };

    let metadata = McpAuthorizationServerMetadata {
        issuer: base_url.clone(),
        authorization_endpoint: format!("{}/api/v1/core/oauth/authorize", base_url),
        token_endpoint: format!("{}/api/v1/core/oauth/token", base_url),
        registration_endpoint: Some(format!("{}/api/v1/core/oauth/register", base_url)),
        scopes_supported: vec!["user".to_owned(), "admin".to_owned()],
        response_types_supported: vec![ResponseType::Code.to_string()],
        grant_types_supported: vec![
            GrantType::AuthorizationCode.to_string(),
            GrantType::RefreshToken.to_string(),
            GrantType::TokenExchange.to_string(),
            GrantType::JwtBearer.to_string(),
        ],
        code_challenge_methods_supported: vec![PkceMethod::S256.to_string()],
        token_endpoint_auth_methods_supported: vec![
            TokenAuthMethod::None.to_string(),
            TokenAuthMethod::ClientSecretPost.to_string(),
            TokenAuthMethod::ClientSecretBasic.to_string(),
        ],
        authorization_response_iss_parameter_supported: true,
        subject_token_types_supported: vec![
            ACCESS_TOKEN_TYPE.to_owned(),
            ID_TOKEN_TYPE.to_owned(),
            ID_JAG_TOKEN_TYPE.to_owned(),
        ],
        issued_token_types_supported: vec![
            ACCESS_TOKEN_TYPE.to_owned(),
            ID_JAG_TOKEN_TYPE.to_owned(),
        ],
        authorization_grant_profiles_supported: vec![ID_JAG_GRANT_PROFILE.to_owned()],
    };

    (StatusCode::OK, Json(metadata)).into_response()
}

async fn mcp_server_requires_ema(registry: &dyn McpRegistryProvider, service_name: &str) -> bool {
    match registry.get_server(service_name).await {
        Ok(info) => info.oauth.required && info.oauth.ema,
        Err(_) => false,
    }
}
