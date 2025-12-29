//! Domain configuration validators for startup validation.
//!
//! Each validator implements the `DomainConfig` trait to participate
//! in the startup validation pipeline.

mod agents;
mod ai;
mod content;
mod mcp;
mod validation_config_provider;
mod web;

pub use agents::AgentConfigValidator;
pub use ai::AiConfigValidator;
pub use content::ContentConfigValidator;
pub use mcp::McpConfigValidator;
pub use validation_config_provider::{ValidationConfigProvider, WebConfigRaw, WebMetadataRaw};
pub use web::WebConfigValidator;
