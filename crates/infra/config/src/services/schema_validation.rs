//! `JsonSchema`-driven validation helpers used by `build.rs` scripts
//! and the `systemprompt cloud config` command.

use std::path::Path;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use thiserror::Error;

/// Errors emitted by the schema-validation helpers.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigValidationError {
    /// Failed to read the config file from disk.
    #[error("Failed to read config file: {0}")]
    Read(#[from] std::io::Error),

    /// Failed to parse YAML into `serde_yaml::Value`.
    #[error("Failed to parse YAML config: {0}")]
    Parse(#[from] serde_yaml::Error),

    /// Concrete schema validation failed.
    #[error("Schema validation failed: {0}")]
    Schema(String),
}

/// Read `yaml_path` from disk and deserialize into `T`.
///
/// # Errors
///
/// Returns [`ConfigValidationError::Read`] on I/O failure and
/// [`ConfigValidationError::Parse`] when YAML deserialization fails.
pub fn validate_config<T: DeserializeOwned + JsonSchema>(
    yaml_path: impl AsRef<Path>,
) -> Result<T, ConfigValidationError> {
    let content = std::fs::read_to_string(yaml_path)?;
    let config: T = serde_yaml::from_str(&content)?;
    Ok(config)
}

/// Generate the JSON Schema for `T`.
///
/// # Errors
///
/// Returns the underlying [`serde_json::Error`] if the generated
/// schema cannot be converted to a [`serde_json::Value`].
pub fn generate_schema<T: JsonSchema>() -> Result<serde_json::Value, serde_json::Error> {
    let schema = schemars::schema_for!(T);
    serde_json::to_value(schema)
}

/// Deserialize `yaml` into `T`.
///
/// # Errors
///
/// Returns [`ConfigValidationError::Parse`] on YAML deserialization
/// failure.
pub fn validate_yaml_str<T: DeserializeOwned>(yaml: &str) -> Result<T, ConfigValidationError> {
    let config: T = serde_yaml::from_str(yaml)?;
    Ok(config)
}

/// Function pointer that validates the contents of a config file.
pub type ConfigValidatorFn = fn(&str) -> Result<(), String>;

/// Run validators against `(path, validator)` pairs.
///
/// Emits `cargo:rerun-if-changed` directives for every path. On any
/// failure the function prints a diagnostic and exits with status 1,
/// matching the `build.rs` calling convention.
///
/// This is intentionally invoked only from `build.rs` scripts; the
/// `clippy::print_*` and `clippy::exit` lints are scoped to this
/// function via local `#[expect]` attributes.
pub fn build_validate_configs(configs: &[(&str, ConfigValidatorFn)]) {
    for (path, validator) in configs {
        emit_rerun(path);
        if let Err(e) = validator(path) {
            emit_failure(path, &e);
        }
    }
}

#[expect(
    clippy::print_stdout,
    reason = "build.rs requires writing cargo:rerun-if-changed directives to stdout"
)]
fn emit_rerun(path: &str) {
    println!("cargo:rerun-if-changed={path}");
}

#[expect(
    clippy::print_stderr,
    clippy::exit,
    reason = "build.rs failure path must write a diagnostic to stderr and abort with status 1"
)]
fn emit_failure(path: &str, message: &str) -> ! {
    eprintln!("Config validation failed for {path}: {message}");
    std::process::exit(1);
}

/// Read and parse `path` as untyped YAML.
///
/// # Errors
///
/// Returns [`ConfigValidationError::Read`] on I/O failure or
/// [`ConfigValidationError::Parse`] on malformed YAML.
pub fn validate_yaml_file(
    path: impl AsRef<Path>,
) -> Result<serde_yaml::Value, ConfigValidationError> {
    let content = std::fs::read_to_string(path)?;
    let value: serde_yaml::Value = serde_yaml::from_str(&content)?;
    Ok(value)
}
