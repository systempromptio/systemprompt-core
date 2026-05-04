//! # systemprompt-config
//!
//! Profile-based configuration for systemprompt.io. This crate is the
//! bootstrap layer: it loads the active profile YAML, the matching
//! secrets document, and installs both into process-wide singletons
//! before any other layer (database, runtime, agent) starts.
//!
//! ## Public surface
//!
//! - [`BootstrapSequence`] / [`presets`] — type-state-driven startup helper
//!   that enforces *profile before secrets* at compile time.
//! - [`ProfileBootstrap`] / [`SecretsBootstrap`] — process-wide cells for the
//!   active profile and secrets document.
//! - [`init_config`] / [`build_from_profile`] — build a runtime
//!   [`systemprompt_models::Config`] from the active profile.
//! - [`ConfigManager`], [`ConfigValidator`] — utilities used by the
//!   `systemprompt cloud config` deployment pipeline.
//! - [`SkillConfigValidator`] — `DomainConfig` implementation that walks
//!   `skills/` and reports missing or malformed manifests.
//!
//! ## Errors
//!
//! All public APIs return [`ConfigResult<T>`] (i.e.
//! `Result<T, ConfigError>`). [`ConfigError`] composes the bootstrap,
//! profile, secrets, schema-validation, and lower-level
//! `serde`/`std::io` errors via `#[from]` so callers can use `?`
//! transparently.
//!
//! ## Feature flags
//!
//! This crate has no Cargo features — every dependency is required at
//! compile time. The `[package.metadata.docs.rs]` section in
//! `Cargo.toml` enables `all-features = true` for parity with the
//! rest of the workspace.

pub mod bootstrap;
pub(crate) mod config_loader;
pub mod error;
pub mod profile_gateway;
pub(crate) mod profile_loader;
pub(crate) mod services;
pub(crate) mod skill_validator;

pub use bootstrap::{
    BootstrapComplete, BootstrapSequence, BootstrapState, JWT_SECRET_MIN_LENGTH,
    MANIFEST_SIGNING_SEED_BYTES, ProfileBootstrap, ProfileBootstrapError, ProfileInitialized,
    SecretsBootstrap, SecretsBootstrapError, SecretsInitialized, Uninitialized,
    build_loaded_secrets_message, decode_seed, generate_seed, load_secrets_from_path,
    log_secrets_issue, log_secrets_skip, log_secrets_warn, persist_seed, presets,
};
pub use config_loader::{
    build_from_profile, init_config, init_config_from_profile, try_init_config,
    validate_database_config,
};
pub use error::{ConfigError, ConfigResult};
pub use profile_loader::load_profile_with_catalog;
pub use services::{
    ConfigManager, ConfigValidationError, ConfigValidator, DeployEnvironment, DeploymentConfig,
    EnvironmentConfig, ValidationReport, build_validate_configs, generate_schema, validate_config,
    validate_yaml_file, validate_yaml_str,
};
pub use skill_validator::SkillConfigValidator;
