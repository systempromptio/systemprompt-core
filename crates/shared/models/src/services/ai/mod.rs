//! AI service configuration model.
//!
//! Plain serde data deserialized from profile config. Split into [`config`]
//! (the `AiConfig` aggregate, sampling, MCP, resilience, and history policy)
//! and [`model`] (provider definitions, per-model capabilities, limits, and
//! pricing). All types are re-exported here so consumers use
//! `crate::services::ai::*` regardless of the internal split.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod config;
pub mod model;

pub use config::{AiConfig, HistoryConfig, McpConfig, ResilienceSettings, SamplingConfig};
pub use model::{AiProviderConfig, ModelCapabilities, ModelDefinition, ModelLimits, ModelPricing};
