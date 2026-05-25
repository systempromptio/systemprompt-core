use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use systemprompt_identifiers::UserId;
use systemprompt_models::auth::UserRole;
use systemprompt_models::execution::context::ContextExtractionError;
use systemprompt_traits::{AnalyticsProvider, AuthUser, UserProvider};

use super::token::JwtUserContext;

const USER_CACHE_TTL: Duration = Duration::from_secs(30);

pub(super) struct ValidatedUser {
    pub user: AuthUser,
}

// Why mutex (not RwLock): writes happen on every fetch (TTL refresh), so a
// reader-writer split would barely help; the contention window is the
// negligible HashMap lookup. No `.await` is held across the guard.
#[derive(Default)]
pub(super) struct UserCache {
    entries: Mutex<HashMap<UserId, (AuthUser, Instant)>>,
}

impl UserCache {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            entries: Mutex::new(HashMap::new()),
        })
    }

    fn get_fresh(&self, user_id: &UserId) -> Option<AuthUser> {
        let guard = self.entries.lock().ok()?;
        let fresh = guard.get(user_id).and_then(|(user, fetched_at)| {
            (fetched_at.elapsed() < USER_CACHE_TTL).then(|| user.clone())
        });
        drop(guard);
        fresh
    }

    pub(crate) fn put(&self, user_id: UserId, user: AuthUser) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.insert(user_id, (user, Instant::now()));
        }
    }
}

pub(super) async fn validate_user_exists(
    user_provider: &Arc<dyn UserProvider>,
    cache: &Arc<UserCache>,
    jwt_context: &JwtUserContext,
    route_context: &str,
) -> Result<ValidatedUser, ContextExtractionError> {
    if let Some(user) = cache.get_fresh(&jwt_context.user_id) {
        return require_active(user, jwt_context, route_context);
    }

    let user = user_provider
        .find_by_id(&jwt_context.user_id)
        .await
        .map_err(|e| {
            ContextExtractionError::DatabaseError(format!("Failed to check user existence: {e}"))
        })?
        .ok_or_else(|| {
            tracing::info!(
                session_id = %jwt_context.session_id.as_str(),
                user_id = %jwt_context.user_id.as_str(),
                route = %route_context,
                "JWT validation failed: user no longer exists in database"
            );
            ContextExtractionError::UserNotFound(format!(
                "User {} no longer exists",
                jwt_context.user_id.as_str()
            ))
        })?;

    cache.put(jwt_context.user_id.clone(), user.clone());
    require_active(user, jwt_context, route_context)
}

fn require_active(
    user: AuthUser,
    jwt_context: &JwtUserContext,
    route_context: &str,
) -> Result<ValidatedUser, ContextExtractionError> {
    if !user.is_active {
        tracing::info!(
            session_id = %jwt_context.session_id.as_str(),
            user_id = %jwt_context.user_id.as_str(),
            route = %route_context,
            "JWT validation failed: user is not active"
        );
        return Err(ContextExtractionError::UserNotFound(format!(
            "User {} is not active",
            jwt_context.user_id.as_str()
        )));
    }
    Ok(ValidatedUser { user })
}

pub(super) fn user_is_admin(user: &AuthUser) -> bool {
    user.roles
        .iter()
        .any(|r| r.as_str() == UserRole::Admin.as_str())
}

pub(super) async fn validate_session_exists(
    analytics_provider: &Arc<dyn AnalyticsProvider>,
    jwt_context: &JwtUserContext,
    route_context: &str,
) -> Result<(), ContextExtractionError> {
    let session = analytics_provider
        .find_active_session_by_id(&jwt_context.session_id)
        .await
        .map_err(|e| {
            ContextExtractionError::DatabaseError(format!("Failed to check session: {e}"))
        })?;

    let Some(session) = session else {
        tracing::info!(
            session_id = %jwt_context.session_id.as_str(),
            user_id = %jwt_context.user_id.as_str(),
            route = %route_context,
            "JWT validation failed: session missing or revoked"
        );
        return Err(ContextExtractionError::InvalidToken(
            "Session missing or revoked".to_owned(),
        ));
    };

    if let Some(session_user_id) = session.user_id.as_ref() {
        if session_user_id.as_str() != jwt_context.user_id.as_str() {
            tracing::warn!(
                session_id = %jwt_context.session_id.as_str(),
                claimed_user_id = %jwt_context.user_id.as_str(),
                session_user_id = %session_user_id.as_str(),
                route = %route_context,
                "JWT validation failed: session user mismatch"
            );
            return Err(ContextExtractionError::InvalidToken(
                "Session user mismatch".to_owned(),
            ));
        }
    }

    Ok(())
}
