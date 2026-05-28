use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::oauth::OAuthServerConfig;
use systemprompt_runtime::AppContext;

use crate::routes::proxy::mcp::get_mcp_server_scopes;
use crate::services::request_base_url::RequestBaseUrl;

#[derive(Debug, Serialize)]
pub struct WellKnownResponse {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub introspection_endpoint: String,
    pub revocation_endpoint: String,
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
}

pub async fn handle_well_known(base: RequestBaseUrl) -> impl IntoResponse {
    let config = OAuthServerConfig::from_api_server_url(base.as_str());

    let response = WellKnownResponse {
        issuer: config.issuer.clone(),
        authorization_endpoint: format!("{}/api/v1/core/oauth/authorize", config.issuer),
        token_endpoint: format!("{}/api/v1/core/oauth/token", config.issuer),
        userinfo_endpoint: format!("{}/api/v1/core/oauth/userinfo", config.issuer),
        introspection_endpoint: format!("{}/api/v1/core/oauth/introspect", config.issuer),
        revocation_endpoint: format!("{}/api/v1/core/oauth/revoke", config.issuer),
        registration_endpoint: Some(format!("{}/api/v1/core/oauth/register", config.issuer)),
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
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[derive(Debug, Serialize)]
pub struct OAuthProtectedResourceResponse {
    pub resource: String,
    pub authorization_servers: Vec<String>,
    pub scopes_supported: Vec<String>,
    pub bearer_methods_supported: Vec<String>,
    pub resource_documentation: Option<String>,
}

pub async fn handle_oauth_protected_resource(base: RequestBaseUrl) -> impl IntoResponse {
    let config = OAuthServerConfig::from_api_server_url(base.as_str());

    let response = OAuthProtectedResourceResponse {
        resource: config.issuer.clone(),
        authorization_servers: vec![config.issuer.clone()],
        scopes_supported: config.supported_scopes,
        bearer_methods_supported: vec!["header".to_owned(), "body".to_owned()],
        resource_documentation: Some(config.issuer),
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub async fn handle_oauth_protected_resource_with_path(
    State(ctx): State<AppContext>,
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

    let scopes = get_mcp_server_scopes(ctx.mcp_registry(), &service_name)
        .await
        .unwrap_or_else(|| vec!["user".to_owned()]);
    let resource_url = format!(
        "{}{}",
        base_url,
        ApiPaths::mcp_server_endpoint(&service_name)
    );

    let response = OAuthProtectedResourceResponse {
        resource: resource_url,
        authorization_servers: vec![base_url.clone()],
        scopes_supported: scopes,
        bearer_methods_supported: vec!["header".to_owned()],
        resource_documentation: Some(base_url),
    };

    (StatusCode::OK, Json(response)).into_response()
}
