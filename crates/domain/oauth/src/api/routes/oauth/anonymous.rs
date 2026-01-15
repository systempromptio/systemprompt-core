use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::services::cimd::ClientValidator;
use crate::services::{
    generate_admin_jwt, CreateAnonymousSessionInput, JwtSigningParams, SessionCreationService,
};
use systemprompt_core_users::{UserProviderImpl, UserService};
use systemprompt_identifiers::{ClientId, SessionSource, UserId};
use systemprompt_models::auth::TokenType;
use systemprompt_runtime::AppContext;

#[derive(Debug, Serialize)]
pub struct AnonymousTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub session_id: String,
    pub user_id: String,
    pub client_id: String,
    pub client_type: String,
}

#[derive(Debug, Serialize)]
pub struct AnonymousError {
    pub error: String,
    pub error_description: String,
}

#[derive(Debug, Deserialize)]
pub struct AnonymousTokenRequest {
    #[serde(default = "default_client_id")]
    pub client_id: String,
    #[serde(default)]
    pub redirect_uri: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// Optional user_id for authenticated TUI sessions
    #[serde(default)]
    pub user_id: Option<String>,
    /// Optional email for TUI sessions (used in JWT claims)
    #[serde(default)]
    pub email: Option<String>,
}

fn default_client_id() -> String {
    "sp_web".to_string()
}

pub async fn generate_anonymous_token(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(req): Json<AnonymousTokenRequest>,
) -> impl IntoResponse {
    let expires_in = 24 * 3600;
    let client_id = ClientId::new(req.client_id.clone());
    let validator = match ClientValidator::new(Arc::clone(ctx.db_pool())) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnonymousError {
                    error: "server_error".to_string(),
                    error_description: format!("Failed to create client validator: {e}"),
                }),
            )
                .into_response();
        },
    };

    let validation = match validator
        .validate_client(&client_id, req.redirect_uri.as_deref())
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AnonymousError {
                    error: "invalid_client".to_string(),
                    error_description: format!("Client validation failed: {e}"),
                }),
            )
                .into_response();
        },
    };

    let client_type = validation.client_type();

    let user_service = match UserService::new(ctx.db_pool()) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnonymousError {
                    error: "server_error".to_string(),
                    error_description: format!("Failed to create user service: {e}"),
                }),
            )
                .into_response();
        },
    };
    let user_provider = Arc::new(UserProviderImpl::new(user_service));
    let session_service =
        SessionCreationService::new(Arc::clone(ctx.analytics_service()), user_provider);

    // Check if this is a TUI session (has user_id and client_id is sp_tui)
    if let Some(ref user_id_str) = req.user_id {
        if req.client_id == "sp_tui" {
            let user_id = UserId::new(user_id_str.clone());
            let email = req.email.clone().unwrap_or_else(|| user_id_str.clone());

            match session_service
                .create_authenticated_session(&user_id, &headers, SessionSource::Tui)
                .await
            {
                Ok(session_id) => {
                    // Generate admin JWT for TUI
                    let jwt_secret = match systemprompt_models::SecretsBootstrap::jwt_secret() {
                        Ok(s) => s,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(AnonymousError {
                                    error: "server_error".to_string(),
                                    error_description: format!("Failed to get JWT secret: {e}"),
                                }),
                            )
                                .into_response();
                        },
                    };
                    let config = match systemprompt_models::Config::get() {
                        Ok(c) => c,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(AnonymousError {
                                    error: "server_error".to_string(),
                                    error_description: format!("Failed to get config: {e}"),
                                }),
                            )
                                .into_response();
                        },
                    };
                    let signing = JwtSigningParams {
                        secret: jwt_secret,
                        issuer: &config.jwt_issuer,
                    };
                    let jwt_token = match generate_admin_jwt(
                        user_id_str,
                        session_id.as_str(),
                        &email,
                        &client_id,
                        &signing,
                    ) {
                        Ok(token) => token,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(AnonymousError {
                                    error: "server_error".to_string(),
                                    error_description: format!("Failed to generate JWT: {e}"),
                                }),
                            )
                                .into_response();
                        },
                    };

                    let cookie = format!(
                        "access_token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                        jwt_token, expires_in
                    );

                    let mut response = (
                        StatusCode::OK,
                        Json(AnonymousTokenResponse {
                            access_token: jwt_token,
                            token_type: TokenType::Bearer.to_string(),
                            expires_in,
                            session_id: session_id.to_string(),
                            user_id: user_id_str.clone(),
                            client_id: client_id.to_string(),
                            client_type: "tui".to_string(),
                        }),
                    )
                        .into_response();

                    if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
                        response
                            .headers_mut()
                            .insert(header::SET_COOKIE, cookie_value);
                    }

                    return response;
                },
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AnonymousError {
                            error: "server_error".to_string(),
                            error_description: format!("Failed to create TUI session: {e}"),
                        }),
                    )
                        .into_response();
                },
            }
        }
    }

    let jwt_secret = match systemprompt_models::SecretsBootstrap::jwt_secret() {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnonymousError {
                    error: "server_error".to_string(),
                    error_description: format!("Failed to get JWT secret: {e}"),
                }),
            )
                .into_response();
        },
    };
    let session_source = SessionSource::from_client_id(&req.client_id);
    match session_service
        .create_anonymous_session(CreateAnonymousSessionInput {
            headers: &headers,
            uri: None,
            client_id: &client_id,
            jwt_secret,
            session_source,
        })
        .await
    {
        Ok(session_info) => {
            let cookie = format!(
                "access_token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                session_info.jwt_token, expires_in
            );

            let mut response = (
                StatusCode::OK,
                Json(AnonymousTokenResponse {
                    access_token: session_info.jwt_token,
                    token_type: TokenType::Bearer.to_string(),
                    expires_in,
                    session_id: session_info.session_id.to_string(),
                    user_id: session_info.user_id.to_string(),
                    client_id: client_id.to_string(),
                    client_type: client_type.to_string(),
                }),
            )
                .into_response();

            if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
                response
                    .headers_mut()
                    .insert(header::SET_COOKIE, cookie_value);
            }

            response
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AnonymousError {
                error: "server_error".to_string(),
                error_description: format!("Failed to create session: {e}"),
            }),
        )
            .into_response(),
    }
}
