use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Form, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validate_jwt_token;
use systemprompt_oauth::services::validation::validate_client_credentials;
use systemprompt_oauth::OAuthState;

#[derive(Debug, Deserialize)]

pub struct IntrospectRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Serialize)]

pub struct IntrospectResponse {
    pub active: bool,
    pub scope: Option<String>,
    pub client_id: Option<String>,
    pub username: Option<String>,
    pub token_type: Option<String>,

    pub exp: Option<i64>,

    pub iat: Option<i64>,
    pub sub: Option<String>,
    #[serde(default)]
    pub aud: Vec<String>,
    pub iss: Option<String>,
    pub jti: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IntrospectError {
    pub error: String,
    pub error_description: Option<String>,
}

pub async fn handle_introspect(
    State(state): State<OAuthState>,
    Form(request): Form<IntrospectRequest>,
) -> impl IntoResponse {
    let repo = match OAuthRepository::new(Arc::clone(state.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };
    if let Some(client_id) = &request.client_id {
        if validate_client_credentials(&repo, client_id, request.client_secret.as_deref())
            .await
            .is_err()
        {
            let error = IntrospectError {
                error: "invalid_client".to_string(),
                error_description: Some("Invalid client credentials".to_string()),
            };
            return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
        }
    }

    match introspect_token(&repo, &request.token) {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => {
            let error = IntrospectError {
                error: "server_error".to_string(),
                error_description: Some(error.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response()
        },
    }
}

fn introspect_token(_repo: &OAuthRepository, token: &str) -> Result<IntrospectResponse> {
    let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()?;
    let config = systemprompt_models::Config::get()?;
    match validate_jwt_token(token, jwt_secret, &config.jwt_issuer, &config.jwt_audiences) {
        Ok(claims) => Ok(IntrospectResponse {
            active: true,
            scope: Some(systemprompt_models::auth::permissions_to_string(
                &claims.scope,
            )),
            client_id: claims.client_id.clone(),
            username: Some(claims.username),
            token_type: Some("Bearer".to_string()),
            exp: Some(claims.exp),
            iat: Some(claims.iat),
            sub: Some(claims.sub),
            aud: claims.aud.iter().map(ToString::to_string).collect(),
            iss: Some(claims.iss),
            jti: Some(claims.jti),
        }),
        Err(_) => Ok(IntrospectResponse {
            active: false,
            scope: None,
            client_id: None,
            username: None,
            token_type: None,
            exp: None,
            iat: None,
            sub: None,
            aud: Vec::new(),
            iss: None,
            jti: None,
        }),
    }
}
