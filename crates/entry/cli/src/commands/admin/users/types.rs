use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use systemprompt_identifiers::{SessionId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserListOutput {
    pub users: Vec<UserSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub status: Option<String>,
    pub roles: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_type: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActivityOutput {
    pub user_id: UserId,
    pub last_active: Option<DateTime<Utc>>,
    pub session_count: i64,
    pub task_count: i64,
    pub message_count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UserCountOutput {
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCreatedOutput {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdatedOutput {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDeletedOutput {
    pub id: UserId,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignOutput {
    pub id: UserId,
    pub name: String,
    pub roles: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListOutput {
    pub sessions: Vec<SessionSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCleanupOutput {
    pub cleaned: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndOutput {
    pub ended: Vec<String>,
    pub count: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanListOutput {
    pub bans: Vec<BanSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanSummary {
    pub ip_address: String,
    pub reason: String,
    pub banned_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_permanent: bool,
    pub ban_count: i32,
    pub ban_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanAddOutput {
    pub ip_address: String,
    pub reason: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_permanent: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanRemoveOutput {
    pub ip_address: String,
    pub removed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanCheckOutput {
    pub ip_address: String,
    pub is_banned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ban_info: Option<BanSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanCleanupOutput {
    pub cleaned: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCountBreakdownOutput {
    pub total: i64,
    pub by_status: HashMap<String, i64>,
    pub by_role: HashMap<String, i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UserStatsOutput {
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
pub struct UserExportOutput {
    pub users: Vec<UserExportItem>,
    pub total: usize,
    pub exported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserExportItem {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkDeleteOutput {
    pub deleted: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkUpdateOutput {
    pub updated: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMergeOutput {
    pub source_id: UserId,
    pub target_id: UserId,
    pub sessions_transferred: u64,
    pub tasks_transferred: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebauthnSetupTokenOutput {
    pub user_email: String,
    pub token: String,
    pub registration_url: String,
    pub expires_minutes: u32,
}
