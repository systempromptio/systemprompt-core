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

#[cfg(test)]
mod services;
