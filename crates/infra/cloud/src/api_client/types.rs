use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct CustomerInfo {
    pub id: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    Trialing,
    PastDue,
    Paused,
    Canceled,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PlanInfo {
    pub name: String,
    pub memory_mb: i32,
    pub volume_gb: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TenantInfo {
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
    pub plan: Option<PlanInfo>,
}

#[derive(Deserialize, Debug)]
pub struct UserMeResponse {
    pub user: UserInfo,
    #[serde(default)]
    pub customer: Option<CustomerInfo>,
    #[serde(default)]
    pub tenants: Vec<TenantInfo>,
}

#[derive(Debug, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub fly_app_name: Option<String>,
    pub fly_hostname: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TenantStatus {
    pub status: String,
    pub message: Option<String>,
    pub app_url: Option<String>,
    #[serde(default)]
    pub secrets_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSecrets {
    pub jwt_secret: String,
    pub database_url: String,
    pub app_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gemini_api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegistryToken {
    pub registry: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct DeployResponse {
    pub status: String,
    pub app_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Plan {
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

#[derive(Debug, Serialize)]
pub struct CheckoutRequest {
    pub price_id: String,
    pub region: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct CheckoutResponse {
    pub checkout_url: String,
    pub transaction_id: String,
    pub checkout_session_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct ProvisioningEvent {
    pub tenant_id: String,
    pub event_type: ProvisioningEventType,
    pub status: String,
    pub message: Option<String>,
    pub app_url: Option<String>,
    #[serde(default)]
    pub fly_app_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckoutEvent {
    pub checkout_session_id: String,
    pub tenant_id: String,
    pub tenant_name: String,
    pub event_type: ProvisioningEventType,
    pub status: String,
    pub message: Option<String>,
    pub app_url: Option<String>,
    #[serde(default)]
    pub fly_app_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    #[serde(default)]
    pub level: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogsResponse {
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatusResponse {
    pub status: String,
}
