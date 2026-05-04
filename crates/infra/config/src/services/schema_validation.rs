//! `JsonSchema`-driven validation helpers used by `build.rs` scripts
//! and the `systemprompt cloud config` command.

use std::path::Path;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigValidationError {
    #[error("Failed to read config file: {0}")]
    Read(#[from] std::io::Error),

    #[error("Failed to parse YAML config: {0}")]
    Parse(#[from] serde_yaml::Error),

    #[error("Schema validation failed: {0}")]
    Schema(String),
}

pub fn validate_config<T: DeserializeOwned + JsonSchema>(
    yaml_path: impl AsRef<Path>,
) -> Result<T, ConfigValidationError> {
    let content = std::fs::read_to_string(yaml_path)?;
    let config: T = serde_yaml::from_str(&content)?;
    Ok(config)
}

pub fn generate_schema<T: JsonSchema>() -> Result<serde_json::Value, serde_json::Error> {
    let schema = schemars::schema_for!(T);
    serde_json::to_value(schema)
}

pub fn validate_yaml_str<T: DeserializeOwned>(yaml: &str) -> Result<T, ConfigValidationError> {
    let config: T = serde_yaml::from_str(yaml)?;
    Ok(config)
}

pub type ConfigValidatorFn = fn(&str) -> Result<(), String>;

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

pub fn validate_yaml_file(
    path: impl AsRef<Path>,
) -> Result<serde_yaml::Value, ConfigValidationError> {
    let content = std::fs::read_to_string(path)?;
    let value: serde_yaml::Value = serde_yaml::from_str(&content)?;
    Ok(value)
}
