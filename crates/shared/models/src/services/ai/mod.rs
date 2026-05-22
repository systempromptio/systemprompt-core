//! AI service configuration model.
//!
//! Plain serde data deserialized from profile config. Split into [`config`]
//! (the `AiConfig` aggregate, sampling, MCP, resilience, and history policy)
//! and [`model`] (provider definitions, per-model capabilities, limits, and
//! pricing). All types are re-exported here so consumers use
//! `crate::services::ai::*` regardless of the internal split.

pub mod config;
pub mod model;

pub use config::{AiConfig, HistoryConfig, McpConfig, ResilienceSettings, SamplingConfig};
pub use model::{
    AiProviderConfig, ModelCapabilities, ModelDefinition, ModelLimits, ModelPricing,
    ToolModelConfig, ToolModelSettings,
};
