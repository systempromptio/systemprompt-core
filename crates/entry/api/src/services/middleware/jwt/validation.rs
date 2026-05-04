use axum::http::HeaderMap;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SessionSource;
use systemprompt_models::execution::context::ContextExtractionError;
use systemprompt_traits::{AnalyticsProvider, CreateSessionInput};
use systemprompt_users::UserService;

use super::token::JwtUserContext;

pub(super) async fn validate_user_exists(
    db_pool: &DbPool,
    jwt_context: &JwtUserContext,
    route_context: &str,
) -> Result<(), ContextExtractionError> {
    let user_service = UserService::new(db_pool).map_err(|e| {
        ContextExtractionError::DatabaseError(format!("Failed to create user service: {e}"))
    })?;
    let user_exists = user_service
        .find_by_id(&jwt_context.user_id)
        .await
        .map_err(|e| {
            ContextExtractionError::DatabaseError(format!("Failed to check user existence: {e}"))
        })?;

    if user_exists.is_none() {
        tracing::info!(
            session_id = %jwt_context.session_id.as_str(),
            user_id = %jwt_context.user_id.as_str(),
            route = %route_context,
            "JWT validation failed: User no longer exists in database"
        );

        return Err(ContextExtractionError::UserNotFound(format!(
            "User {} no longer exists",
            jwt_context.user_id.as_str()
        )));
    }
    Ok(())
}

pub(super) async fn validate_session_exists(
    analytics_provider: Option<&Arc<dyn AnalyticsProvider>>,
    jwt_context: &JwtUserContext,
    headers: &HeaderMap,
    route_context: &str,
) -> Result<(), ContextExtractionError> {
    let Some(analytics_provider) = analytics_provider else {
        return Ok(());
    };

    let session_exists = analytics_provider
        .find_session_by_id(&jwt_context.session_id)
        .await
        .map_err(|e| {
            ContextExtractionError::DatabaseError(format!("Failed to check session: {e}"))
        })?
        .is_some();

    if session_exists {
        return Ok(());
    }

    tracing::info!(
        session_id = %jwt_context.session_id.as_str(),
        user_id = %jwt_context.user_id.as_str(),
        route = %route_context,
        "Creating missing session for legacy token"
    );

    let config = systemprompt_models::Config::get()
        .map_err(|e| ContextExtractionError::DatabaseError(format!("Failed to get config: {e}")))?;
    let expires_at =
        chrono::Utc::now() + chrono::Duration::seconds(config.jwt_access_token_expiration);
    let analytics = analytics_provider.extract_analytics(headers, None);
    let session_source = jwt_context
        .client_id
        .as_ref()
        .map_or(SessionSource::Api, |c| {
            SessionSource::from_client_id(c.as_str())
        });

    analytics_provider
        .create_session(CreateSessionInput {
            session_id: &jwt_context.session_id,
            user_id: Some(&jwt_context.user_id),
            analytics: &analytics,
            session_source,
            is_bot: false,
            is_ai_crawler: false,
            expires_at,
        })
        .await
        .map_err(|e| {
            ContextExtractionError::DatabaseError(format!("Failed to create session: {e}"))
        })?;

    Ok(())
}
