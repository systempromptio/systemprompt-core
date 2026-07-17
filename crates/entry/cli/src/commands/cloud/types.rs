//! Serializable output DTOs for the `cloud` command group.
//!
//! Each type is the structured payload a cloud subcommand returns (status,
//! login, tenant list/detail/create, secrets, deploy, sync, and related), used
//! for both human rendering and `--json` output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use systemprompt_models::profile::ProfileInfo;

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
pub struct RotateCredentialsOutput {
    #[serde(rename = "tenant_id")]
    pub tenant: String,
    pub status: String,
    pub internal_database_url: String,
    pub external_database_url: String,
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
pub struct DockerfileOutput {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CancelSubscriptionOutput {
    #[serde(rename = "tenant_id")]
    pub tenant: String,
    pub tenant_name: String,
    pub message: String,
}
