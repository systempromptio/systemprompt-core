//! Vertex AI inputs for the setup wizard: a project id for workload identity,
//! or a service-account key file whose contents become the `vertex` secret.
//!
//! Both are checked before anything is written: a project id against Google's
//! naming rules, and a key file by parsing it as the gateway will, so a wrong
//! file is refused at setup rather than on the first model request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result, bail};
use systemprompt_security::credential::{CredentialKind, ProviderCredential};

use super::SecretsData;

pub fn validate_project(project: &str) -> Result<()> {
    let length_ok = (6..=30).contains(&project.len());
    let starts_with_letter = project.starts_with(|c: char| c.is_ascii_lowercase());
    let charset_ok = project
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !(length_ok && starts_with_letter && charset_ok) || project.ends_with('-') {
        bail!(
            "'{project}' is not a Google Cloud project id: 6-30 lowercase letters, digits or \
             hyphens, starting with a letter and not ending with a hyphen"
        );
    }
    Ok(())
}

pub fn read_key_file(path: &Path) -> Result<String> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read the service-account key {}", path.display()))?;
    let key = raw.trim().to_owned();
    let credential = ProviderCredential::parse(&key).with_context(|| {
        format!(
            "{} is not a usable Google service-account key",
            path.display()
        )
    })?;
    if credential.kind() != CredentialKind::GoogleServiceAccount {
        bail!(
            "{} is not a Google service-account key (expected a JSON key file with \
             \"type\": \"service_account\")",
            path.display()
        );
    }
    Ok(key)
}

pub fn apply(
    secrets: &mut SecretsData,
    project: Option<&str>,
    key_file: Option<&Path>,
) -> Result<()> {
    match (project, key_file) {
        (Some(_), Some(_)) => {
            bail!("pass either --vertex-project (workload identity) or --vertex-key-file, not both")
        },
        (Some(project), None) => {
            validate_project(project)?;
            secrets.vertex_project = Some(project.to_owned());
        },
        (None, Some(path)) => secrets.vertex = Some(read_key_file(path)?),
        (None, None) => {},
    }
    Ok(())
}
