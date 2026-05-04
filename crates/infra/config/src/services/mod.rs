//! Higher-level config services built on top of the bootstrap layer.
//!
//! - [`ConfigManager`] — generate environment-specific deployment configs by
//!   merging `base.yaml` with `environments/<env>/*.yaml`.
//! - [`ConfigValidator`] — quality checks for the generated `.env`.
//! - [`ConfigWriter`] — on-disk writers with the right symlinks for the web
//!   frontend.
//! - [`schema_validation`] — `JsonSchema`-driven helpers used by `build.rs`
//!   scripts.

mod manager;
mod report;
pub mod schema_validation;
mod types;
mod validator;
mod writer;

pub use manager::ConfigManager;
pub use report::ValidationReport;
pub use schema_validation::{
    ConfigValidationError, build_validate_configs, generate_schema, validate_config,
    validate_yaml_file, validate_yaml_str,
};
pub use types::{DeployEnvironment, DeploymentConfig, EnvironmentConfig};
pub use validator::ConfigValidator;
