//! JWT validation middleware with a short-TTL user cache.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use systemprompt_identifiers::UserId;
use systemprompt_models::auth::UserRole;
use systemprompt_models::execution::context::ContextExtractionError;
use systemprompt_security::JwtUserContext;
use systemprompt_traits::{AuthUser, SessionProvider, UserProvider};

use crate::services::middleware::session::{SessionAttestationError, attest_session};

const USER_CACHE_TTL: Duration = Duration::from_secs(30);
const USER_CACHE_CAPACITY: usize = 4096;

#[derive(Debug)]
pub struct ValidatedUser {
    pub user: AuthUser,
}

#[derive(Debug)]
pub struct UserCache {
    entries: Mutex<HashMap<UserId, (AuthUser, Instant)>>,
    ttl: Duration,
}

impl Default for UserCache {
    fn default() -> Self {
        Self::with_ttl(USER_CACHE_TTL)
    }
}

impl UserCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    pub fn get_fresh(&self, user_id: &UserId) -> Option<AuthUser> {
        let guard = self.entries.lock().ok()?;
        let fresh = guard
            .get(user_id)
            .and_then(|(user, fetched_at)| (fetched_at.elapsed() < self.ttl).then(|| user.clone()));
        drop(guard);
        fresh
    }

    pub fn put(&self, user_id: UserId, user: AuthUser) {
        if let Ok(mut guard) = self.entries.lock() {
            if guard.len() >= USER_CACHE_CAPACITY && !guard.contains_key(&user_id) {
                let ttl = self.ttl;
                guard.retain(|_, (_, fetched_at)| fetched_at.elapsed() < ttl);
                if guard.len() >= USER_CACHE_CAPACITY {
                    guard.clear();
                }
            }
            guard.insert(user_id, (user, Instant::now()));
        }
    }
}

pub async fn validate_user_exists(
    user_provider: &Arc<dyn UserProvider>,
    cache: &Arc<UserCache>,
    user_id: &UserId,
    route_context: &str,
) -> Result<ValidatedUser, ContextExtractionError> {
    if let Some(user) = cache.get_fresh(user_id) {
        return require_active(user, user_id, route_context);
    }

    let user = user_provider
        .find_by_id(user_id)
        .await
        .map_err(|e| ContextExtractionError::DatabaseError {
            context: "Failed to check user existence".to_owned(),
            source: e.into(),
        })?
        .ok_or_else(|| {
            tracing::info!(
                user_id = %user_id.as_str(),
                route = %route_context,
                "credential validation failed: user no longer exists in database"
            );
            ContextExtractionError::UserNotFound(format!(
                "User {} no longer exists",
                user_id.as_str()
            ))
        })?;

    cache.put(user_id.clone(), user.clone());
    require_active(user, user_id, route_context)
}

fn require_active(
    user: AuthUser,
    user_id: &UserId,
    route_context: &str,
) -> Result<ValidatedUser, ContextExtractionError> {
    if !user.is_active {
        tracing::info!(
            user_id = %user_id.as_str(),
            route = %route_context,
            "credential validation failed: user is not active"
        );
        return Err(ContextExtractionError::UserNotFound(format!(
            "User {} is not active",
            user_id.as_str()
        )));
    }
    Ok(ValidatedUser { user })
}

pub fn user_is_admin(user: &AuthUser) -> bool {
    user.roles
        .iter()
        .any(|r| r.as_str() == UserRole::Admin.as_str())
}

pub(super) async fn validate_session_exists(
    session_provider: &Arc<dyn SessionProvider>,
    jwt_context: &JwtUserContext,
    route_context: &str,
) -> Result<(), ContextExtractionError> {
    attest_session(
        session_provider,
        &jwt_context.session_id,
        &jwt_context.user_id,
        route_context,
    )
    .await
    .map_err(|e| match e {
        SessionAttestationError::Lookup(source) => ContextExtractionError::DatabaseError {
            context: "Failed to check session".to_owned(),
            source: Box::new(source),
        },
        other => ContextExtractionError::InvalidToken(other.into()),
    })
}
