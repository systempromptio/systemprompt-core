use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::{SessionId, UserId};

pub use systemprompt_models::auth::{UserRole, UserStatus};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    #[sqlx(try_from = "String")]
    pub id: UserId,
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

impl User {
    pub fn is_active(&self) -> bool {
        self.status.as_deref() == Some(UserStatus::Active.as_str())
    }

    pub fn is_admin(&self) -> bool {
        self.roles.contains(&UserRole::Admin.as_str().to_string())
    }

    pub fn has_role(&self, role: UserRole) -> bool {
        self.roles.contains(&role.as_str().to_string())
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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserWithSessions {
    #[sqlx(try_from = "String")]
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub status: Option<String>,
    pub roles: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub active_sessions: i64,
    pub last_session_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, FromRow)]
pub(crate) struct UserSessionRow {
    #[sqlx(try_from = "String")]
    pub session_id: SessionId,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_type: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl From<UserSessionRow> for UserSession {
    fn from(row: UserSessionRow) -> Self {
        Self {
            session_id: row.session_id,
            user_id: row.user_id.map(UserId::new),
            ip_address: row.ip_address,
            user_agent: row.user_agent,
            device_type: row.device_type,
            started_at: row.started_at,
            last_activity_at: row.last_activity_at,
            ended_at: row.ended_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub id: String,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub display_name: Option<String>,
    pub status: Option<String>,
    pub email_verified: Option<bool>,
    pub roles: Vec<String>,
    pub is_bot: bool,
    pub is_scanner: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl From<User> for UserExport {
    fn from(user: User) -> Self {
        Self {
            id: user.id.to_string(),
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
