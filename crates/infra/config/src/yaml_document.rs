//! Untyped YAML document reads and writes for operator-authored config.
//!
//! [`read_yaml_file`] and [`write_yaml_file`] round-trip a file as a
//! `serde_yaml::Value`, so a command can change one field and write the file
//! back without dropping keys the tooling does not model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum YamlDocumentError {
    #[error("Failed to read file: {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse YAML from: {}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("Failed to serialize YAML")]
    Serialize(#[source] serde_yaml::Error),
    #[error("Failed to write file: {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub fn read_yaml_file(path: &Path) -> Result<serde_yaml::Value, YamlDocumentError> {
    // JSON: an untyped document preserves unknown keys in operator-authored
    // config so a single field can be mutated and written back losslessly.
    let content = std::fs::read_to_string(path).map_err(|source| YamlDocumentError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_yaml::from_str(&content).map_err(|source| YamlDocumentError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

pub fn write_yaml_file(path: &Path, content: &serde_yaml::Value) -> Result<(), YamlDocumentError> {
    // JSON: writes back the untyped document read by `read_yaml_file`, keeping
    // operator-authored keys this tooling does not model.
    let yaml_str = serde_yaml::to_string(content).map_err(YamlDocumentError::Serialize)?;
    std::fs::write(path, yaml_str).map_err(|source| YamlDocumentError::Write {
        path: path.to_path_buf(),
        source,
    })
}
