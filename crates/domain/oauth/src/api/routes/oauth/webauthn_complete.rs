use anyhow::Result;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::repository::{AuthCodeParams, OAuthRepository};
use crate::services::{generate_secure_token, BrowserRedirectService};
use systemprompt_core_users::{UserProviderImpl, UserService};
use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};
use systemprompt_traits::UserProvider;

#[derive(Debug, Deserialize)]
pub struct WebAuthnCompleteQuery {
    pub user_id: String,
    pub response_type: Option<String>,
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub response_mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WebAuthnCompleteError {
    pub error: String,
    pub error_description: String,
}

#[allow(unused_qualifications)]
pub async fn handle_webauthn_complete(
    headers: HeaderMap,
    Query(params): Query<WebAuthnCompleteQuery>,
    State(ctx): State<systemprompt_runtime::AppContext>,
) -> impl IntoResponse {
    let repo = match OAuthRepository::new(Arc::clone(ctx.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };
    if params.client_id.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(WebAuthnCompleteError {
                error: "invalid_request".to_string(),
                error_description: "Missing client_id parameter".to_string(),
            }),
        )
            .into_response();
    }

    let Some(redirect_uri) = &params.redirect_uri else {
        return (
            StatusCode::BAD_REQUEST,
            Json(WebAuthnCompleteError {
                error: "invalid_request".to_string(),
                error_description: "Missing redirect_uri parameter".to_string(),
            }),
        )
            .into_response();
    };

    let user_service = match UserService::new(ctx.db_pool()) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebAuthnCompleteError {
                    error: "server_error".to_string(),
                    error_description: format!("Failed to create user service: {e}"),
                }),
            )
                .into_response();
        },
    };
    let user_provider: Arc<dyn UserProvider> = Arc::new(UserProviderImpl::new(user_service));

    match user_provider.find_by_id(&params.user_id).await {
        Ok(Some(_)) => {
            let authorization_code = generate_secure_token("auth_code");

            match store_authorization_code(&repo, &authorization_code, &params).await {
                Ok(()) => {
                    create_successful_response(&headers, redirect_uri, &authorization_code, &params)
                },
                Err(error) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(WebAuthnCompleteError {
                        error: "server_error".to_string(),
                        error_description: error.to_string(),
                    }),
                )
                    .into_response(),
            }
        },
        Ok(None) => (
            StatusCode::UNAUTHORIZED,
            Json(WebAuthnCompleteError {
                error: "access_denied".to_string(),
                error_description: "User not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            let status_code = if error.to_string().contains("User not found") {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            let error_type = if status_code == StatusCode::UNAUTHORIZED {
                "access_denied"
            } else {
                "server_error"
            };

            (
                status_code,
                Json(WebAuthnCompleteError {
                    error: error_type.to_string(),
                    error_description: error.to_string(),
                }),
            )
                .into_response()
        },
    }
}

async fn store_authorization_code(
    repo: &OAuthRepository,
    code_str: &str,
    query: &WebAuthnCompleteQuery,
) -> Result<()> {
    let client_id_str = query
        .client_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("client_id is required"))?;
    let redirect_uri = query
        .redirect_uri
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("redirect_uri is required"))?;
    let scope = if let Some(scope_str) = &query.scope {
        scope_str.clone()
    } else {
        let default_roles = repo
            .get_default_roles()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get default roles: {e}"))?;
        if default_roles.is_empty() {
            "user".to_string()
        } else {
            default_roles.join(" ")
        }
    };

    let code = AuthorizationCode::new(code_str);
    let client_id = ClientId::new(client_id_str);
    let user_id = UserId::new(&query.user_id);

    let mut builder = AuthCodeParams::builder(&code, &client_id, &user_id, redirect_uri, &scope);

    if let (Some(challenge), Some(method)) = (
        query.code_challenge.as_deref(),
        query
            .code_challenge_method
            .as_deref()
            .filter(|s| !s.is_empty()),
    ) {
        builder = builder.with_pkce(challenge, method);
    }

    repo.store_authorization_code(builder.build()).await
}

#[derive(Debug, Serialize)]
pub struct WebAuthnCompleteResponse {
    pub authorization_code: String,
    pub state: String,
    pub redirect_uri: String,
    pub client_id: String,
}

fn create_successful_response(
    headers: &HeaderMap,
    redirect_uri: &str,
    authorization_code: &str,
    params: &WebAuthnCompleteQuery,
) -> axum::response::Response {
    let state = params.state.as_deref().filter(|s| !s.is_empty());

    if BrowserRedirectService::is_browser_request(headers) {
        let mut target = format!("{redirect_uri}?code={authorization_code}");

        if let Some(client_id_val) = params.client_id.as_deref() {
            target.push_str(&format!(
                "&client_id={}",
                urlencoding::encode(client_id_val)
            ));
        }

        if let Some(state_val) = state {
            target.push_str(&format!("&state={}", urlencoding::encode(state_val)));
        }
        Redirect::to(&target).into_response()
    } else {
        let response_data = WebAuthnCompleteResponse {
            authorization_code: authorization_code.to_string(),
            state: state.unwrap_or_default().to_string(),
            redirect_uri: redirect_uri.to_string(),
            client_id: params.client_id.as_deref().unwrap_or_default().to_string(),
        };

        let mut response = Json(response_data).into_response();

        let headers = response.headers_mut();
        headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
        headers.insert(
            "access-control-allow-methods",
            HeaderValue::from_static("GET, POST, OPTIONS"),
        );
        headers.insert(
            "access-control-allow-headers",
            HeaderValue::from_static("content-type, authorization"),
        );

        response
    }
}
