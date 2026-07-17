//! Domain configuration validators for startup validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod agents;
mod ai;
mod content;
mod mcp;
mod rate_limits;
mod validation_config_provider;
mod web;

pub use agents::AgentConfigValidator;
pub use ai::AiConfigValidator;
pub use content::ContentConfigValidator;
pub use mcp::McpConfigValidator;
pub use rate_limits::RateLimitsConfigValidator;
pub use validation_config_provider::{ValidationConfigProvider, WebConfigRaw, WebMetadataRaw};
pub use web::WebConfigValidator;
