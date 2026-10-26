//! Higher-level config services built on top of the bootstrap layer.
//!
//! - [`schema_validation`] — `JsonSchema`-driven helpers for runtime config
//!   parsing.
//! - [`ProviderCatalogService`] — typed mutations of the four catalog
//!   registries
//!   (`services/ai/{credentials,providers,models,deployments}.yaml`).
//! - [`SecurityConfigService`] — typed mutations of the profile's security
//!   section.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod provider_catalog;
mod schema_validation;
mod security_config;

pub use provider_catalog::{
    CredentialSpec, EndpointSpec, ModelSpec, ProviderCatalogService, ProviderSpec, read_catalog,
    write_catalog,
};
pub use schema_validation::{
    ConfigValidationError, generate_schema, validate_config, validate_yaml_file, validate_yaml_str,
};
pub use security_config::{SecurityChange, SecurityConfigService, SecurityUpdate};
