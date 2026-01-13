use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{SessionId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserListOutput {
    pub users: Vec<UserSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserSummary {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub status: Option<String>,
    pub roles: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserDetailOutput {
    pub id: UserId,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sessions: Option<Vec<SessionSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<UserActivityOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_type: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserActivityOutput {
    pub user_id: UserId,
    pub last_active: Option<DateTime<Utc>>,
    pub session_count: i64,
    pub task_count: i64,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserCountOutput {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserCreatedOutput {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserUpdatedOutput {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserDeletedOutput {
    pub id: UserId,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoleAssignOutput {
    pub id: UserId,
    pub name: String,
    pub roles: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionListOutput {
    pub sessions: Vec<SessionSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionCleanupOutput {
    pub cleaned: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BanListOutput {
    pub bans: Vec<BanSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BanSummary {
    pub ip_address: String,
    pub reason: String,
    pub banned_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_permanent: bool,
    pub ban_count: i32,
    pub ban_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BanAddOutput {
    pub ip_address: String,
    pub reason: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_permanent: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BanRemoveOutput {
    pub ip_address: String,
    pub removed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BanCheckOutput {
    pub ip_address: String,
    pub is_banned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ban_info: Option<BanSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BanCleanupOutput {
    pub cleaned: u64,
    pub message: String,
}
