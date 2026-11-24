//! Every admitted member's declared configuration sections, stamped with
//! their owners in composition order.
//!
//! The kernel builds one [`AdmittedConfigs`] from the admitted composition
//! and seeds it into state, open: a declaration is a section prefix, its
//! schema and its validator, never a value, so a member's command (`cloud
//! doctor`'s extension-config check) validates the services tree against it
//! without naming the admitted composition.
//!
//! [`AdmittedConfigs::validate`] is the one validation pass over the services
//! tree: each declared section is read from `config/<prefix>.yaml` (an absent
//! file validates as an empty object) and checked by its declaration. The
//! serve boot and `cloud doctor` both run it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_identifiers::ExtensionId;

use crate::admitted::BuiltState;
use crate::contribute::ConfigDecl;
use crate::error::ExtensionConfigError;

/// The owner-stamped configuration declarations of an admitted composition.
#[derive(Debug, Clone, Default)]
pub struct AdmittedConfigs {
    decls: Vec<(ExtensionId, ConfigDecl)>,
}

impl AdmittedConfigs {
    #[must_use]
    pub fn of(built: &BuiltState) -> Self {
        let decls = built
            .members()
            .flat_map(|member| {
                member
                    .sink
                    .configs()
                    .iter()
                    .map(|decl| (member.owner.clone(), *decl))
            })
            .collect();
        Self { decls }
    }

    #[must_use]
    pub fn decls(&self) -> &[(ExtensionId, ConfigDecl)] {
        &self.decls
    }

    #[must_use]
    pub fn validate(&self, services_path: &Path) -> Vec<ExtensionConfigOutcome> {
        self.decls
            .iter()
            .map(|(owner, decl)| ExtensionConfigOutcome {
                extension_id: owner.clone(),
                config_key: format!("{}.config", decl.prefix()),
                error: validate_config(decl, services_path).err(),
            })
            .collect()
    }
}

/// One declared section's validation result.
#[derive(Debug)]
pub struct ExtensionConfigOutcome {
    pub extension_id: ExtensionId,
    pub config_key: String,
    pub error: Option<ExtensionConfigFileError>,
}

/// Why a declared section of the services tree did not validate.
#[derive(Debug, thiserror::Error)]
pub enum ExtensionConfigFileError {
    #[error("Cannot read {}: {source}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Cannot parse {}: {source}", .path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("Cannot convert to JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Validate(#[source] ExtensionConfigError),
}

pub fn validate_config(
    decl: &ConfigDecl,
    services_path: &Path,
) -> Result<(), ExtensionConfigFileError> {
    let path = services_path
        .join("config")
        .join(format!("{}.yaml", decl.prefix()));
    let config = if path.exists() {
        let content =
            std::fs::read_to_string(&path).map_err(|source| ExtensionConfigFileError::Read {
                path: path.clone(),
                source,
            })?;
        // JSON: a member's config section; the member's declaration owns its shape.
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|source| ExtensionConfigFileError::Parse { path, source })?;
        serde_json::to_value(yaml)?
    } else {
        serde_json::json!({})
    };
    decl.validate(&config)
        .map_err(ExtensionConfigFileError::Validate)
}
