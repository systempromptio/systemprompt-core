//! Higher-level config services built on top of the bootstrap layer.
//!
//! - [`ConfigService`] — generate environment-specific deployment configs by
//!   merging `base.yaml` with `environments/<env>/*.yaml`.
//! - [`ConfigValidator`] — quality checks for the generated `.env`.
//! - [`ConfigWriter`] — on-disk writers with the right symlinks for the web
//!   frontend.
//! - [`schema_validation`] — `JsonSchema`-driven helpers for runtime config
//!   parsing.

mod report;
mod schema_validation;
mod service;
mod types;
mod validator;
mod writer;

pub use report::ValidationReport;
pub use schema_validation::{
    ConfigValidationError, generate_schema, validate_config, validate_yaml_file, validate_yaml_str,
};
pub use service::ConfigService;
pub use types::{DeployEnvironment, DeploymentConfig, EnvironmentConfig};
pub use validator::ConfigValidator;
