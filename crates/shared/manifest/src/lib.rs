//! Services manifest, profile and boot-time configuration for systemprompt.io.
//!
//! `systemprompt-manifest` holds the shapes that are read from disk at boot
//! and validated before the runtime starts: the services manifest (agents,
//! plugins, hooks, MCP servers, skills, marketplaces, the provider registry
//! and the gateway policy), the on-disk profile, the secrets document, the
//! global [`Config`] assembled from them, and the startup validators. Runtime
//! request and response models stay in `systemprompt-models`; the provider
//! wire codecs live in `systemprompt-wire`.
//!
//! # Module map
//!
//! - [`services`] — the services manifest and its `validate()` passes.
//! - [`validators`] — startup configuration validation passes.
//! - [`profile`] — on-disk profile and bootstrap configuration.
//! - [`config`] — global [`config::Config`] singleton and validation helpers.
//! - [`secrets`] — secrets document model.
//! - [`paths`] — path-resolution contract and well-known directory constants.
//! - [`env`](mod@env) — `${VAR}` interpolation over environment variables.
//! - [`bridge_profile`] — the builder of the `/v1/bridge/profile` payload from
//!   the provider registry.
//!
//! Public functions return `thiserror`-derived enums from
//! `systemprompt_models::errors`; `anyhow::Error` is never used in a public
//! signature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod bridge_profile;
pub mod config;
pub mod env;
pub mod paths;
pub mod profile;
pub mod secrets;
pub mod services;
pub mod validators;

pub use config::{Config, PathNotConfiguredError};
pub use env::{contains_placeholder, interpolate, none_if_blank, read_env_optional};
pub use paths::PathResolution;
pub use profile::{
    CloudConfig, CloudValidationMode, ContentNegotiationConfig,
    DatabaseConfig as ProfileDatabaseConfig, Environment, ExtensionsConfig, LogLevel, OutputFormat,
    PathsConfig, Profile, ProfileStyle, ProfileType, RateLimitsConfig, RuntimeConfig,
    SecurityConfig, SecurityHeadersConfig, ServerConfig, SiteConfig,
};
pub use secrets::Secrets;
pub use services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, AiConfig, CapabilitiesConfig,
    DiskHookConfig, DiskSkillConfig, HOOK_CONFIG_FILENAME, MarketplaceConfig,
    OAuthConfig as AgentOAuthConfig, PluginConfig, PluginConfigFile, PluginVariableDef,
    RuntimeStatus, SKILL_CONFIG_FILENAME, SchedulerConfig, ServiceModule, ServiceStatus,
    ServiceType, ServicesConfig, SkillsConfig, SystemAdmin, WebConfig, split_frontmatter,
    strip_frontmatter,
};
