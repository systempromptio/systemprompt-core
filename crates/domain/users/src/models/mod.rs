//! Data types for the users domain.
//!
//! Defines the persisted [`User`] record and its projections
//! ([`UserActivity`], [`UserWithSessions`], [`UserStats`],
//! [`UserCountBreakdown`], [`UserExport`]), session rows
//! ([`UserSession`]), and the credential records
//! [`UserApiKey`] / [`NewApiKey`] and [`UserDeviceCert`]. Role and status
//! enums are re-exported from `systemprompt_models::auth`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{ApiKeyId, DeviceCertId, SessionId, UserId};
use systemprompt_models::attribution::ScopeBinding;

pub use systemprompt_models::auth::{UserRole, UserStatus};

mod rows;
pub(crate) use rows::{
    UserActivityRow, UserApiKeyRow, UserDeviceCertRow, UserRow, UserWithSessionsRow,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub display_name: Option<String>,
    pub status: UserStatus,
    pub email_verified: bool,
    pub roles: Vec<String>,
    pub avatar_url: Option<String>,
    pub is_bot: bool,
    pub is_scanner: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[must_use]
pub fn normalise_email(email: &str) -> String {
    email.trim().to_lowercase()
}

impl User {
    pub const fn is_active(&self) -> bool {
        self.status.is_active()
    }

    pub fn is_admin(&self) -> bool {
        self.has_role(UserRole::Admin)
    }

    pub fn has_role(&self, role: UserRole) -> bool {
        self.roles.iter().any(|held| held == role.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserActivity {
    #[sqlx(try_from = "String")]
    pub user_id: UserId,
    pub last_active: Option<DateTime<Utc>>,
    pub session_count: i64,
    pub task_count: i64,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserWithSessions {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub status: UserStatus,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub active_sessions: i64,
    pub last_session_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserSession {
    pub session_id: SessionId,
    pub user_id: Option<UserId>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_type: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UserStats {
    pub total: i64,
    pub created_24h: i64,
    pub created_7d: i64,
    pub created_30d: i64,
    pub active: i64,
    pub suspended: i64,
    pub admins: i64,
    pub anonymous: i64,
    pub bots: i64,
    pub oldest_user: Option<DateTime<Utc>>,
    pub newest_user: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCountBreakdown {
    pub total: i64,
    pub by_status: std::collections::HashMap<String, i64>,
    pub by_role: std::collections::HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserExport {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub display_name: Option<String>,
    pub status: UserStatus,
    pub email_verified: bool,
    pub roles: Vec<String>,
    pub is_bot: bool,
    pub is_scanner: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The limits an API key carries: an optional model allowlist, and a spend
/// budget and request ceiling counted over `request_window_seconds`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyLimits {
    pub model_allowlist: Option<Vec<String>>,
    pub budget_microdollars: Option<i64>,
    pub max_requests: Option<i32>,
    pub request_window_seconds: Option<i32>,
}

impl ApiKeyLimits {
    #[must_use]
    pub fn allows_model(&self, model: &str) -> bool {
        self.model_allowlist
            .as_ref()
            .is_none_or(|allowed| allowed.iter().any(|m| m == model))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserApiKey {
    pub id: ApiKeyId,
    pub user_id: UserId,
    pub name: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub created_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub limits: ApiKeyLimits,
    pub scopes: Vec<ScopeBinding>,
}

impl UserApiKey {
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        if self.revoked_at.is_some() {
            return false;
        }
        if let Some(expires_at) = self.expires_at
            && now >= expires_at
        {
            return false;
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct NewApiKey {
    pub record: UserApiKey,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserDeviceCert {
    #[sqlx(try_from = "String")]
    pub id: DeviceCertId,
    #[sqlx(try_from = "String")]
    pub user_id: UserId,
    pub fingerprint: String,
    pub label: String,
    pub enrolled_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl UserDeviceCert {
    pub const fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }
}

impl From<User> for UserExport {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            name: user.name,
            email: user.email,
            full_name: user.full_name,
            display_name: user.display_name,
            status: user.status,
            email_verified: user.email_verified,
            roles: user.roles,
            is_bot: user.is_bot,
            is_scanner: user.is_scanner,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}
