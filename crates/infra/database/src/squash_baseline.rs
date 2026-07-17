//! Baseline-file placement for squashed extension migrations.
//!
//! [`SquashBaselineService`] owns where a squashed baseline lands on disk: it
//! locates an extension's source crate in the workspace layout
//! (`crates/{layer}/{extension_id}`, searching upward from a start directory
//! for the repository root) and writes the baseline SQL produced by
//! [`crate::lifecycle::MigrationService::squash_through`] into the crate's
//! `schema/migrations/` directory. Purely filesystem-facing — no SQL is
//! executed here — so it carries its own [`SquashBaselineError`] instead of
//! [`crate::RepositoryError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use thiserror::Error;

const CRATE_LAYERS: [&str; 5] = ["domain", "infra", "app", "shared", "entry"];

#[derive(Debug, Error)]
pub enum SquashBaselineError {
    #[error(
        "Could not locate source crate for extension '{extension_id}'. Tried: {tried:?}. The \
         squash tool maps extension id → crate dir as crates/{{layer}}/{{id}}; if your extension \
         lives elsewhere, write the baseline file by hand."
    )]
    ExtensionCrateNotFound {
        extension_id: String,
        tried: Vec<String>,
    },

    #[error("Failed to create directory {}", path.display())]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write baseline SQL to {}", path.display())]
    WriteBaseline {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct SquashBaselineService;

impl SquashBaselineService {
    pub fn baseline_target_path(
        start_dir: &Path,
        extension_id: &str,
        through: u32,
    ) -> Result<PathBuf, SquashBaselineError> {
        let crate_dir = locate_extension_crate(start_dir, extension_id)?;
        Ok(crate_dir
            .join("schema")
            .join("migrations")
            .join(format!("000_baseline_v{through}.sql")))
    }

    pub fn write_baseline_file(path: &Path, sql: &str) -> Result<(), SquashBaselineError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| SquashBaselineError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(path, sql).map_err(|source| SquashBaselineError::WriteBaseline {
            path: path.to_path_buf(),
            source,
        })
    }
}

fn locate_extension_crate(
    start_dir: &Path,
    extension_id: &str,
) -> Result<PathBuf, SquashBaselineError> {
    let repo_root = find_repo_root(start_dir).unwrap_or_else(|| start_dir.to_path_buf());

    let mut tried = Vec::new();
    for layer in CRATE_LAYERS {
        let candidate = repo_root.join("crates").join(layer).join(extension_id);
        if candidate.join("Cargo.toml").is_file() {
            return Ok(candidate);
        }
        tried.push(candidate.display().to_string());
    }

    Err(SquashBaselineError::ExtensionCrateNotFound {
        extension_id: extension_id.to_owned(),
        tried,
    })
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut cur = start;
    loop {
        if cur.join("Cargo.toml").is_file() && cur.join("crates").is_dir() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}
