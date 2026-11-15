//! Untyped YAML document reads and writes for operator-authored config.
//!
//! [`read_yaml_file`] and [`write_yaml_file`] round-trip a file as a
//! `serde_yaml::Value`, so a command can change one field and write the file
//! back without dropping keys the tooling does not model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result};

pub fn read_yaml_file(path: &Path) -> Result<serde_yaml::Value> {
    // JSON: an untyped document preserves unknown keys in operator-authored
    // config so a single field can be mutated and written back losslessly.
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML from: {}", path.display()))
}

pub fn write_yaml_file(path: &Path, content: &serde_yaml::Value) -> Result<()> {
    // JSON: writes back the untyped document read by `read_yaml_file`, keeping
    // operator-authored keys this tooling does not model.
    let yaml_str = serde_yaml::to_string(content).with_context(|| "Failed to serialize YAML")?;
    std::fs::write(path, yaml_str)
        .with_context(|| format!("Failed to write file: {}", path.display()))
}
