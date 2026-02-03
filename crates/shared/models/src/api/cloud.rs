//! Cloud Management API types shared between CLI and API server.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudApiResponse<T> {
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudApiError {
    pub error: CloudApiErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudApiErrorDetail {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudUserInfo {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCustomerInfo {
    pub id: String,
    #[serde(default)]
    pub status: Option<String>,
}

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
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub memory_mb: i32,
    pub volume_gb: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudPlan {
    pub id: String,
    pub name: String,
    pub paddle_price_id: String,
    #[serde(default)]
    pub memory_mb_default: i32,
    #[serde(default)]
    pub volume_gb: i32,
    #[serde(default)]
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
    #[serde(default)]
    pub subscription_id: Option<String>,
    #[serde(default)]
    pub subscription_status: Option<SubscriptionStatus>,
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub status: Option<CloudTenantStatus>,
    #[serde(default)]
    pub plan: Option<CloudPlanInfo>,
    #[serde(default)]
    pub external_db_access: bool,
    pub database_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudTenant {
    pub id: String,
    pub name: String,
    pub fly_app_name: Option<String>,
    pub fly_hostname: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudTenantStatusResponse {
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub app_url: Option<String>,
    #[serde(default)]
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
    pub tenant_id: String,
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
    pub status: String,
    pub sync_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMeResponse {
    pub user: CloudUserInfo,
    #[serde(default)]
    pub customer: Option<CloudCustomerInfo>,
    #[serde(default)]
    pub tenants: Vec<CloudTenantInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudListResponse<T> {
    pub data: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryToken {
    pub registry: String,
    pub username: String,
    pub token: String,
    pub repository: String,
    pub tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResponse {
    pub status: String,
    pub app_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudStatusResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSecretsRequest {
    pub secrets: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutRequest {
    pub price_id: String,
    pub region: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutResponse {
    pub checkout_url: String,
    pub transaction_id: String,
    pub checkout_session_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningEventType {
    SubscriptionCreated,
    TenantCreated,
    DatabaseCreated,
    SecretsStored,
    VmProvisioningStarted,
    VmProvisioningProgress,
    VmProvisioned,
    SecretsConfigured,
    InfrastructureReady,
    TenantReady,
    ProvisioningFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisioningEvent {
    pub tenant_id: String,
    pub event_type: ProvisioningEventType,
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub app_url: Option<String>,
    #[serde(default)]
    pub fly_app_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutEvent {
    pub checkout_session_id: String,
    pub tenant_id: String,
    pub tenant_name: String,
    pub event_type: ProvisioningEventType,
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub app_url: Option<String>,
    #[serde(default)]
    pub fly_app_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CloudLogEntry {
    pub timestamp: String,
    pub message: String,
    #[serde(default)]
    pub level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudLogsResponse {
    pub logs: Vec<CloudLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListSecretsResponse {
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetCustomDomainRequest {
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DnsInstructions {
    pub record_type: String,
    pub host: String,
    pub value: String,
    pub ttl: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomDomainResponse {
    pub domain: String,
    pub status: String,
    pub verified: bool,
    pub dns_target: String,
    pub dns_instructions: DnsInstructions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRequest {
    pub event: String,
    pub timestamp: String,
    pub data: ActivityData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityData {
    pub user_id: String,
}

pub type ApiResponse<T> = CloudApiResponse<T>;
pub type ApiError = CloudApiError;
pub type ApiErrorDetail = CloudApiErrorDetail;
pub type UserInfo = CloudUserInfo;
pub type CustomerInfo = CloudCustomerInfo;
pub type PlanInfo = CloudPlanInfo;
pub type Plan = CloudPlan;
pub type TenantInfo = CloudTenantInfo;
pub type Tenant = CloudTenant;
pub type TenantStatus = CloudTenantStatusResponse;
pub type TenantSecrets = CloudTenantSecrets;
pub type ListResponse<T> = CloudListResponse<T>;
pub type StatusResponse = CloudStatusResponse;
pub type LogEntry = CloudLogEntry;
pub type LogsResponse = CloudLogsResponse;
