//! Cloud tenant, plan, and subscription DTOs exchanged between the CLI and the cloud API.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    Trialing,
    PastDue,
    Paused,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudPlanInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub memory_mb: i32,
    pub volume_gb: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudPlan {
    pub id: String,
    pub name: String,
    pub paddle_price_id: systemprompt_identifiers::PriceId,
    #[serde(default)]
    pub memory_mb_default: i32,
    #[serde(default)]
    pub volume_gb: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tenants: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudTenantStatus {
    Pending,
    Active,
    Suspended,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudTenantInfo {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_status: Option<SubscriptionStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<CloudTenantStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<CloudPlanInfo>,
    #[serde(default)]
    pub external_db_access: bool,
    pub database_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudTenant {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fly_app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fly_hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudTenantStatusResponse {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudTenantSecrets {
    pub jwt_secret: String,
    pub database_url: String,
    pub internal_database_url: String,
    pub app_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini_api_key: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SetExternalDbAccessRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalDbAccessResponse {
    pub tenant_id: systemprompt_identifiers::TenantId,
    pub external_db_access: bool,
    pub database_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateCredentialsResponse {
    pub status: String,
    pub message: String,
    pub internal_database_url: String,
    pub external_database_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateSyncTokenResponse {
    pub sync_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudEnterpriseLicenseInfo {
    pub id: String,
    pub name: String,
    pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<CloudPlanInfo>,
}
