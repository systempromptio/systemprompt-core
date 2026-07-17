//! Cloud API request and response types re-exported from `systemprompt_models`.
//!
//! Splits the wire types into a crate-private set used only by the API client
//! and a public set surfaced to callers (tenants, plans, deploy/provisioning
//! events, secrets, and subscription status).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(super) use systemprompt_models::api::cloud::{
    ActivityData, ActivityRequest, ApiError, ApiResponse, CheckoutRequest, CustomDomainResponse,
    ExternalDbAccessResponse, ListResponse, SetCustomDomainRequest, SetExternalDbAccessRequest,
    SetSecretsRequest,
};
pub use systemprompt_models::api::cloud::{
    CheckoutEvent, CheckoutResponse, DeployResponse, ListSecretsResponse, Plan, ProvisioningEvent,
    ProvisioningEventType, RegistryToken, RotateCredentialsResponse, StatusResponse,
    SubscriptionStatus, Tenant, TenantInfo, TenantSecrets, TenantStatus, UserInfo, UserMeResponse,
};
