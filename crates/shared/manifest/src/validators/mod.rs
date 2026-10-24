//! Domain configuration validators for startup validation.
//!
//! Holds the [`DomainConfig`] trait each domain registers, the
//! [`DomainConfigRegistry`] the runtime walks, the [`ValidationReport`] types
//! they produce, and the shared-config validators.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod agents;
mod ai;
mod content;
pub mod domain_config;
mod mcp;
mod rate_limits;
mod validation_config_provider;
pub mod validation_report;
mod web;

pub use agents::AgentConfigValidator;
pub use ai::AiConfigValidator;
pub use content::ContentConfigValidator;
pub use domain_config::{DomainConfig, DomainConfigError, DomainConfigRegistry};
pub use mcp::McpConfigValidator;
pub use rate_limits::RateLimitsConfigValidator;
pub use validation_config_provider::{ValidationConfigProvider, WebConfigRaw, WebMetadataRaw};
pub use validation_report::{
    StartupValidationError, StartupValidationReport, ValidationIssue, ValidationReport,
    ValidationWarning,
};
pub use web::WebConfigValidator;
