//! systemprompt.io Cloud API client.
//!
//! - [`CloudApiClient`] is constructed in `client.rs`.
//! - Low-level HTTP verbs live in `methods.rs`.
//! - Top-level endpoints live in `endpoints.rs`; tenant-scoped endpoints in
//!   `tenant_api.rs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod client;
mod endpoints;
mod methods;
mod tenant_api;
mod types;

pub use client::CloudApiClient;
pub use types::{
    DeployResponse, RegistryToken, RotateCredentialsResponse, StatusResponse, SubscriptionStatus,
    Tenant, TenantInfo, TenantSecrets, TenantStatus, UserInfo, UserMeResponse,
};
