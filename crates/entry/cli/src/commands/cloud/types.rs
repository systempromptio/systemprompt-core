use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProfileInfo {
    pub name: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CredentialsInfo {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,
    pub token_expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TenantStatusInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub configured_in_profile: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudStatusOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileInfo>,
    pub credentials: CredentialsInfo,
    pub tenants: Vec<TenantStatusInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TenantPlanInfo {
    pub name: String,
    pub memory_mb: i32,
    pub volume_gb: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoginTenantInfo {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<TenantPlanInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoginUserInfo {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoginCustomerInfo {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoginOutput {
    pub user: LoginUserInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<LoginCustomerInfo>,
    pub tenants: Vec<LoginTenantInfo>,
    pub credentials_path: String,
    pub tenants_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WhoamiOutput {
    pub user_email: String,
    pub api_url: String,
    pub token_status: String,
    pub authenticated_at: DateTime<Utc>,
    pub local_profiles: usize,
    pub cloud_tenants: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogoutOutput {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TenantSummary {
    pub id: String,
    pub name: String,
    pub tenant_type: String,
    pub has_database: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TenantListOutput {
    pub tenants: Vec<TenantSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TenantDetailOutput {
    pub id: String,
    pub name: String,
    pub tenant_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    pub has_database: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TenantCreateOutput {
    pub id: String,
    pub name: String,
    pub tenant_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RotateCredentialsOutput {
    pub tenant_id: String,
    pub status: String,
    pub internal_database_url: String,
    pub external_database_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RotateSyncTokenOutput {
    pub tenant_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProfileSummary {
    pub name: String,
    pub has_secrets: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProfileListOutput {
    pub profiles: Vec<ProfileSummary>,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeployOutput {
    pub tenant_name: String,
    pub image: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub secrets_synced: usize,
    pub cloud_credentials_synced: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SyncOutput {
    pub direction: String,
    pub dry_run: bool,
    pub operations: Vec<SyncOperationOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SyncOperationOutput {
    pub operation: String,
    pub success: bool,
    pub items_synced: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdminUserSyncResultOutput {
    pub profile: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdminUserSyncOutput {
    pub cloud_user_email: String,
    pub results: Vec<AdminUserSyncResultOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillsSyncOutput {
    pub direction: String,
    pub dry_run: bool,
    pub synced: usize,
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecretsOutput {
    pub operation: String,
    pub keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RestartOutput {
    pub tenant_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DomainOutput {
    pub tenant_name: String,
    pub domain: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DockerfileOutput {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InitOutput {
    pub message: String,
    pub created_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CancelSubscriptionOutput {
    pub tenant_id: String,
    pub tenant_name: String,
    pub message: String,
}
