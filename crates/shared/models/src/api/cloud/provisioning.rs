//! Cloud provisioning lifecycle events emitted while a tenant is
//! being created and deployed.

use serde::{Deserialize, Serialize};

/// Discrete stages of the tenant-provisioning pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningEventType {
    /// The Stripe / Paddle subscription was created.
    SubscriptionCreated,
    /// The tenant record was created in the cloud control plane.
    TenantCreated,
    /// The tenant database was provisioned.
    DatabaseCreated,
    /// Initial secrets were stored.
    SecretsStored,
    /// VM provisioning kicked off.
    VmProvisioningStarted,
    /// VM provisioning is progressing.
    VmProvisioningProgress,
    /// VM provisioning finished.
    VmProvisioned,
    /// Final secrets were configured on the VM.
    SecretsConfigured,
    /// Infrastructure is ready for use.
    InfrastructureReady,
    /// Tenant is ready end-to-end.
    TenantReady,
    /// Provisioning failed and the pipeline was aborted.
    ProvisioningFailed,
}

/// Single provisioning lifecycle event for an existing tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisioningEvent {
    /// Tenant the event refers to (external vendor identifier).
    pub tenant_id: String,
    /// Stage that produced this event.
    pub event_type: ProvisioningEventType,
    /// Coarse status string (e.g. `"in_progress"`, `"completed"`).
    pub status: String,
    /// Optional human-readable message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// URL the deployed app is reachable at, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_url: Option<String>,
    /// Underlying Fly app name, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fly_app_name: Option<String>,
}

/// Provisioning event emitted by the checkout flow before a tenant id
/// has been issued.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutEvent {
    /// Checkout session this event belongs to.
    pub checkout_session_id: String,
    /// Tenant id assigned during checkout (external vendor identifier).
    pub tenant_id: String,
    /// Tenant display name.
    pub tenant_name: String,
    /// Stage that produced this event.
    pub event_type: ProvisioningEventType,
    /// Coarse status string.
    pub status: String,
    /// Optional human-readable message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// URL the deployed app is reachable at, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_url: Option<String>,
    /// Underlying Fly app name, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fly_app_name: Option<String>,
}

/// Checkout-session creation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutRequest {
    /// Plan price the customer selected.
    pub price_id: String,
    /// Region the tenant should be deployed in.
    pub region: String,
    /// Optional post-checkout redirect URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
}

/// Response returned by the checkout endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutResponse {
    /// URL the customer should be sent to.
    pub checkout_url: String,
    /// Vendor transaction id.
    pub transaction_id: String,
    /// Internal checkout session id.
    pub checkout_session_id: String,
}

/// Deploy command response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResponse {
    /// Coarse status string.
    pub status: String,
    /// URL the deployed app is reachable at, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_url: Option<String>,
}

/// User activity event submitted to the cloud activity feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRequest {
    /// Event name.
    pub event: String,
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Event-specific payload.
    pub data: ActivityData,
}

/// Inner activity payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityData {
    /// User the event is attributed to (external vendor identifier).
    pub user_id: String,
}
