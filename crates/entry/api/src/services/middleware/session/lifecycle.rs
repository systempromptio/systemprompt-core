use std::sync::Arc;
use systemprompt_analytics::AnalyticsService;
use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
use systemprompt_models::api::ApiError;
use systemprompt_oauth::services::{
    CreateAnonymousSessionInput, SessionCreationError, SessionCreationService,
};

pub(super) async fn create_new_session(
    session_creation_service: &Arc<SessionCreationService>,
    headers: &http::HeaderMap,
    uri: &http::Uri,
    _method: &http::Method,
) -> Result<(SessionId, UserId, String, bool, String), ApiError> {
    let client_id = ClientId::new("sp_web".to_string());

    let jwt_secret = systemprompt_config::SecretsBootstrap::jwt_secret().map_err(|e| {
        tracing::error!(error = %e, "Failed to get JWT secret during session creation");
        ApiError::internal_error("Failed to initialize session")
    })?;

    session_creation_service
        .create_anonymous_session(CreateAnonymousSessionInput {
            headers,
            uri: Some(uri),
            client_id: &client_id,
            jwt_secret,
            session_source: SessionSource::Web,
        })
        .await
        .map(|session_info| {
            (
                session_info.session_id,
                session_info.user_id,
                session_info.jwt_token,
                session_info.is_new,
                session_info.fingerprint_hash,
            )
        })
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create anonymous session");
            ApiError::internal_error("Service temporarily unavailable")
        })
}

pub(super) async fn refresh_session_for_user(
    session_creation_service: &Arc<SessionCreationService>,
    analytics_service: &Arc<AnalyticsService>,
    user_id: &UserId,
    headers: &http::HeaderMap,
    uri: &http::Uri,
) -> Result<(SessionId, UserId, String, bool, String), ApiError> {
    let session_id = session_creation_service
        .create_authenticated_session(user_id, headers, SessionSource::Web)
        .await
        .map_err(|e| match e {
            SessionCreationError::UserNotFound { ref user_id } => {
                ApiError::not_found(format!("User not found: {}", user_id))
                    .with_error_key("user_not_found")
            },
            SessionCreationError::Internal(ref msg) => {
                tracing::error!(error = %msg, user_id = %user_id, "Failed to create session for user");
                ApiError::internal_error("Failed to refresh session")
            },
        })?;

    let jwt_secret = systemprompt_config::SecretsBootstrap::jwt_secret().map_err(|e| {
        tracing::error!(error = %e, "Failed to get JWT secret during session refresh");
        ApiError::internal_error("Failed to refresh session")
    })?;

    let config = systemprompt_models::Config::get().map_err(|e| {
        tracing::error!(error = %e, "Failed to get config during session refresh");
        ApiError::internal_error("Failed to refresh session")
    })?;

    let token = systemprompt_oauth::services::generation::generate_anonymous_jwt(
        user_id,
        &session_id,
        &ClientId::new("sp_web".to_string()),
        &systemprompt_oauth::services::JwtSigningParams {
            secret: jwt_secret,
            issuer: &config.jwt_issuer,
        },
    )
    .map_err(|e| {
        tracing::error!(error = %e, "Failed to generate JWT during session refresh");
        ApiError::internal_error("Failed to refresh session")
    })?;

    let analytics = analytics_service.extract_analytics(headers, Some(uri));
    let fingerprint = AnalyticsService::compute_fingerprint(&analytics);

    Ok((session_id, user_id.clone(), token, true, fingerprint))
}
