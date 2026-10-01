//! Private decode targets for `sqlx::query_as!`: the macro converts each
//! column with `From<inferred type>`, which the validating identifier types
//! deliberately do not implement, so rows decode into plain strings here and
//! become typed ids through the trusted `new` constructor (a row is trusted).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ApiKeyId, DeviceCertId, UserId};

use super::{User, UserActivity, UserApiKey, UserDeviceCert, UserWithSessions};

#[derive(Debug)]
pub(crate) struct UserRow {
    pub id: String,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub display_name: Option<String>,
    pub status: Option<String>,
    pub email_verified: Option<bool>,
    pub roles: Vec<String>,
    pub avatar_url: Option<String>,
    pub is_bot: bool,
    pub is_scanner: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: UserId::new(row.id),
            name: row.name,
            email: row.email,
            full_name: row.full_name,
            display_name: row.display_name,
            status: row.status,
            email_verified: row.email_verified,
            roles: row.roles,
            avatar_url: row.avatar_url,
            is_bot: row.is_bot,
            is_scanner: row.is_scanner,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug)]
pub(crate) struct UserActivityRow {
    pub user_id: String,
    pub last_active: Option<DateTime<Utc>>,
    pub session_count: i64,
    pub task_count: i64,
    pub message_count: i64,
}

impl From<UserActivityRow> for UserActivity {
    fn from(row: UserActivityRow) -> Self {
        Self {
            user_id: UserId::new(row.user_id),
            last_active: row.last_active,
            session_count: row.session_count,
            task_count: row.task_count,
            message_count: row.message_count,
        }
    }
}

#[derive(Debug)]
pub(crate) struct UserWithSessionsRow {
    pub id: String,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub status: Option<String>,
    pub roles: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub active_sessions: i64,
    pub last_session_at: Option<DateTime<Utc>>,
}

impl From<UserWithSessionsRow> for UserWithSessions {
    fn from(row: UserWithSessionsRow) -> Self {
        Self {
            id: UserId::new(row.id),
            name: row.name,
            email: row.email,
            full_name: row.full_name,
            status: row.status,
            roles: row.roles,
            created_at: row.created_at,
            active_sessions: row.active_sessions,
            last_session_at: row.last_session_at,
        }
    }
}

#[derive(Debug)]
pub(crate) struct UserApiKeyRow {
    pub id: ApiKeyId,
    pub user_id: String,
    pub name: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub created_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl From<UserApiKeyRow> for UserApiKey {
    fn from(row: UserApiKeyRow) -> Self {
        Self {
            id: row.id,
            user_id: UserId::new(row.user_id),
            name: row.name,
            key_prefix: row.key_prefix,
            key_hash: row.key_hash,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
            expires_at: row.expires_at,
            revoked_at: row.revoked_at,
        }
    }
}

#[derive(Debug)]
pub(crate) struct UserDeviceCertRow {
    pub id: DeviceCertId,
    pub user_id: String,
    pub fingerprint: String,
    pub label: String,
    pub enrolled_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl From<UserDeviceCertRow> for UserDeviceCert {
    fn from(row: UserDeviceCertRow) -> Self {
        Self {
            id: row.id,
            user_id: UserId::new(row.user_id),
            fingerprint: row.fingerprint,
            label: row.label,
            enrolled_at: row.enrolled_at,
            revoked_at: row.revoked_at,
        }
    }
}
