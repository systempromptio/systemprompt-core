//! systemprompt.io Cloud API client.
//!
//! - [`CloudApiClient`] is constructed in `client.rs`.
//! - Low-level HTTP verbs live in `methods.rs`.
//! - Top-level endpoints live in `endpoints.rs`; tenant-scoped endpoints in
//!   `tenant_api.rs`.
//! - SSE stream subscriptions live in `streams.rs`.

mod client;
mod endpoints;
mod methods;
mod streams;
mod tenant_api;
mod types;

pub use client::CloudApiClient;
pub use types::{
    CheckoutEvent, CheckoutResponse, DeployResponse, ListSecretsResponse, Plan, ProvisioningEvent,
    ProvisioningEventType, RegistryToken, RotateCredentialsResponse, StatusResponse,
    SubscriptionStatus, Tenant, TenantInfo, TenantSecrets, TenantStatus, UserInfo, UserMeResponse,
};
