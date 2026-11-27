//! The ledgers that record what the applier owns, so nothing else in the config
//! dir is ever touched.
//!
//! `skills/.systemprompt-managed.json` is `{version, ids, marketplaces}`,
//! pretty-printed with two spaces and no final newline; `ids` are the skill
//! directories written. `.systemprompt-owned.json` is `{files}`, sorted, in the
//! same layout: the bundle files written. A ledger that does not parse, or that
//! lists anything the applier could not have written, fails the read: what a
//! ledger lists is deleted, so a corrupt one is never read as empty or trusted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde::{Deserialize, Serialize};
use systemprompt_device::secure::read_optional;

use crate::projection::bundle::is_owned_path;
use crate::projection::error::ProjectionError;
use crate::projection::model::SkillSet;
use crate::projection::skill_md;

pub const SKILLS_DIR: &str = "skills";
pub const SKILL_FILE: &str = "SKILL.md";
pub const SKILLS_LEDGER: &str = ".systemprompt-managed.json";
pub const OWNED_LEDGER: &str = ".systemprompt-owned.json";
pub const RUNTIME_LEDGER: &str = ".systemprompt-runtime.json";
pub const AGENTS_FILE: &str = "AGENTS.md";

const MANAGED_DIR_MAX_LEN: usize = 64;

#[derive(Serialize, Deserialize)]
struct SkillsLedger {
    version: String,
    ids: Vec<String>,
    #[serde(default)]
    marketplaces: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct OwnedLedger {
    #[serde(default)]
    files: Vec<String>,
}

pub(crate) fn is_managed_dir(dir: &str) -> bool {
    let bytes = dir.as_bytes();
    match bytes.split_first() {
        Some((first, rest)) => {
            bytes.len() <= MANAGED_DIR_MAX_LEN
                && (first.is_ascii_lowercase() || first.is_ascii_digit())
                && rest
                    .iter()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        },
        None => false,
    }
}

pub(crate) fn read_skill_ids(path: &Path) -> Result<Vec<String>, ProjectionError> {
    let Some(bytes) = read(path)? else {
        return Ok(Vec::new());
    };
    let ledger: SkillsLedger =
        serde_json::from_slice(&bytes).map_err(|source| ProjectionError::LedgerJson {
            path: path.to_path_buf(),
            source,
        })?;
    if let Some(entry) = ledger.ids.iter().find(|id| !is_managed_dir(id)) {
        return Err(ProjectionError::LedgerEntry {
            path: path.to_path_buf(),
            entry: entry.clone(),
        });
    }
    Ok(ledger.ids)
}

pub(crate) fn read_owned(path: &Path) -> Result<Vec<String>, ProjectionError> {
    let Some(bytes) = read(path)? else {
        return Ok(Vec::new());
    };
    let ledger: OwnedLedger =
        serde_json::from_slice(&bytes).map_err(|source| ProjectionError::LedgerJson {
            path: path.to_path_buf(),
            source,
        })?;
    if let Some(entry) = ledger.files.iter().find(|file| !is_owned_path(file)) {
        return Err(ProjectionError::LedgerEntry {
            path: path.to_path_buf(),
            entry: entry.clone(),
        });
    }
    Ok(ledger.files)
}

pub(crate) fn skills_bytes(set: &SkillSet) -> Result<Vec<u8>, ProjectionError> {
    let ledger = SkillsLedger {
        version: skill_md::version(&set.files),
        ids: set.files.iter().map(|file| file.dir.clone()).collect(),
        marketplaces: set.marketplaces.clone(),
    };
    serde_json::to_vec_pretty(&ledger).map_err(|source| ProjectionError::Encode {
        what: "the skills ledger",
        source,
    })
}

pub(crate) fn owned_bytes(files: &[String]) -> Result<Vec<u8>, ProjectionError> {
    let ledger = OwnedLedger {
        files: files.to_vec(),
    };
    serde_json::to_vec_pretty(&ledger).map_err(|source| ProjectionError::Encode {
        what: "the owned-files ledger",
        source,
    })
}

fn read(path: &Path) -> Result<Option<Vec<u8>>, ProjectionError> {
    read_optional(path).map_err(ProjectionError::secure("read a ledger"))
}
