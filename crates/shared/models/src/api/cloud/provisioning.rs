//! Cloud provisioning lifecycle events emitted while a tenant is
//! being created and deployed.

use serde::{Deserialize, Serialize};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fly_app_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutEvent {
    pub checkout_session_id: String,
    pub tenant_id: String,
    pub tenant_name: String,
    pub event_type: ProvisioningEventType,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fly_app_name: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_url: Option<String>,
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
