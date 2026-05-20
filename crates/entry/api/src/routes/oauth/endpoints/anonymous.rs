use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
use systemprompt_models::auth::TokenType;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::cimd::ClientValidator;
use systemprompt_oauth::services::{CreateAnonymousSessionInput, SessionCreationService};

#[derive(Debug, Serialize)]
pub struct AnonymousTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub session_id: SessionId,
    pub user_id: UserId,
    pub client_id: ClientId,
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
    pub client_id: ClientId,
    #[serde(default)]
    pub redirect_uri: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

fn default_client_id() -> ClientId {
    ClientId::new("sp_web")
}

fn server_error(description: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AnonymousError {
            error: "server_error".to_string(),
            error_description: description,
        }),
    )
        .into_response()
}

fn token_response(body: AnonymousTokenResponse, jwt_token: &str, expires_in: i64) -> Response {
    let cookie =
        format!("access_token={jwt_token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={expires_in}");
    let mut response = (StatusCode::OK, Json(body)).into_response();
    if let Ok(cookie_value) = HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, cookie_value);
    }
    response
}

async fn issue_anonymous_session(
    session_service: &SessionCreationService,
    req: &AnonymousTokenRequest,
    headers: &HeaderMap,
    client_type: String,
    expires_in: i64,
) -> Response {
    let client_id = req.client_id.clone();
    let session_source = SessionSource::from_client_id(&req.client_id);

    match session_service
        .create_anonymous_session(CreateAnonymousSessionInput {
            headers,
            uri: None,
            client_id: &client_id,
            session_source,
        })
        .await
    {
        Ok(session_info) => {
            let jwt_token = session_info.jwt_token;
            let body = AnonymousTokenResponse {
                access_token: jwt_token.clone(),
                token_type: TokenType::Bearer.to_string(),
                expires_in,
                session_id: session_info.session_id,
                user_id: session_info.user_id,
                client_id,
                client_type,
            };
            token_response(body, &jwt_token, expires_in)
        },
        Err(e) => server_error(format!("Failed to create session: {e}")),
    }
}

fn build_session_service(state: &OAuthState) -> SessionCreationService {
    let mut session_service = SessionCreationService::new(
        Arc::clone(state.analytics_provider()),
        Arc::clone(state.user_provider()),
    );
    if let Some(fp_provider) = state.fingerprint_provider() {
        session_service = session_service.with_fingerprint_provider(Arc::clone(fp_provider));
    }
    if let Some(event_publisher) = state.event_publisher() {
        session_service = session_service.with_event_publisher(Arc::clone(event_publisher));
    }
    session_service
}

pub async fn generate_anonymous_token(
    State(state): State<OAuthState>,
    headers: HeaderMap,
    Json(req): Json<AnonymousTokenRequest>,
) -> impl IntoResponse {
    let expires_in = systemprompt_oauth::constants::token::ANONYMOUS_TOKEN_EXPIRY_SECONDS;
    let client_id = req.client_id.clone();

    let validator = match ClientValidator::new(state.db_pool()) {
        Ok(v) => v,
        Err(e) => return server_error(format!("Failed to create client validator: {e}")),
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

    let session_service = build_session_service(&state);

    issue_anonymous_session(
        &session_service,
        &req,
        &headers,
        client_type.to_string(),
        expires_in,
    )
    .await
}
