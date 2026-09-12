//! Configured secret-pattern definitions and compilation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use regex::Regex;
use serde::Deserialize;
use systemprompt_identifiers::SecretPatternId;
use thiserror::Error;

/// One installation-owned plaintext-credential signature.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretPattern {
    pub id: SecretPatternId,
    pub name: String,
    pub regex: String,
    #[serde(default)]
    pub secret_capture: Option<String>,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub redact_whole_value: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretPatternError {
    #[error("secret_scan.patterns must be a sequence")]
    InvalidList,
    #[error("secret pattern {index} is invalid: {message}")]
    InvalidDefinition { index: usize, message: String },
    #[error("duplicate secret pattern id `{id}`")]
    DuplicateId { id: SecretPatternId },
    #[error("secret pattern `{id}` has invalid regex: {message}")]
    InvalidRegex {
        id: SecretPatternId,
        message: String,
    },
    #[error("secret pattern `{id}` regex can match an empty value")]
    EmptyMatch { id: SecretPatternId },
    #[error("secret pattern `{id}` names missing capture `{capture}`")]
    MissingCapture {
        id: SecretPatternId,
        capture: String,
    },
    #[error(
        "secret pattern `{id}` restricts a structured field and must set redact_whole_value: true"
    )]
    UnsafeFieldRecovery { id: SecretPatternId },
}

#[derive(Debug, Clone)]
pub(super) struct CompiledSecretPattern {
    pub definition: SecretPattern,
    pub regex: Regex,
}

pub(super) fn compile_patterns(
    node: Option<&serde_yaml::Value>,
) -> Result<Vec<CompiledSecretPattern>, SecretPatternError> {
    let Some(node) = node else {
        return Ok(Vec::new());
    };
    let sequence = node.as_sequence().ok_or(SecretPatternError::InvalidList)?;
    let mut ids = HashSet::with_capacity(sequence.len());
    let mut compiled = Vec::with_capacity(sequence.len());
    for (index, value) in sequence.iter().enumerate() {
        let pattern: SecretPattern = serde_yaml::from_value(value.clone()).map_err(|error| {
            SecretPatternError::InvalidDefinition {
                index,
                message: error.to_string(),
            }
        })?;
        if !ids.insert(pattern.id.clone()) {
            return Err(SecretPatternError::DuplicateId { id: pattern.id });
        }
        if pattern.name.trim().is_empty() {
            return Err(SecretPatternError::InvalidDefinition {
                index,
                message: "name must not be empty".to_owned(),
            });
        }
        if pattern
            .field
            .as_ref()
            .is_some_and(|field| field.trim().is_empty())
        {
            return Err(SecretPatternError::InvalidDefinition {
                index,
                message: "field must not be empty".to_owned(),
            });
        }
        if pattern.field.is_some() && !pattern.redact_whole_value {
            return Err(SecretPatternError::UnsafeFieldRecovery { id: pattern.id });
        }
        let regex =
            Regex::new(&pattern.regex).map_err(|error| SecretPatternError::InvalidRegex {
                id: pattern.id.clone(),
                message: error.to_string(),
            })?;
        if regex.is_match("") {
            return Err(SecretPatternError::EmptyMatch { id: pattern.id });
        }
        if let Some(capture) = &pattern.secret_capture
            && !regex.capture_names().flatten().any(|name| name == capture)
        {
            return Err(SecretPatternError::MissingCapture {
                id: pattern.id,
                capture: capture.clone(),
            });
        }
        compiled.push(CompiledSecretPattern {
            definition: pattern,
            regex,
        });
    }
    Ok(compiled)
}

pub(super) fn field_matches(path: &str, field: Option<&str>) -> bool {
    field.is_none_or(|field| {
        path.rsplit('.')
            .next()
            .is_some_and(|key| key.eq_ignore_ascii_case(field))
    })
}

pub(super) const HIGH_ENTROPY_PATTERN_ID: &str = "high-entropy-token";
pub(super) const HIGH_ENTROPY_PATTERN_NAME: &str = "High-entropy token (possible credential)";
