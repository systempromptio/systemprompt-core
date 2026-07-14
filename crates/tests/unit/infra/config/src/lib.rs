//! Unit tests for systemprompt-core-config crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/infra/config/src/services/types.rs`
//! - Test: `crates/tests/unit/infra/config/src/services/types.rs`
//!
//! Tests cover:
//! - DeployEnvironment parsing and conversion
//! - DeploymentConfig construction and serialization
//! - EnvironmentConfig creation and validation
//! - ConfigValidator validation rules
//! - ValidationReport error and warning tracking
//! - Schema validation functions
//! - ProviderCatalogService registry mutations
//! - SecurityConfigService security-section mutations

#[cfg(test)]
mod fixture;

#[cfg(test)]
mod bootstrap_profile;

#[cfg(test)]
mod bootstrap_secrets;

#[cfg(test)]
mod bootstrap_secrets_env;

#[cfg(test)]
mod config_loader_build;

#[cfg(test)]
mod services;

#[cfg(test)]
mod config_loader;

#[cfg(test)]
mod manifest;

#[cfg(test)]
mod path_validation;

#[cfg(test)]
mod profile_gateway;

#[cfg(test)]
mod error_display;

#[cfg(test)]
mod secrets_io;

#[cfg(test)]
mod secrets_logging;

#[cfg(test)]
mod skill_validator;
