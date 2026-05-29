//! Outbound integrations bridging agents to external systems.
//!
//! Currently this surface is the [`webhook`] service for signed webhook
//! delivery and verification; the shared integration error, result, and
//! request/response models are re-exported here for consumers.

pub mod webhook;

pub use crate::models::external_integrations::{
    IntegrationError, IntegrationResult, RegisteredMcpServer, ToolExecutionResult, WebhookEndpoint,
    WebhookRequest, WebhookResponse,
};

pub use webhook::{RetryPolicy, WebhookConfig, WebhookDeliveryResult, WebhookService};
