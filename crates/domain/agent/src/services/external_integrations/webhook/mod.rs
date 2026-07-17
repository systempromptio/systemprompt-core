//! Webhook delivery and inbound-signature verification for external
//! integrations.
//!
//! Re-exports the [`WebhookService`] facade along with its configuration,
//! retry-policy, and delivery/test result types from the `service` submodule.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod service;

pub use service::{
    RetryPolicy, WebhookConfig, WebhookDeliveryResult, WebhookService, WebhookTestResult,
};
