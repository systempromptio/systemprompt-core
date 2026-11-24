//! OAuth discovery document construction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use std::sync::Arc;
use systemprompt_identifiers::McpServerId;
use systemprompt_manifest::profile::UiSessionDiscovery;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::oauth::{OAuthServerConfig, ProtectedResourceMetadata};
use systemprompt_oauth::services::validation::id_jag::ID_JAG_GRANT_PROFILE;
use systemprompt_security::http::RequestBaseUrl;
use systemprompt_traits::McpRegistryProvider;

/// What the discovery documents read beyond the request: whether dynamic
/// client registration is open, the UI-session lifetimes, and the MCP
/// registry a per-server protected-resource document reads its scopes from.
#[derive(Clone)]
pub struct DiscoveryState {
    pub allow_dynamic_client_registration: bool,
    pub ui_session: UiSessionDiscovery,
    pub mcp_registry: Option<Arc<dyn McpRegistryProvider>>,
}

impl DiscoveryState {
    #[must_use]
    pub fn from_config(
        config: &systemprompt_manifest::Config,
        mcp_registry: Option<Arc<dyn McpRegistryProvider>>,
    ) -> Self {
        Self {
            allow_dynamic_client_registration: config.allow_dynamic_client_registration,
            ui_session: UiSessionDiscovery::from_config(&config.ui_session),
            mcp_registry,
        }
    }
}

impl std::fmt::Debug for DiscoveryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscoveryState")
            .field("allow_dynamic_client_registration", &self.allow_dynamic_client_registration)
            .field("mcp_registry", &self.mcp_registry.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
pub struct WellKnownResponse {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub introspection_endpoint: String,
    pub revocation_endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_endpoint: Option<String>,
    pub scopes_supported: Vec<String>,
    pub response_types_supported: Vec<String>,
    pub response_modes_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
    pub claims_supported: Vec<String>,
    pub prompt_values_supported: Vec<String>,
    pub authorization_response_iss_parameter_supported: bool,
    pub subject_token_types_supported: Vec<String>,
    pub issued_token_types_supported: Vec<String>,
    pub authorization_grant_profiles_supported: Vec<String>,
    pub ui_session_supported: bool,
    pub ui_session: UiSessionDiscovery,
    pub device_link_ui_session_supported: bool,
}

pub async fn handle_well_known(
    State(state): State<DiscoveryState>,
    base: RequestBaseUrl,
) -> impl IntoResponse {
    let config = OAuthServerConfig::from_api_server_url(base.as_str());
    let registration_endpoint = state
        .allow_dynamic_client_registration
        .then(|| format!("{}/api/v1/core/oauth/register", config.issuer));

    let response = WellKnownResponse {
        issuer: config.issuer.clone(),
        authorization_endpoint: format!("{}/api/v1/core/oauth/authorize", config.issuer),
        token_endpoint: format!("{}/api/v1/core/oauth/token", config.issuer),
        userinfo_endpoint: format!("{}/api/v1/core/oauth/userinfo", config.issuer),
        introspection_endpoint: format!("{}/api/v1/core/oauth/introspect", config.issuer),
        revocation_endpoint: format!("{}/api/v1/core/oauth/revoke", config.issuer),
        registration_endpoint,
        scopes_supported: config.supported_scopes,
        response_types_supported: config.supported_response_types,
        response_modes_supported: vec!["query".to_owned()],
        grant_types_supported: config.supported_grant_types,
        token_endpoint_auth_methods_supported: vec![
            "none".to_owned(),
            "client_secret_post".to_owned(),
            "client_secret_basic".to_owned(),
        ],
        code_challenge_methods_supported: config.supported_code_challenge_methods,
        subject_types_supported: vec!["public".to_owned()],
        id_token_signing_alg_values_supported: vec!["RS256".to_owned()],
        claims_supported: vec![
            "sub".to_owned(),
            "username".to_owned(),
            "email".to_owned(),
            "user_type".to_owned(),
            "roles".to_owned(),
            "permissions".to_owned(),
            "iat".to_owned(),
            "exp".to_owned(),
            "iss".to_owned(),
            "aud".to_owned(),
            "jti".to_owned(),
        ],
        prompt_values_supported: vec![
            "none".to_owned(),
            "login".to_owned(),
            "consent".to_owned(),
            "select_account".to_owned(),
        ],
        authorization_response_iss_parameter_supported: true,
        subject_token_types_supported: vec![
            "urn:ietf:params:oauth:token-type:access_token".to_owned(),
            "urn:ietf:params:oauth:token-type:id_token".to_owned(),
            "urn:ietf:params:oauth:token-type:jwt".to_owned(),
            "urn:ietf:params:oauth:token-type:id-jag".to_owned(),
        ],
        issued_token_types_supported: vec![
            "urn:ietf:params:oauth:token-type:access_token".to_owned(),
            "urn:ietf:params:oauth:token-type:id-jag".to_owned(),
        ],
        authorization_grant_profiles_supported: vec![ID_JAG_GRANT_PROFILE.to_owned()],
        ui_session_supported: true,
        ui_session: state.ui_session,
        device_link_ui_session_supported: true,
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub async fn handle_oauth_protected_resource(base: RequestBaseUrl) -> impl IntoResponse {
    let config = OAuthServerConfig::from_api_server_url(base.as_str());

    let response = ProtectedResourceMetadata {
        resource: config.issuer.clone(),
        authorization_servers: vec![config.issuer.clone()],
        scopes_supported: config.supported_scopes,
        bearer_methods_supported: vec!["header".to_owned(), "body".to_owned()],
        resource_documentation: Some(config.issuer),
        mcp_extensions_supported: Vec::new(),
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub async fn handle_oauth_protected_resource_with_path(
    State(state): State<DiscoveryState>,
    base: RequestBaseUrl,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    let mcp_prefix = ApiPaths::MCP_BASE.trim_start_matches('/');
    let normalized = path.trim_start_matches('/');

    let service_name = normalized
        .strip_prefix(mcp_prefix)
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| {
            rest.strip_suffix("/mcp")
                .or_else(|| rest.strip_suffix("/mcp/"))
        })
        .filter(|name| !name.is_empty() && !name.contains('/'));

    let service_name = match service_name {
        Some(name) => name.to_owned(),
        None => return handle_oauth_protected_resource(base).await.into_response(),
    };

    let base_url = base.as_str().to_owned();

    let scopes = match (McpServerId::try_new(service_name.as_str()), state.mcp_registry.as_ref()) {
        (Ok(server), Some(registry)) => registry.oauth_scopes(&server).await,
        _ => None,
    }
    .unwrap_or_else(|| vec!["user".to_owned()]);
    let resource_url = format!(
        "{}{}",
        base_url,
        ApiPaths::mcp_server_endpoint(&service_name)
    );

    let response = ProtectedResourceMetadata {
        resource: resource_url,
        authorization_servers: vec![base_url.clone()],
        scopes_supported: scopes,
        bearer_methods_supported: vec!["header".to_owned()],
        resource_documentation: Some(base_url),
        mcp_extensions_supported: Vec::new(),
    };

    (StatusCode::OK, Json(response)).into_response()
}
